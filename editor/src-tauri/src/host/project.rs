use crate::*;

impl EditorHost {
    pub(crate) fn app_open_folder(&mut self, params: &Value) -> EngineResult<Value> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path'"))?;

        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open")
                .arg(path)
                .spawn()
                .map_err(|e| EngineError::other(format!("failed to open folder: {e}")))?;
        }
        #[cfg(target_os = "macos")]
        {
            Command::new("open")
                .arg(path)
                .spawn()
                .map_err(|e| EngineError::other(format!("failed to open folder: {e}")))?;
        }
        #[cfg(target_os = "windows")]
        {
            Command::new("explorer")
                .arg(path)
                .spawn()
                .map_err(|e| EngineError::other(format!("failed to open folder: {e}")))?;
        }

        Ok(serde_json::json!({ "opened": true }))
    }

    // ── Hub handlers ──

    pub(crate) fn hub_get_state(&mut self, _params: &Value) -> EngineResult<Value> {
        Ok(serde_json::json!({
            "page": match self.hub.page() {
                engine_editor::ui_state::HubPage::Projects => "projects",
                engine_editor::ui_state::HubPage::Installs => "installs",
                engine_editor::ui_state::HubPage::Settings => "settings",
            },
            "theme": match self.hub.preferences().theme {
                ThemePreference::Dark => "dark",
                ThemePreference::Light => "light",
                ThemePreference::System => "system",
            },
            "recent_projects": self.hub.filtered_projects().iter().map(|p| serde_json::json!({
                "name": p.name,
                "path": p.path.to_string_lossy(),
                "last_touched": p.last_touched,
                "toolchain_version": p.toolchain_version,
            })).collect::<Vec<_>>(),
            "locale": locale_code(self.hub.preferences().locale),
            "installs": self.hub.installs().iter().map(|i| serde_json::json!({
                "version": i.version,
                "path": i.path.to_string_lossy(),
                "editor_available": i.editor_available,
                "runtime_available": i.runtime_available,
            })).collect::<Vec<_>>(),
            "open_project": self.shell.project().map(|p| p.root.to_string_lossy()),
            "last_open_project": self.durable_state.last_open_project.as_ref().map(|p| p.to_string_lossy()),
            "reopen_last_project": self.hub.preferences().reopen_last_project,
            "desktop_integration": self.desktop_integration.as_json(),
        }))
    }

    pub(crate) fn hub_list_projects(&mut self, _params: &Value) -> EngineResult<Value> {
        let projects: Vec<Value> = self
            .hub
            .filtered_projects()
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "path": p.path.to_string_lossy(),
                    "last_touched": p.last_touched,
                    "toolchain_version": p.toolchain_version,
                })
            })
            .collect();
        Ok(serde_json::json!({ "projects": projects }))
    }

    pub(crate) fn hub_open_project(&mut self, params: &Value) -> EngineResult<Value> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path' parameter"))?;
        let project_path = PathBuf::from(path);

        // Load the project into the editor shell
        self.shell.open_project(&project_path)?;

        // Mark as recent
        let name = self
            .shell
            .project()
            .map(|p| p.name().to_owned())
            .unwrap_or_else(|| {
                project_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            });
        let metadata = ProjectMetadata::new(&name, &project_path, timestamp_now(), "0.1.0");
        self.hub.upsert_project(metadata);

        // Persist state
        self.hub.mark_project_open(project_path.clone());
        self.sync_durable_state();

        // Forward console entries from shell open
        self.drain_shell_console();

        Ok(serde_json::json!({
            "name": name,
            "path": project_path.to_string_lossy(),
        }))
    }

    pub(crate) fn hub_create_project(&mut self, params: &Value) -> EngineResult<Value> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'name' parameter"))?;
        let location = params
            .get("location")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'location' parameter"))?;

        let request = engine_editor::NewProjectRequest {
            name: name.to_owned(),
            location: Some(PathBuf::from(location)),
            template_id: params
                .get("template_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned()),
            toolchain_version: params
                .get("toolchain_version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned()),
        };

        let plan = self.hub.create_project_plan(&request)?;
        self.hub.create_project_files(&plan)?;

        let metadata = ProjectMetadata::new(
            &plan.name,
            &plan.path,
            timestamp_now(),
            &plan.toolchain_version,
        );
        self.hub.upsert_project(metadata);
        self.sync_durable_state();

        Ok(serde_json::json!({
            "name": plan.name,
            "path": plan.path.to_string_lossy(),
        }))
    }

    pub(crate) fn hub_delete_project(&mut self, params: &Value) -> EngineResult<Value> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path' parameter"))?;
        let confirmed = params
            .get("confirmed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let project_path = PathBuf::from(path);
        let decision = self.hub.request_project_deletion(
            &project_path,
            ProjectDeletionMode::RemoveRecent,
            confirmed,
        );

        match decision {
            ProjectDeletionDecision::RemovedFromRecent { .. } => {
                self.sync_durable_state();
                Ok(serde_json::json!({ "status": "removed" }))
            }
            ProjectDeletionDecision::NeedsConfirmation { .. } => {
                Ok(serde_json::json!({ "status": "needs_confirmation" }))
            }
            ProjectDeletionDecision::RefusedOpenProject { .. } => {
                Err(EngineError::config("cannot delete an open project"))
            }
            ProjectDeletionDecision::DeleteFilesApproved { .. } => Err(EngineError::config(
                "file deletion not supported through IPC",
            )),
        }
    }

    pub(crate) fn hub_set_theme(&mut self, params: &Value) -> EngineResult<Value> {
        let theme = params
            .get("theme")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'theme' parameter"))?;
        let pref = match theme {
            "light" => ThemePreference::Light,
            "dark" => ThemePreference::Dark,
            _ => ThemePreference::System,
        };
        self.hub.set_theme(pref);
        self.sync_durable_state();
        Ok(serde_json::json!({ "theme": theme }))
    }

    pub(crate) fn hub_set_page(&mut self, params: &Value) -> EngineResult<Value> {
        let page = params
            .get("page")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'page' parameter"))?;
        use engine_editor::ui_state::HubPage;
        let p = match page {
            "installs" => HubPage::Installs,
            "settings" => HubPage::Settings,
            _ => HubPage::Projects,
        };
        self.hub.set_page(p);
        self.sync_durable_state();
        Ok(serde_json::json!({ "page": page }))
    }

    pub(crate) fn hub_get_translations(&mut self, params: &Value) -> EngineResult<Value> {
        let requested_locale = params.get("locale").and_then(Value::as_str);
        let translations;
        let active_translations = if requested_locale.is_some() {
            translations = Translations::load(parse_locale(requested_locale));
            &translations
        } else {
            &self.translations
        };
        let entries: Vec<serde_json::Value> = active_translations
            .entries()
            .into_iter()
            .map(|(k, v)| serde_json::json!({ "key": k, "value": v }))
            .collect();
        Ok(serde_json::json!({
            "locale": locale_code(active_translations.locale()),
            "entries": entries,
        }))
    }

    pub(crate) fn hub_set_locale(&mut self, params: &Value) -> EngineResult<Value> {
        let locale_str = params
            .get("locale")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'locale' parameter"))?;
        let locale = parse_locale(Some(locale_str));
        self.hub.set_locale(locale);
        // Reload translations for the new locale
        self.translations = Translations::load(locale);
        self.sync_durable_state();
        Ok(serde_json::json!({ "locale": locale_code(locale) }))
    }

    // ── Project handlers ──

    pub(crate) fn project_list_assets(&mut self, _params: &Value) -> EngineResult<Value> {
        let Some(project) = self.shell.project() else {
            return Err(EngineError::config("no project open"));
        };

        let entries: Vec<Value> = project
            .database
            .iter_entries()
            .map(|entry| {
                serde_json::json!({
                    "guid": entry.guid.to_string(),
                    "path": entry.path.to_string_lossy(),
                    "kind": format!("{:?}", entry.kind),
                })
            })
            .collect();

        // Also get assets from ProjectContext.sorted_assets() for richer metadata
        let assets: Vec<Value> = project
            .sorted_assets()
            .iter()
            .map(|meta| {
                serde_json::json!({
                    "guid": meta.guid.to_string(),
                    "source_path": meta.source_path.to_string_lossy(),
                    "kind": format!("{:?}", meta.kind),
                    "importer": meta.importer,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "entries": entries,
            "assets": assets,
        }))
    }

    pub(crate) fn project_list_files(&mut self, params: &Value) -> EngineResult<Value> {
        let include_hidden = params
            .get("include_hidden")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let max_entries = params
            .get("max_entries")
            .and_then(Value::as_u64)
            .unwrap_or(2_000) as usize;

        let Some(project) = self.shell.project() else {
            return Err(EngineError::config("no project open"));
        };

        let root = project
            .root
            .canonicalize()
            .map_err(|source| EngineError::Filesystem {
                path: project.root.clone(),
                source,
            })?;
        let asset_root = project
            .root
            .join(&project.manifest.asset_root)
            .canonicalize()
            .ok();
        let mut stack = vec![root.clone()];
        let mut files = Vec::new();

        while let Some(dir) = stack.pop() {
            if files.len() >= max_entries {
                break;
            }
            let entries = std::fs::read_dir(&dir).map_err(|source| EngineError::Filesystem {
                path: dir.clone(),
                source,
            })?;
            let mut entries = entries.collect::<Result<Vec<_>, _>>().map_err(|source| {
                EngineError::Filesystem {
                    path: dir.clone(),
                    source,
                }
            })?;
            entries.sort_by_key(|entry| entry.path());

            for entry in entries {
                if files.len() >= max_entries {
                    break;
                }
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();
                let hidden = file_name.starts_with('.');
                if hidden && !include_hidden {
                    continue;
                }
                let metadata = entry.metadata().map_err(|source| EngineError::Filesystem {
                    path: path.clone(),
                    source,
                })?;
                let is_dir = metadata.is_dir();
                let relative = path
                    .strip_prefix(&root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let extension = path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let asset_path = asset_root.as_ref().and_then(|asset_root| {
                    path.strip_prefix(asset_root)
                        .ok()
                        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
                });
                let text = is_text_project_file(&extension, &file_name);

                files.push(serde_json::json!({
                    "path": relative,
                    "name": file_name,
                    "kind": if is_dir { "directory" } else { "file" },
                    "hidden": hidden,
                    "text": text,
                    "asset_path": asset_path,
                }));

                if is_dir {
                    stack.push(path);
                }
            }
        }

        Ok(serde_json::json!({
            "root": root.to_string_lossy(),
            "truncated": files.len() >= max_entries,
            "files": files,
        }))
    }

    pub(crate) fn project_import_file(&mut self, params: &Value) -> EngineResult<Value> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path'"))?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        project.import_file(std::path::PathBuf::from(path))?;
        self.console.push(engine_editor::ConsoleEntry {
            timestamp: "now".into(),
            level: engine_editor::ConsoleLevel::Info,
            source: engine_editor::ConsoleSource {
                subsystem: "editor".into(),
                file: None,
                line: None,
            },
            message: format!("Imported file: {path}"),
        });

        Ok(serde_json::json!({"imported": path}))
    }

    pub(crate) fn project_create_script(&mut self, params: &Value) -> EngineResult<Value> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'name'"))?;
        validate_file_name(name)?;
        let Some(project) = self.shell.project() else {
            return Err(EngineError::config("no project open"));
        };

        let script_root = project.root.join(project.manifest.primary_script_root());
        std::fs::create_dir_all(&script_root).map_err(|source| EngineError::Filesystem {
            path: script_root.clone(),
            source,
        })?;

        let script_path = format!("{}/{name}.varg", project.manifest.primary_script_root());
        let full_path = project.root.join(&script_path);

        let template = format!(
            r#"script {name} {{
    @export var speed: Float = 6.0

    func start() {{
        log("{name} ready")
    }}

    func update(_ dt: Float) {{
    }}
}}
"#
        );

        // Check if parent directory exists
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| EngineError::Filesystem {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        std::fs::write(&full_path, template).map_err(|source| EngineError::Filesystem {
            path: full_path.clone(),
            source,
        })?;

        self.console.push(engine_editor::ConsoleEntry {
            timestamp: "now".into(),
            level: engine_editor::ConsoleLevel::Info,
            source: engine_editor::ConsoleSource {
                subsystem: "editor".into(),
                file: Some(full_path.clone()),
                line: None,
            },
            message: format!("Created script: {}", full_path.display()),
        });

        Ok(serde_json::json!({
            "path": script_path,
            "full_path": full_path.to_string_lossy(),
        }))
    }

    pub(crate) fn project_create_material(&mut self, params: &Value) -> EngineResult<Value> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'name'"))?;
        validate_file_name(name)?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        let content = varg_material_template(name);
        let (asset_path, full_path) =
            write_project_asset(project, &format!("materials/{name}.vasset"), &content)?;
        project.rescan_assets()?;
        push_created_asset_console(&mut self.console, "material", &full_path);

        Ok(serde_json::json!({
            "path": asset_path,
            "full_path": full_path.to_string_lossy(),
        }))
    }

    pub(crate) fn project_create_texture_paint(&mut self, params: &Value) -> EngineResult<Value> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'name'"))?;
        validate_file_name(name)?;

        let width = params
            .get("width")
            .and_then(Value::as_u64)
            .unwrap_or(1024)
            .try_into()
            .map_err(|_| EngineError::config("texture paint width is too large"))?;
        let height = params
            .get("height")
            .and_then(Value::as_u64)
            .unwrap_or(1024)
            .try_into()
            .map_err(|_| EngineError::config("texture paint height is too large"))?;
        let base_color = params
            .get("base_color")
            .and_then(Value::as_str)
            .unwrap_or("#7aa2ff");

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        let paint_path = format!("textures/{name}.vpaint");
        let target = format!("textures/{name}.png");
        let paint = TexturePaintDocument::new(width, height, target.clone(), base_color)?;
        let content = paint.to_pretty_json()?;
        let (asset_path, full_path) = write_project_asset(project, &paint_path, &content)?;
        project.rescan_assets()?;
        push_created_asset_console(&mut self.console, "texture paint", &full_path);

        Ok(serde_json::json!({
            "path": asset_path,
            "target": target,
            "full_path": full_path.to_string_lossy(),
        }))
    }

    pub(crate) fn texture_paint_add_stroke(&mut self, params: &Value) -> EngineResult<Value> {
        let asset_path = params
            .get("asset")
            .and_then(Value::as_str)
            .ok_or_else(|| EngineError::config("missing 'asset'"))?;
        let layer_name = params
            .get("layer")
            .and_then(Value::as_str)
            .unwrap_or("paint");

        let stroke = TexturePaintStroke::from_params(params)?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };
        let asset_root = project.root.join(&project.manifest.asset_root);
        let full_path = resolve_existing_relative_path(&asset_root, asset_path)?;
        ensure_texture_paint_path(&full_path)?;
        let mut paint = TexturePaintDocument::read_from_path(&full_path)?;
        paint.add_stroke(layer_name, stroke);
        paint.write_to_path(&full_path)?;
        project.rescan_assets()?;

        Ok(serde_json::json!({
            "path": asset_path,
            "layers": paint.layers.len(),
            "strokes": paint.stroke_count(),
        }))
    }

    pub(crate) fn texture_paint_bake(&mut self, params: &Value) -> EngineResult<Value> {
        let asset_path = params
            .get("asset")
            .and_then(Value::as_str)
            .ok_or_else(|| EngineError::config("missing 'asset'"))?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };
        let asset_root = project.root.join(&project.manifest.asset_root);
        let full_path = resolve_existing_relative_path(&asset_root, asset_path)?;
        ensure_texture_paint_path(&full_path)?;
        let paint = TexturePaintDocument::read_from_path(&full_path)?;
        let target = params
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or(&paint.target);
        let target_path = resolve_writable_relative_path(&asset_root, target)?;
        paint.bake_to_png(&target_path)?;
        project.rescan_assets()?;

        self.console.push(engine_editor::ConsoleEntry {
            timestamp: timestamp_now(),
            level: engine_editor::ConsoleLevel::Info,
            source: engine_editor::ConsoleSource {
                subsystem: "assets".into(),
                file: Some(target_path.clone()),
                line: None,
            },
            message: format!("Baked texture paint: {}", target_path.display()),
        });

        Ok(serde_json::json!({
            "path": asset_path,
            "target": target,
            "full_path": target_path.to_string_lossy(),
        }))
    }

    pub(crate) fn project_create_animation(&mut self, params: &Value) -> EngineResult<Value> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'name'"))?;
        validate_file_name(name)?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        let content = varg_animation_template(name);
        let (asset_path, full_path) =
            write_project_asset(project, &format!("animations/{name}.vasset"), &content)?;
        project.rescan_assets()?;
        push_created_asset_console(&mut self.console, "animation", &full_path);

        Ok(serde_json::json!({
            "path": asset_path,
            "full_path": full_path.to_string_lossy(),
        }))
    }

    pub(crate) fn project_create_audio_bus(&mut self, params: &Value) -> EngineResult<Value> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'name'"))?;
        validate_file_name(name)?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        let content = varg_audio_bus_template(name);
        let (asset_path, full_path) =
            write_project_asset(project, &format!("audio/{name}.vasset"), &content)?;
        project.rescan_assets()?;
        push_created_asset_console(&mut self.console, "audio bus", &full_path);

        Ok(serde_json::json!({
            "path": asset_path,
            "full_path": full_path.to_string_lossy(),
        }))
    }

    pub(crate) fn project_create_prefab(&mut self, params: &Value) -> EngineResult<Value> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'name'"))?;
        validate_file_name(name)?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        let content = varg_prefab_template(name);
        let (asset_path, full_path) =
            write_project_asset(project, &format!("prefabs/{name}.vscene"), &content)?;
        project.rescan_assets()?;
        push_created_asset_console(&mut self.console, "prefab", &full_path);

        Ok(serde_json::json!({
            "path": asset_path,
            "full_path": full_path.to_string_lossy(),
        }))
    }

    pub(crate) fn project_get_settings_summary(&mut self, _params: &Value) -> EngineResult<Value> {
        let Some(project) = self.shell.project() else {
            return Err(EngineError::config("no project open"));
        };

        let build_path = project.root.join(&project.manifest.build_config);
        let build = std::fs::read_to_string(&build_path)
            .ok()
            .and_then(|text| toml::from_str::<engine_ecs::BuildConfiguration>(&text).ok());

        Ok(serde_json::json!({
            "project": {
                "name": project.manifest.name.clone(),
                "root": project.root.to_string_lossy(),
                "asset_root": project.manifest.asset_root.clone(),
                "default_scene": project.manifest.default_scene.clone(),
                "script_roots": project.manifest.script_roots.clone(),
                "build_config": project.manifest.build_config.clone(),
            },
            "build": build.map(|build| serde_json::json!({
                "target": build.target,
                "release": build.release,
                "features": build.features,
                "render": {
                    "quality": build.render.quality,
                    "upscaler": build.render.upscaler,
                    "dynamic_resolution": build.render.dynamic_resolution,
                    "target_fps": build.render.target_fps,
                    "min_render_scale_percent": build.render.min_render_scale_percent,
                    "max_render_scale_percent": build.render.max_render_scale_percent,
                    "sharpness_percent": build.render.sharpness_percent,
                    "anti_aliasing": build.render.anti_aliasing,
                }
            })),
        }))
    }

    pub(crate) fn project_version_control_status(
        &mut self,
        _params: &Value,
    ) -> EngineResult<Value> {
        let Some(project) = self.shell.project() else {
            return Err(EngineError::config("no project open"));
        };

        let output = Command::new("git")
            .arg("-C")
            .arg(&project.root)
            .arg("status")
            .arg("--porcelain=v1")
            .output();

        let Ok(output) = output else {
            return Ok(serde_json::json!({
                "available": false,
                "branch": null,
                "entries": [],
            }));
        };

        let branch = Command::new("git")
            .arg("-C")
            .arg(&project.root)
            .arg("branch")
            .arg("--show-current")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|branch| branch.trim().to_owned())
            .filter(|branch| !branch.is_empty());

        let stdout = String::from_utf8_lossy(&output.stdout);
        let entries = stdout
            .lines()
            .filter_map(|line| {
                if line.len() < 4 {
                    return None;
                }
                let status = line[..2].trim();
                let path = line[3..].to_owned();
                Some(serde_json::json!({
                    "status": if status.is_empty() { "modified" } else { status },
                    "path": path,
                }))
            })
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "available": output.status.success(),
            "branch": branch,
            "entries": entries,
        }))
    }

    pub(crate) fn project_create_scene(&mut self, params: &Value) -> EngineResult<Value> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'name'"))?;
        validate_file_name(name)?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        let content = varg_scene_template(name);
        let (asset_path, full_path) =
            write_project_asset(project, &format!("scenes/{name}.vscene"), &content)?;
        project.rescan_assets()?;
        push_created_asset_console(&mut self.console, "scene", &full_path);

        Ok(serde_json::json!({
            "path": asset_path,
            "full_path": full_path.to_string_lossy(),
        }))
    }

    pub(crate) fn project_list_asset_references(&mut self, params: &Value) -> EngineResult<Value> {
        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path'"))?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };
        project.rescan_assets()?;

        let asset_path = normalize_relative_path(path_str)?;
        let guid = project.database.guid_for_path(&asset_path).ok();
        let mut rows = Vec::new();

        if let Some(guid) = guid {
            for dependency in project.database.dependencies().dependencies(guid) {
                rows.push(asset_reference_row(
                    "dependency",
                    "Asset dependency",
                    resolve_asset_reference_label(project, dependency),
                ));
            }
            for dependent in project.database.dependencies().dependents(guid) {
                rows.push(asset_reference_row(
                    "dependent",
                    "Used by asset",
                    resolve_asset_reference_label(project, dependent),
                ));
            }
        }

        for (_entity, object) in project.scene.objects() {
            for component in &object.components {
                if let Some(guid) = guid {
                    collect_component_asset_references(&mut rows, &object.name, component, guid);
                }
                if let engine_ecs::ComponentData::Script(script) = component {
                    if script.source == path_str {
                        rows.push(asset_reference_row(
                            "scene",
                            "Script component",
                            format!("{} -> {}", object.name, script.source),
                        ));
                    }
                }
            }
            for script in &object.scripts {
                if script.source == path_str {
                    rows.push(asset_reference_row(
                        "scene",
                        "Legacy script",
                        format!("{} -> {}", object.name, script.source),
                    ));
                }
            }
        }

        rows.sort_by(|left, right| {
            left["kind"]
                .as_str()
                .cmp(&right["kind"].as_str())
                .then_with(|| left["label"].as_str().cmp(&right["label"].as_str()))
                .then_with(|| left["detail"].as_str().cmp(&right["detail"].as_str()))
        });
        rows.dedup();

        Ok(serde_json::json!({
            "guid": guid.map(|guid| guid.to_string()),
            "path": asset_path.to_string_lossy(),
            "references": rows,
        }))
    }

    pub(crate) fn project_rename_asset(&mut self, params: &Value) -> EngineResult<Value> {
        let old_path_str = params
            .get("old_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'old_path'"))?;
        let new_name = params
            .get("new_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'new_name'"))?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        validate_file_name(new_name)?;
        let asset_root = project.root.join(&project.manifest.asset_root);
        let old_path = resolve_existing_relative_path(&asset_root, old_path_str)?;
        let parent = old_path
            .parent()
            .ok_or_else(|| EngineError::config("cannot rename root directory"))?;
        let ext = old_path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        let new_path = parent.join(format!("{}{}", new_name, ext));
        let canonical_asset_root =
            asset_root
                .canonicalize()
                .map_err(|source| EngineError::Filesystem {
                    path: asset_root.clone(),
                    source,
                })?;
        if !new_path.starts_with(&canonical_asset_root) {
            return Err(EngineError::config("path is outside the project"));
        }

        std::fs::rename(&old_path, &new_path).map_err(|source| EngineError::Filesystem {
            path: old_path.clone(),
            source,
        })?;

        // Also rename the .meta file if it exists
        let old_meta = asset_meta_path_for_source(&old_path);
        if old_meta.exists() {
            let new_meta = asset_meta_path_for_source(&new_path);
            std::fs::rename(&old_meta, &new_meta).ok();
        }

        // Rescan to update the database
        project.rescan_assets()?;

        self.console.push(engine_editor::ConsoleEntry {
            timestamp: timestamp_now(),
            level: engine_editor::ConsoleLevel::Info,
            source: engine_editor::ConsoleSource {
                subsystem: "editor".into(),
                file: Some(new_path.clone()),
                line: None,
            },
            message: format!("Renamed asset: {} → {}", old_path_str, new_path.display()),
        });

        Ok(serde_json::json!({ "new_path": new_path.to_string_lossy() }))
    }

    pub(crate) fn project_delete_asset(&mut self, params: &Value) -> EngineResult<Value> {
        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path'"))?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        let asset_root = project.root.join(&project.manifest.asset_root);
        let path = resolve_existing_relative_path(&asset_root, path_str)?;

        // Delete the file
        if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(|source| EngineError::Filesystem {
                path: path.clone(),
                source,
            })?;
        } else {
            std::fs::remove_file(&path).map_err(|source| EngineError::Filesystem {
                path: path.clone(),
                source,
            })?;
            // Also delete the .meta file
            let meta_path = asset_meta_path_for_source(&path);
            if meta_path.exists() {
                std::fs::remove_file(&meta_path).ok();
            }
        }

        // Rescan to update the database
        project.rescan_assets()?;

        self.console.push(engine_editor::ConsoleEntry {
            timestamp: timestamp_now(),
            level: engine_editor::ConsoleLevel::Info,
            source: engine_editor::ConsoleSource {
                subsystem: "editor".into(),
                file: None,
                line: None,
            },
            message: format!("Deleted asset: {path_str}"),
        });

        Ok(serde_json::json!({ "deleted": true }))
    }

    pub(crate) fn project_reimport_asset(&mut self, params: &Value) -> EngineResult<Value> {
        let reimport_all = params.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
        if reimport_all {
            let Some(project) = self.shell.project_mut() else {
                return Err(EngineError::config("no project open"));
            };

            let asset_root = project.root.join(&project.manifest.asset_root);
            let mut stack = vec![asset_root.clone()];
            while let Some(path) = stack.pop() {
                let entries = match std::fs::read_dir(&path) {
                    Ok(entries) => entries,
                    Err(source) => {
                        return Err(EngineError::Filesystem { path, source });
                    }
                };
                for entry in entries {
                    let entry = entry.map_err(|source| EngineError::Filesystem {
                        path: asset_root.clone(),
                        source,
                    })?;
                    let entry_path = entry.path();
                    if entry_path.is_dir() {
                        stack.push(entry_path);
                    } else if entry_path.extension().is_some_and(|ext| ext == "meta") {
                        std::fs::remove_file(&entry_path).ok();
                    }
                }
            }

            project.rescan_assets()?;
            self.console.push(engine_editor::ConsoleEntry {
                timestamp: timestamp_now(),
                level: engine_editor::ConsoleLevel::Info,
                source: engine_editor::ConsoleSource {
                    subsystem: "editor".into(),
                    file: None,
                    line: None,
                },
                message: "Reimported all assets".into(),
            });

            return Ok(serde_json::json!({ "reimported": true }));
        }

        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path'"))?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        // Delete existing meta file to force reimport
        let asset_root = project.root.join(&project.manifest.asset_root);
        let path = resolve_existing_relative_path(&asset_root, path_str)?;
        let meta_path = asset_meta_path_for_source(&path);
        if meta_path.exists() {
            std::fs::remove_file(&meta_path).ok();
        }

        project.rescan_assets()?;

        self.console.push(engine_editor::ConsoleEntry {
            timestamp: timestamp_now(),
            level: engine_editor::ConsoleLevel::Info,
            source: engine_editor::ConsoleSource {
                subsystem: "editor".into(),
                file: None,
                line: None,
            },
            message: format!("Reimported asset: {path_str}"),
        });

        Ok(serde_json::json!({ "reimported": true }))
    }

    pub(crate) fn project_read_file(&mut self, params: &Value) -> EngineResult<Value> {
        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path'"))?;

        let Some(project) = self.shell.project() else {
            return Err(EngineError::config("no project open"));
        };

        let asset_root = project.root.join(&project.manifest.asset_root);
        let full_path = resolve_existing_relative_path(&asset_root, path_str)?;

        let content =
            std::fs::read_to_string(&full_path).map_err(|source| EngineError::Filesystem {
                path: full_path.clone(),
                source,
            })?;

        Ok(serde_json::json!({ "content": content }))
    }

    pub(crate) fn project_read_project_file(&mut self, params: &Value) -> EngineResult<Value> {
        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path'"))?;

        let Some(project) = self.shell.project() else {
            return Err(EngineError::config("no project open"));
        };

        let full_path = resolve_existing_relative_path(&project.root, path_str)?;
        let content =
            std::fs::read_to_string(&full_path).map_err(|source| EngineError::Filesystem {
                path: full_path.clone(),
                source,
            })?;

        Ok(serde_json::json!({ "content": content }))
    }

    pub(crate) fn project_write_file(&mut self, params: &Value) -> EngineResult<Value> {
        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path'"))?;
        let content = params
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'content'"))?;

        let Some(project) = self.shell.project() else {
            return Err(EngineError::config("no project open"));
        };

        let full_path = resolve_writable_project_source_path(project, path_str)?;

        let extension = full_path
            .extension()
            .and_then(|extension| extension.to_str());
        if matches!(extension, Some("varg" | "vscene" | "vasset")) {
            let diagnostics = engine_script_varg::diagnose_source(&full_path, content);
            if !diagnostics.is_empty() {
                return Err(EngineError::config(format_varg_diagnostics(
                    path_str,
                    &diagnostics,
                )));
            }
        }

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| EngineError::Filesystem {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        std::fs::write(&full_path, content).map_err(|source| EngineError::Filesystem {
            path: full_path.clone(),
            source,
        })?;

        Ok(serde_json::json!({ "saved": true }))
    }

    pub(crate) fn project_write_project_file(&mut self, params: &Value) -> EngineResult<Value> {
        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'path'"))?;
        let content = params
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::config("missing 'content'"))?;

        let Some(project) = self.shell.project_mut() else {
            return Err(EngineError::config("no project open"));
        };

        let full_path = resolve_writable_project_source_path(project, path_str)?;
        let extension = full_path
            .extension()
            .and_then(|extension| extension.to_str());
        if matches!(extension, Some("varg" | "vscene" | "vasset")) {
            let diagnostics = engine_script_varg::diagnose_source(&full_path, content);
            if !diagnostics.is_empty() {
                return Err(EngineError::config(format_varg_diagnostics(
                    path_str,
                    &diagnostics,
                )));
            }
        }

        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| EngineError::Filesystem {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::write(&full_path, content).map_err(|source| EngineError::Filesystem {
            path: full_path.clone(),
            source,
        })?;

        project.rescan_assets()?;

        Ok(serde_json::json!({ "saved": true }))
    }

    pub(crate) fn project_check_script(&mut self, params: &Value) -> EngineResult<Value> {
        let path_str = params
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| EngineError::config("missing 'path'"))?;
        let source = params
            .get("source")
            .and_then(Value::as_str)
            .ok_or_else(|| EngineError::config("missing 'source'"))?;

        let Some(project) = self.shell.project() else {
            return Err(EngineError::config("no project open"));
        };
        let full_path = resolve_writable_project_source_path(project, path_str)?;
        let extension = full_path
            .extension()
            .and_then(|extension| extension.to_str());
        let (diagnostics, ast) = if matches!(extension, Some("varg" | "vscene" | "vasset")) {
            let (ast, diagnostics) = engine_script_varg::parse_source(&full_path, source);
            let diagnostics = diagnostics
                .into_iter()
                .map(|diagnostic| {
                    serde_json::json!({
                        "code": diagnostic.code,
                        "severity": match diagnostic.severity {
                            engine_script_varg::VargDiagnosticSeverity::Error => "error",
                            engine_script_varg::VargDiagnosticSeverity::Warning => "warning",
                        },
                        "line": diagnostic.line,
                        "column": diagnostic.column,
                        "message": diagnostic.message,
                        "suggestion": diagnostic.suggestion,
                        "source_line": diagnostic.source_line,
                    })
                })
                .collect::<Vec<_>>();
            let ast = ast
                .map(|ast| serde_json::to_value(ast).unwrap_or(serde_json::Value::Null))
                .unwrap_or(serde_json::Value::Null);
            (diagnostics, ast)
        } else {
            (
                vec![serde_json::json!({
                    "code": "VARG0000",
                    "severity": "error",
                    "line": null,
                    "column": null,
                    "message": "unsupported script file extension",
                    "suggestion": "Use .varg for runtime scripts, .vscene for scenes, or .vasset for assets.",
                    "source_line": null,
                })],
                serde_json::Value::Null,
            )
        };
        Ok(serde_json::json!({
            "valid": diagnostics.is_empty(),
            "diagnostics": diagnostics,
            "ast": ast,
        }))
    }

    pub(crate) fn project_package(&mut self, params: &Value) -> EngineResult<Value> {
        let target = params
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("native");
        let format = params
            .get("format")
            .and_then(Value::as_str)
            .unwrap_or("folder");
        let channel = params
            .get("channel")
            .and_then(Value::as_str)
            .unwrap_or("release");
        let optimize_assets = params
            .get("optimize_assets")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let include_debug_symbols = params
            .get("include_debug_symbols")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let project_root = {
            let Some(project) = self.shell.project() else {
                return Err(EngineError::config("no project open"));
            };
            project.root.clone()
        };

        if self
            .shell
            .project()
            .is_some_and(|project| project.scene_dirty)
        {
            self.shell_save_scene(&serde_json::json!({}))?;
        }

        let output = package_project(&PackageRequest {
            project: project_root,
            repo_root: varg_repo_root(),
            target: PackageTarget::parse(target)?,
            format: PackageFormat::parse(format)?,
            channel: PackageChannel::parse(channel)?,
            optimize_assets,
            include_debug_symbols,
            output_dir: None,
        })?;

        self.console.push(ConsoleEntry {
            timestamp: timestamp_now(),
            level: ConsoleLevel::Info,
            source: engine_editor::ConsoleSource {
                subsystem: "build".to_owned(),
                file: None,
                line: None,
            },
            message: format!(
                "Packaged {} for {}/{} at {}",
                output.project,
                output.target,
                output.channel,
                output.path.display()
            ),
        });

        Ok(serde_json::json!({
            "project": output.project,
            "target": output.target,
            "format": output.format,
            "channel": output.channel,
            "path": output.path.to_string_lossy(),
            "binary": output.binary.map(|path| path.to_string_lossy().to_string()),
            "launcher": output.launcher.map(|path| path.to_string_lossy().to_string()),
            "assets_manifest": output.assets_manifest.to_string_lossy(),
            "asset_count": output.asset_count,
        }))
    }

    // ── Console handlers ──
}

fn resolve_writable_project_source_path(
    project: &engine_editor::ProjectContext,
    path: &str,
) -> EngineResult<PathBuf> {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str());
    if matches!(extension, Some("varg")) {
        resolve_writable_relative_path(&project.root, path)
    } else {
        let asset_root = project.root.join(&project.manifest.asset_root);
        resolve_writable_relative_path(&asset_root, path)
    }
}

const TEXTURE_PAINT_FORMAT: &str = "varg.texture_paint";
const MAX_TEXTURE_PAINT_SIZE: u32 = 8192;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TexturePaintDocument {
    format: String,
    version: u32,
    target: String,
    width: u32,
    height: u32,
    color_space: String,
    channels: String,
    base_color: String,
    layers: Vec<TexturePaintLayer>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TexturePaintLayer {
    name: String,
    opacity: f32,
    blend: String,
    visible: bool,
    strokes: Vec<TexturePaintStroke>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TexturePaintStroke {
    brush: String,
    color: String,
    size: f32,
    opacity: f32,
    space: String,
    points: Vec<TexturePaintPoint>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
struct TexturePaintPoint {
    u: f32,
    v: f32,
    #[serde(default = "default_pressure")]
    pressure: f32,
}

fn default_pressure() -> f32 {
    1.0
}

impl TexturePaintDocument {
    fn new(width: u32, height: u32, target: String, base_color: &str) -> EngineResult<Self> {
        validate_texture_paint_size(width, height)?;
        parse_hex_rgba(base_color)?;
        Ok(Self {
            format: TEXTURE_PAINT_FORMAT.to_owned(),
            version: 1,
            target,
            width,
            height,
            color_space: "srgb".to_owned(),
            channels: "rgba".to_owned(),
            base_color: base_color.to_owned(),
            layers: vec![TexturePaintLayer {
                name: "paint".to_owned(),
                opacity: 1.0,
                blend: "normal".to_owned(),
                visible: true,
                strokes: Vec::new(),
            }],
        })
    }

    fn read_from_path(path: &Path) -> EngineResult<Self> {
        let content = std::fs::read_to_string(path).map_err(|source| EngineError::Filesystem {
            path: path.to_path_buf(),
            source,
        })?;
        let document: Self = serde_json::from_str(&content)
            .map_err(|error| EngineError::config(format!("invalid texture paint file: {error}")))?;
        document.validate()?;
        Ok(document)
    }

    fn write_to_path(&self, path: &Path) -> EngineResult<()> {
        let content = self.to_pretty_json()?;
        std::fs::write(path, content).map_err(|source| EngineError::Filesystem {
            path: path.to_path_buf(),
            source,
        })
    }

    fn to_pretty_json(&self) -> EngineResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|error| EngineError::other(format!("texture paint serialize failed: {error}")))
    }

    fn validate(&self) -> EngineResult<()> {
        if self.format != TEXTURE_PAINT_FORMAT {
            return Err(EngineError::config("unsupported texture paint format"));
        }
        if self.version != 1 {
            return Err(EngineError::config("unsupported texture paint version"));
        }
        validate_texture_paint_size(self.width, self.height)?;
        parse_hex_rgba(&self.base_color)?;
        for layer in &self.layers {
            if layer.name.trim().is_empty() {
                return Err(EngineError::config("texture paint layer name is empty"));
            }
            for stroke in &layer.strokes {
                stroke.validate()?;
            }
        }
        Ok(())
    }

    fn add_stroke(&mut self, layer_name: &str, stroke: TexturePaintStroke) {
        let layer_name = layer_name.trim();
        let layer_name = if layer_name.is_empty() {
            "paint"
        } else {
            layer_name
        };
        if let Some(layer) = self
            .layers
            .iter_mut()
            .find(|layer| layer.name == layer_name)
        {
            layer.strokes.push(stroke);
            return;
        }
        self.layers.push(TexturePaintLayer {
            name: layer_name.to_owned(),
            opacity: 1.0,
            blend: "normal".to_owned(),
            visible: true,
            strokes: vec![stroke],
        });
    }

    fn stroke_count(&self) -> usize {
        self.layers
            .iter()
            .map(|layer| layer.strokes.len())
            .sum::<usize>()
    }

    fn bake_to_png(&self, path: &Path) -> EngineResult<()> {
        self.validate()?;
        let base = parse_hex_rgba(&self.base_color)?;
        let mut image = image::RgbaImage::from_pixel(self.width, self.height, image::Rgba(base));
        for layer in &self.layers {
            if !layer.visible {
                continue;
            }
            let layer_opacity = layer.opacity.clamp(0.0, 1.0);
            for stroke in &layer.strokes {
                stroke.paint(&mut image, layer_opacity)?;
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| EngineError::Filesystem {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        image.save(path).map_err(|error| {
            EngineError::other(format!(
                "failed to write baked texture {}: {error}",
                path.display()
            ))
        })
    }
}

impl TexturePaintStroke {
    fn from_params(params: &Value) -> EngineResult<Self> {
        if let Some(stroke) = params.get("stroke") {
            let stroke: Self = serde_json::from_value(stroke.clone()).map_err(|error| {
                EngineError::config(format!("invalid texture paint stroke: {error}"))
            })?;
            stroke.validate()?;
            return Ok(stroke);
        }

        let brush = params
            .get("brush")
            .and_then(Value::as_object)
            .and_then(|brush| brush.get("shape"))
            .and_then(Value::as_str)
            .or_else(|| params.get("brush").and_then(Value::as_str))
            .unwrap_or("soft_round")
            .to_owned();
        let color = params
            .get("brush")
            .and_then(Value::as_object)
            .and_then(|brush| brush.get("color"))
            .and_then(Value::as_str)
            .or_else(|| params.get("color").and_then(Value::as_str))
            .unwrap_or("#ffffff")
            .to_owned();
        let size = params
            .get("brush")
            .and_then(Value::as_object)
            .and_then(|brush| brush.get("size"))
            .and_then(Value::as_f64)
            .or_else(|| params.get("size").and_then(Value::as_f64))
            .unwrap_or(0.025) as f32;
        let opacity = params
            .get("brush")
            .and_then(Value::as_object)
            .and_then(|brush| brush.get("opacity"))
            .and_then(Value::as_f64)
            .or_else(|| params.get("opacity").and_then(Value::as_f64))
            .unwrap_or(1.0) as f32;
        let space = params
            .get("space")
            .and_then(Value::as_str)
            .unwrap_or("uv")
            .to_owned();
        let points_value = params
            .get("points")
            .ok_or_else(|| EngineError::config("missing 'points'"))?;
        let points = parse_texture_paint_points(points_value)?;

        let stroke = Self {
            brush,
            color,
            size,
            opacity,
            space,
            points,
        };
        stroke.validate()?;
        Ok(stroke)
    }

    fn validate(&self) -> EngineResult<()> {
        if self.space != "uv" {
            return Err(EngineError::config(
                "only uv texture paint strokes are supported",
            ));
        }
        if !matches!(self.brush.as_str(), "round" | "soft_round") {
            return Err(EngineError::config("unsupported texture paint brush"));
        }
        if self.points.is_empty() {
            return Err(EngineError::config("texture paint stroke has no points"));
        }
        if !(self.size.is_finite() && self.size > 0.0 && self.size <= 1.0) {
            return Err(EngineError::config(
                "texture paint stroke size must be in 0.0..=1.0 uv units",
            ));
        }
        if !(self.opacity.is_finite() && self.opacity >= 0.0 && self.opacity <= 1.0) {
            return Err(EngineError::config(
                "texture paint stroke opacity must be in 0.0..=1.0",
            ));
        }
        parse_hex_rgba(&self.color)?;
        for point in &self.points {
            if !(point.u.is_finite()
                && point.v.is_finite()
                && point.pressure.is_finite()
                && point.u >= 0.0
                && point.u <= 1.0
                && point.v >= 0.0
                && point.v <= 1.0
                && point.pressure >= 0.0
                && point.pressure <= 1.0)
            {
                return Err(EngineError::config(
                    "texture paint points must contain finite u/v/pressure in 0.0..=1.0",
                ));
            }
        }
        Ok(())
    }

    fn paint(&self, image: &mut image::RgbaImage, layer_opacity: f32) -> EngineResult<()> {
        let color = parse_hex_rgba(&self.color)?;
        for point in &self.points {
            paint_stamp(
                image,
                *point,
                self.size,
                self.opacity * layer_opacity,
                self.brush == "soft_round",
                color,
            );
        }
        Ok(())
    }
}

fn parse_texture_paint_points(value: &Value) -> EngineResult<Vec<TexturePaintPoint>> {
    let array = value
        .as_array()
        .ok_or_else(|| EngineError::config("'points' must be an array"))?;
    let mut points = Vec::with_capacity(array.len());
    for point in array {
        if let Some(object) = point.as_object() {
            let u = object
                .get("u")
                .and_then(Value::as_f64)
                .ok_or_else(|| EngineError::config("texture paint point missing 'u'"))?;
            let v = object
                .get("v")
                .and_then(Value::as_f64)
                .ok_or_else(|| EngineError::config("texture paint point missing 'v'"))?;
            let pressure = object
                .get("pressure")
                .and_then(Value::as_f64)
                .unwrap_or(1.0);
            points.push(TexturePaintPoint {
                u: u as f32,
                v: v as f32,
                pressure: pressure as f32,
            });
        } else if let Some(array) = point.as_array() {
            if array.len() < 2 {
                return Err(EngineError::config(
                    "texture paint point arrays must contain at least u and v",
                ));
            }
            points.push(TexturePaintPoint {
                u: array[0].as_f64().unwrap_or(f64::NAN) as f32,
                v: array[1].as_f64().unwrap_or(f64::NAN) as f32,
                pressure: array.get(2).and_then(Value::as_f64).unwrap_or(1.0) as f32,
            });
        } else {
            return Err(EngineError::config(
                "texture paint points must be objects or arrays",
            ));
        }
    }
    Ok(points)
}

fn validate_texture_paint_size(width: u32, height: u32) -> EngineResult<()> {
    if width == 0
        || height == 0
        || width > MAX_TEXTURE_PAINT_SIZE
        || height > MAX_TEXTURE_PAINT_SIZE
    {
        return Err(EngineError::config(format!(
            "texture paint size must be between 1 and {MAX_TEXTURE_PAINT_SIZE}"
        )));
    }
    Ok(())
}

fn ensure_texture_paint_path(path: &Path) -> EngineResult<()> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("vpaint") {
        return Err(EngineError::config(
            "texture paint asset must end with .vpaint",
        ));
    }
    Ok(())
}

fn parse_hex_rgba(input: &str) -> EngineResult<[u8; 4]> {
    let hex = input.trim().strip_prefix('#').unwrap_or(input.trim());
    let parse_pair = |pair: &str| {
        u8::from_str_radix(pair, 16)
            .map_err(|_| EngineError::config(format!("invalid hex color: {input}")))
    };
    match hex.len() {
        6 => Ok([
            parse_pair(&hex[0..2])?,
            parse_pair(&hex[2..4])?,
            parse_pair(&hex[4..6])?,
            255,
        ]),
        8 => Ok([
            parse_pair(&hex[0..2])?,
            parse_pair(&hex[2..4])?,
            parse_pair(&hex[4..6])?,
            parse_pair(&hex[6..8])?,
        ]),
        _ => Err(EngineError::config(format!("invalid hex color: {input}"))),
    }
}

fn paint_stamp(
    image: &mut image::RgbaImage,
    point: TexturePaintPoint,
    size_uv: f32,
    opacity: f32,
    soft: bool,
    color: [u8; 4],
) {
    let width = image.width();
    let height = image.height();
    let radius = ((size_uv * width.max(height) as f32) * point.pressure).max(1.0);
    let center_x = point.u * width.saturating_sub(1) as f32;
    let center_y = (1.0 - point.v) * height.saturating_sub(1) as f32;
    let min_x = (center_x - radius).floor().max(0.0) as u32;
    let max_x = (center_x + radius)
        .ceil()
        .min(width.saturating_sub(1) as f32) as u32;
    let min_y = (center_y - radius).floor().max(0.0) as u32;
    let max_y = (center_y + radius)
        .ceil()
        .min(height.saturating_sub(1) as f32) as u32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f32 - center_x;
            let dy = y as f32 - center_y;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > radius {
                continue;
            }
            let falloff = if soft {
                1.0 - (distance / radius).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let alpha = (opacity * falloff * (color[3] as f32 / 255.0)).clamp(0.0, 1.0);
            if alpha <= 0.0 {
                continue;
            }
            let pixel = image.get_pixel_mut(x, y);
            for channel in 0..3 {
                pixel[channel] = ((pixel[channel] as f32 * (1.0 - alpha))
                    + (color[channel] as f32 * alpha))
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            pixel[3] = ((pixel[3] as f32 * (1.0 - alpha)) + (color[3] as f32 * alpha))
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }
}

#[cfg(test)]
mod texture_paint_tests {
    use super::*;

    #[test]
    fn texture_paint_document_serializes_agent_friendly_strokes() {
        let mut document =
            TexturePaintDocument::new(64, 32, "textures/test.png".to_owned(), "#102030").unwrap();
        document.add_stroke(
            "moss",
            TexturePaintStroke {
                brush: "soft_round".to_owned(),
                color: "#4f7f4a".to_owned(),
                size: 0.1,
                opacity: 0.7,
                space: "uv".to_owned(),
                points: vec![TexturePaintPoint {
                    u: 0.5,
                    v: 0.5,
                    pressure: 0.8,
                }],
            },
        );

        let json = document.to_pretty_json().unwrap();
        let parsed: TexturePaintDocument = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.format, TEXTURE_PAINT_FORMAT);
        assert_eq!(parsed.stroke_count(), 1);
        assert_eq!(parsed.layers[1].name, "moss");
    }

    #[test]
    fn texture_paint_bake_writes_png_with_stroke_pixels() {
        let temp = tempfile::tempdir().unwrap();
        let png_path = temp.path().join("paint.png");
        let mut document =
            TexturePaintDocument::new(32, 32, "textures/paint.png".to_owned(), "#000000").unwrap();
        document.add_stroke(
            "paint",
            TexturePaintStroke {
                brush: "round".to_owned(),
                color: "#ff0000".to_owned(),
                size: 0.15,
                opacity: 1.0,
                space: "uv".to_owned(),
                points: vec![TexturePaintPoint {
                    u: 0.5,
                    v: 0.5,
                    pressure: 1.0,
                }],
            },
        );

        document.bake_to_png(&png_path).unwrap();
        let baked = image::open(&png_path).unwrap().to_rgba8();

        assert_eq!(baked.width(), 32);
        assert_eq!(baked.height(), 32);
        assert_eq!(baked.get_pixel(16, 15)[0], 255);
    }
}

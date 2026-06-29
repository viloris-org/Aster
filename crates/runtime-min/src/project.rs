use super::*;

/// Loaded project context used by runtime-game.
#[derive(Debug)]
pub struct RuntimeProject {
    /// Project root directory.
    pub root: PathBuf,
    /// Parsed project manifest.
    pub manifest: ProjectManifest,
    /// Parsed build configuration.
    pub build: BuildConfiguration,
    /// Default scene loaded from the manifest.
    pub scene: Scene,
}

/// Loads a project manifest and default scene.
pub fn load_runtime_project(project: impl AsRef<Path>) -> EngineResult<RuntimeProject> {
    let project = project.as_ref();
    let manifest_path = if project.is_dir() {
        project_manifest_path(project)
    } else {
        project.to_path_buf()
    };
    let root = manifest_path
        .parent()
        .ok_or_else(|| EngineError::config("project manifest must have a parent directory"))?
        .to_path_buf();
    let manifest_text =
        fs::read_to_string(&manifest_path).map_err(|source| EngineError::Filesystem {
            path: manifest_path.clone(),
            source,
        })?;
    let manifest = toml::from_str::<ProjectManifest>(&manifest_text)
        .map_err(|error| EngineError::config(format!("project manifest parse failed: {error}")))?;
    if let Some(diagnostic) = manifest.diagnostics().into_iter().next() {
        return Err(EngineError::config(format!(
            "{}: {}",
            diagnostic.path, diagnostic.message
        )));
    }
    let scene_path = root.join(&manifest.default_scene);
    let scene = load_scene_from_path(&scene_path)?;
    let build_path = root.join(&manifest.build_config);
    let build_text = fs::read_to_string(&build_path).map_err(|source| EngineError::Filesystem {
        path: build_path.clone(),
        source,
    })?;
    let build = toml::from_str::<BuildConfiguration>(&build_text).map_err(|error| {
        EngineError::config(format!("build configuration parse failed: {error}"))
    })?;
    if let Some(diagnostic) = build.diagnostics().into_iter().next() {
        return Err(EngineError::config(format!(
            "{}: {}",
            diagnostic.path, diagnostic.message
        )));
    }
    Ok(RuntimeProject {
        root,
        manifest,
        build,
        scene,
    })
}

fn load_scene_from_path(path: &Path) -> EngineResult<Scene> {
    let scene_text = fs::read_to_string(path).map_err(|source| EngineError::Filesystem {
        path: path.to_path_buf(),
        source,
    })?;
    if path.extension().and_then(|extension| extension.to_str()) != Some("vscene") {
        return Err(EngineError::config(format!(
            "expected native .vscene scene file, got {}",
            path.display()
        )));
    }
    let (scene, diagnostics) = compile_vscene_source_to_scene(path, &scene_text);
    if let Some(diagnostic) = diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.blocking)
    {
        return Err(EngineError::config(format!(
            "{} at {}:{}: {}",
            diagnostic.code,
            diagnostic.line.unwrap_or(1),
            diagnostic.column.unwrap_or(1),
            diagnostic.message
        )));
    }
    scene.ok_or_else(|| EngineError::config("native .vscene did not produce a scene"))
}

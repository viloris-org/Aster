//! Integration tests for the Tauri editor RPC backend.
//!
//! Tests the `EditorHost` RPC dispatch directly (headless, no Tauri window).
use aster_editor_tauri_lib::EditorHost;
use engine_editor::FileEditorStore;

fn temp_store() -> (tempfile::TempDir, FileEditorStore) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = FileEditorStore::new(&dir.path().join("aster-test-state.toml"));
    (dir, store)
}

fn create_host() -> EditorHost {
    let (_dir, store) = temp_store();
    EditorHost::new(store).expect("create editor host")
}

fn create_project(host: &mut EditorHost) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("create temp dir");
    host.handle(
        "hub/create_project",
        &serde_json::json!({
            "name": "TestProject",
            "location": tmp.path().to_str().unwrap(),
            "template_id": "three_d",
            "toolchain_version": "0.1.0",
        }),
    )
    .expect("create project");
    tmp
}

fn open_project(host: &mut EditorHost, tmp: &tempfile::TempDir) -> String {
    let path = tmp.path().join("TestProject");
    host.handle(
        "hub/open_project",
        &serde_json::json!({ "path": path.to_str().unwrap() }),
    )
    .expect("open project");
    path.to_string_lossy().to_string()
}

fn write_test_asset(project_root: &std::path::Path, relative_path: &str, bytes: &[u8]) {
    let full_path = project_root.join("assets").join(relative_path);
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).expect("create test asset parent directory");
    }
    std::fs::write(full_path, bytes).expect("write test asset");
}

fn test_material_json() -> Vec<u8> {
    serde_json::to_vec_pretty(&serde_json::json!({
        "version": engine_assets::CURRENT_SCHEMA_VERSION,
        "shader": "00000000000000000000000000000000",
        "textures": {},
        "parameters": {},
    }))
    .expect("serialize test material")
}

#[test]
fn host_initializes_with_empty_hub() {
    let mut host = create_host();
    let state = host
        .handle("hub/get_state", &serde_json::json!({}))
        .expect("get state");
    assert_eq!(state["page"], "projects", "starts on projects page");
    assert_eq!(state["theme"], "dark", "default theme is dark");
}

#[test]
fn hub_get_state_returns_recent_projects() {
    let mut host = create_host();
    let state = host
        .handle("hub/get_state", &serde_json::json!({}))
        .expect("get state");
    let projects = state["recent_projects"].as_array().unwrap();
    assert!(projects.is_empty(), "no projects initially");
}

#[test]
fn hub_set_theme_toggles_preference() {
    let mut host = create_host();

    let light = host
        .handle("hub/set_theme", &serde_json::json!({ "theme": "light" }))
        .expect("set theme light");
    assert_eq!(light["theme"], "light");

    let state = host
        .handle("hub/get_state", &serde_json::json!({}))
        .expect("get state");
    assert_eq!(state["theme"], "light");
}

#[test]
fn hub_set_page_changes_page() {
    let mut host = create_host();

    let result = host
        .handle("hub/set_page", &serde_json::json!({ "page": "settings" }))
        .expect("set page settings");
    assert_eq!(result["page"], "settings");

    let state = host
        .handle("hub/get_state", &serde_json::json!({}))
        .expect("get state");
    assert_eq!(state["page"], "settings");
}

#[test]
fn hub_set_locale_toggles_language() {
    let mut host = create_host();

    let zh = host
        .handle("hub/set_locale", &serde_json::json!({ "locale": "zh" }))
        .expect("set locale zh");
    assert_eq!(zh["locale"], "zh");

    let state = host
        .handle("hub/get_state", &serde_json::json!({}))
        .expect("get state");
    assert_eq!(state["locale"], "zh");

    let en = host
        .handle("hub/set_locale", &serde_json::json!({ "locale": "en" }))
        .expect("set locale en");
    assert_eq!(en["locale"], "en");

    let state = host
        .handle("hub/get_state", &serde_json::json!({}))
        .expect("get state");
    assert_eq!(state["locale"], "en");
}

#[test]
fn hub_create_project_returns_plan() {
    let mut host = create_host();
    let tmp = tempfile::tempdir().expect("create temp dir");
    let location = tmp.path().to_path_buf();

    let result = host
        .handle(
            "hub/create_project",
            &serde_json::json!({
                "name": "TestProject",
                "location": location.to_str().unwrap(),
                "template_id": "three_d",
                "toolchain_version": "0.1.0",
            }),
        )
        .expect("create project");
    assert_eq!(result["name"], "TestProject");
    let path = result["path"].as_str().unwrap();
    assert!(
        path.contains("TestProject"),
        "path should contain project name: {path}"
    );

    // Verify project appears in recent list
    let state = host
        .handle("hub/get_state", &serde_json::json!({}))
        .expect("get state");
    let projects = state["recent_projects"].as_array().unwrap();
    assert!(
        projects.iter().any(|p| p["name"] == "TestProject"),
        "project should appear in recent list"
    );

    // TempDir drops here, cleaning up automatically
}

#[test]
fn shell_mutations_are_dirty_and_undoable() {
    let mut host = create_host();
    let tmp = create_project(&mut host);
    open_project(&mut host, &tmp);

    let before = host
        .handle("shell/get_scene_tree", &serde_json::json!({}))
        .expect("scene tree");
    let before_len = before["objects"].as_array().unwrap().len();

    host.handle("shell/create_object", &serde_json::json!({}))
        .expect("create object");

    let state = host
        .handle("shell/get_state", &serde_json::json!({}))
        .expect("shell state");
    assert!(state["scene_dirty"].as_bool().unwrap());
    assert!(state["can_undo"].as_bool().unwrap());

    let after = host
        .handle("shell/get_scene_tree", &serde_json::json!({}))
        .expect("scene tree");
    assert_eq!(after["objects"].as_array().unwrap().len(), before_len + 1);

    host.handle("shell/undo", &serde_json::json!({}))
        .expect("undo");
    let undone = host
        .handle("shell/get_scene_tree", &serde_json::json!({}))
        .expect("scene tree");
    assert_eq!(undone["objects"].as_array().unwrap().len(), before_len);
}

#[test]
fn shell_save_clears_dirty_state() {
    let mut host = create_host();
    let tmp = create_project(&mut host);
    open_project(&mut host, &tmp);

    host.handle("shell/create_object", &serde_json::json!({}))
        .expect("create object");
    host.handle("shell/save_scene", &serde_json::json!({}))
        .expect("save scene");

    let state = host
        .handle("shell/get_state", &serde_json::json!({}))
        .expect("shell state");
    assert!(!state["scene_dirty"].as_bool().unwrap());
}

#[test]
fn shell_component_schema_lists_inspector_fields() {
    let mut host = create_host();
    let schemas = host
        .handle("shell/list_component_schemas", &serde_json::json!({}))
        .expect("list component schemas");
    let components = schemas["components"].as_array().unwrap();

    let camera = components
        .iter()
        .find(|schema| schema["type_id"] == "Camera")
        .expect("Camera schema exists");
    assert_eq!(camera["display_name"], "Camera");
    assert!(camera["version"].as_u64().unwrap() >= 1);
    let camera_fields = camera["fields"].as_array().unwrap();
    assert!(camera_fields
        .iter()
        .any(|field| { field["name"] == "vertical_fov_degrees" && field["kind"] == "F32" }));
    assert!(camera_fields
        .iter()
        .any(|field| { field["name"] == "primary" && field["kind"] == "Bool" }));

    let light = components
        .iter()
        .find(|schema| schema["type_id"] == "Light")
        .expect("Light schema exists");
    assert!(light["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| { field["name"] == "color" && field["kind"] == "Vec3" }));
}


#[test]
fn shell_component_schema_exposes_runtime_asset_reference_components() {
    let mut host = create_host();
    let schemas = host
        .handle("shell/list_component_schemas", &serde_json::json!({}))
        .expect("list component schemas");
    let components = schemas["components"].as_array().unwrap();

    let field_kind = |component_type: &str, field_name: &str| -> Option<String> {
        components
            .iter()
            .find(|schema| schema["type_id"] == component_type)
            .and_then(|schema| schema["fields"].as_array())
            .and_then(|fields| fields.iter().find(|field| field["name"] == field_name))
            .and_then(|field| field["kind"].as_str())
            .map(str::to_owned)
    };

    for (component_type, field_name) in [
        ("Skybox", "cubemap"),
        ("Sprite2D", "texture"),
        ("TileMap", "tileset"),
        ("AnimationPlayer", "clip"),
        ("SkinnedMeshRenderer", "mesh"),
        ("AudioStreamPlayer2D", "clip"),
        ("AudioStreamPlayer3D", "clip"),
    ] {
        assert_eq!(
            field_kind(component_type, field_name).as_deref(),
            Some("AssetRef"),
            "{component_type}.{field_name} should be an Inspector asset picker field"
        );
    }

    assert_eq!(
        field_kind("SkinnedMeshRenderer", "material").as_deref(),
        Some("Object"),
        "SkinnedMeshRenderer.material should use the MaterialRef picker"
    );
}

#[test]
fn shell_can_add_every_advertised_component_schema() {
    let mut host = create_host();
    let tmp = create_project(&mut host);
    open_project(&mut host, &tmp);

    let object = host
        .handle(
            "shell/create_object",
            &serde_json::json!({ "name": "Schema Coverage" }),
        )
        .expect("create object");
    let object_id = object["id"].as_str().unwrap();

    let schemas = host
        .handle("shell/list_component_schemas", &serde_json::json!({}))
        .expect("list component schemas");
    let advertised_types = schemas["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|schema| schema["type_id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();

    assert!(
        !advertised_types.is_empty(),
        "Inspector schema registry should advertise addable components"
    );

    for component_type in &advertised_types {
        host.handle(
            "shell/add_component",
            &serde_json::json!({
                "id": object_id,
                "component_type": component_type,
            }),
        )
        .unwrap_or_else(|err| {
            panic!("schema advertised {component_type}, but shell/add_component failed: {err}")
        });
    }

    let entity = host
        .handle("shell/get_entity", &serde_json::json!({ "id": object_id }))
        .expect("get entity");
    let actual_types = entity["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|component| component["type"].as_str().unwrap().to_owned())
        .collect::<std::collections::BTreeSet<_>>();

    for component_type in advertised_types {
        assert!(
            actual_types.contains(&component_type),
            "added entity should contain advertised component {component_type}"
        );
    }
}

#[test]
fn shell_component_fields_save_and_reopen() {
    let mut host = create_host();
    let tmp = create_project(&mut host);
    open_project(&mut host, &tmp);

    let object = host
        .handle(
            "shell/create_object",
            &serde_json::json!({ "name": "Editable Light" }),
        )
        .expect("create object");
    let object_id = object["id"].as_str().unwrap();

    host.handle(
        "shell/add_component",
        &serde_json::json!({ "id": object_id, "component_type": "Light" }),
    )
    .expect("add light component");
    host.handle(
        "shell/update_component",
        &serde_json::json!({
            "id": object_id,
            "component_type": "Light",
            "data": {
                "kind": "point",
                "intensity": 2.75,
                "color": { "x": 0.25, "y": 0.5, "z": 1.0 }
            },
        }),
    )
    .expect("update light component");

    let before_save = host
        .handle("shell/get_entity", &serde_json::json!({ "id": object_id }))
        .expect("get entity before save");
    let light = before_save["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|component| component["type"] == "Light")
        .expect("Light component before save");
    assert_eq!(light["data"]["kind"], "point");
    assert_eq!(light["data"]["intensity"], 2.75);
    assert_eq!(light["data"]["color"]["x"], 0.25);

    host.handle("shell/save_scene", &serde_json::json!({}))
        .expect("save scene");
    host.handle("shell/close_project", &serde_json::json!({}))
        .expect("close project");
    open_project(&mut host, &tmp);

    let after_reopen = host
        .handle("shell/get_entity", &serde_json::json!({ "id": object_id }))
        .expect("get entity after reopen");
    let light = after_reopen["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|component| component["type"] == "Light")
        .expect("Light component after reopen");
    assert_eq!(light["data"]["kind"], "point");
    assert_eq!(light["data"]["intensity"], 2.75);
    assert_eq!(light["data"]["color"]["x"], 0.25);
    assert_eq!(light["data"]["color"]["y"], 0.5);
    assert_eq!(light["data"]["color"]["z"], 1.0);
}

#[test]
fn shell_delete_object_removes_it_from_scene_tree() {
    let mut host = create_host();
    let tmp = create_project(&mut host);
    open_project(&mut host, &tmp);

    let created = host
        .handle("shell/create_object", &serde_json::json!({}))
        .expect("create object");
    let id = created["id"].as_str().unwrap();

    host.handle("shell/delete_object", &serde_json::json!({ "id": id }))
        .expect("delete object");

    let tree = host
        .handle("shell/get_scene_tree", &serde_json::json!({}))
        .expect("scene tree");
    let objects = tree["objects"].as_array().unwrap();
    assert!(
        objects.iter().all(|object| object["id"] != id),
        "deleted object should not remain in scene tree"
    );
}

#[test]
fn play_mode_starts_from_open_project_snapshot() {
    let mut host = create_host();
    let tmp = create_project(&mut host);
    open_project(&mut host, &tmp);

    let started = host
        .handle("play/start", &serde_json::json!({}))
        .expect("start play mode");
    assert!(started["playing"].as_bool().unwrap());

    let state = host
        .handle("play/get_state", &serde_json::json!({}))
        .expect("play state");
    assert!(state["playing"].as_bool().unwrap());

    host.handle("play/stop", &serde_json::json!({}))
        .expect("stop play mode");
    let stopped = host
        .handle("play/get_state", &serde_json::json!({}))
        .expect("play state");
    assert!(!stopped["playing"].as_bool().unwrap());
}

#[test]
fn project_creates_material_prefab_and_scene_assets() {
    let mut host = create_host();
    let tmp = create_project(&mut host);
    let project_root = open_project(&mut host, &tmp);
    let asset_root = std::path::Path::new(&project_root).join("assets");

    let material = host
        .handle(
            "project/create_material",
            &serde_json::json!({ "name": "new_material" }),
        )
        .expect("create material");
    assert_eq!(material["path"], "materials/new_material.material.json");
    let material_text =
        std::fs::read_to_string(asset_root.join("materials/new_material.material.json")).unwrap();
    engine_assets::MaterialFormat::from_json(&material_text).expect("material parses");

    let prefab = host
        .handle(
            "project/create_prefab",
            &serde_json::json!({ "name": "new_prefab" }),
        )
        .expect("create prefab");
    assert_eq!(prefab["path"], "prefabs/new_prefab.prefab.json");
    let prefab_text =
        std::fs::read_to_string(asset_root.join("prefabs/new_prefab.prefab.json")).unwrap();
    let parsed_prefab: engine_ecs::PrefabFile =
        serde_json::from_str(&prefab_text).expect("prefab parses");
    assert_eq!(parsed_prefab.name, "new_prefab");

    let scene = host
        .handle(
            "project/create_scene",
            &serde_json::json!({ "name": "new_scene" }),
        )
        .expect("create scene");
    assert_eq!(scene["path"], "scenes/new_scene.scene.json");
    let scene_text =
        std::fs::read_to_string(asset_root.join("scenes/new_scene.scene.json")).unwrap();
    engine_ecs::Scene::from_json(&scene_text).expect("scene parses");

    let assets = host
        .handle("project/list_assets", &serde_json::json!({}))
        .expect("list assets");
    let asset_rows = assets["assets"].as_array().unwrap();
    assert!(asset_rows.iter().any(|asset| asset["source_path"]
        == "materials/new_material.material.json"
        && asset["kind"] == "Material"));
    assert!(asset_rows.iter().any(|asset| asset["source_path"]
        == "prefabs/new_prefab.prefab.json"
        && asset["kind"] == "Prefab"));
    assert!(asset_rows.iter().any(
        |asset| asset["source_path"] == "scenes/new_scene.scene.json" && asset["kind"] == "Scene"
    ));
}

#[test]
fn project_lists_scene_references_for_script_assets() {
    let mut host = create_host();
    let tmp = create_project(&mut host);
    open_project(&mut host, &tmp);

    let script = host
        .handle(
            "project/create_script",
            &serde_json::json!({ "name": "controller", "backend": "rhai" }),
        )
        .expect("create script");
    let script_path = script["path"].as_str().unwrap();

    let object = host
        .handle(
            "shell/create_object",
            &serde_json::json!({ "name": "Scripted Object" }),
        )
        .expect("create object");
    let object_id = object["id"].as_str().unwrap();
    host.handle(
        "shell/add_component",
        &serde_json::json!({ "id": object_id, "component_type": "Script" }),
    )
    .expect("add script component");
    host.handle(
        "shell/update_component",
        &serde_json::json!({
            "id": object_id,
            "component_type": "Script",
            "data": { "script": script_path },
        }),
    )
    .expect("set script component path");

    let references = host
        .handle(
            "project/list_asset_references",
            &serde_json::json!({ "path": script_path }),
        )
        .expect("list references");
    let rows = references["references"].as_array().unwrap();
    assert!(rows.iter().any(|row| row["kind"] == "scene"
        && row["label"] == "Script component"
        && row["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("Scripted Object"))));
}

#[test]
fn shell_asset_references_save_close_and_reopen_across_runtime_components() {
    let mut host = create_host();
    let tmp = create_project(&mut host);
    let project_root = open_project(&mut host, &tmp);
    let project_root = std::path::PathBuf::from(project_root);

    write_test_asset(&project_root, "models/hero.gltf", br#"{"asset":{"version":"2.0"}}"#);
    write_test_asset(&project_root, "textures/sprite.png", b"fake png payload");
    write_test_asset(&project_root, "textures/tileset.png", b"fake tileset payload");
    write_test_asset(&project_root, "textures/skybox.png", b"fake skybox payload");
    write_test_asset(&project_root, "audio/jump.wav", b"fake wav payload");
    write_test_asset(&project_root, "animations/run.anim.json", br#"{"name":"run","frames":[]}"#);
    write_test_asset(&project_root, "materials/hero.material.json", &test_material_json());

    let reference_guid = |host: &mut EditorHost, path: &str| -> String {
        host.handle(
            "project/list_asset_references",
            &serde_json::json!({ "path": path }),
        )
        .unwrap_or_else(|error| panic!("{path} should be a known project asset: {error}"))["guid"]
            .as_str()
            .unwrap()
            .to_owned()
    };

    let model_guid = reference_guid(&mut host, "models/hero.gltf");
    let sprite_guid = reference_guid(&mut host, "textures/sprite.png");
    let tileset_guid = reference_guid(&mut host, "textures/tileset.png");
    let skybox_guid = reference_guid(&mut host, "textures/skybox.png");
    let audio_guid = reference_guid(&mut host, "audio/jump.wav");
    let animation_guid = reference_guid(&mut host, "animations/run.anim.json");
    let material_guid = reference_guid(&mut host, "materials/hero.material.json");

    let object = host
        .handle(
            "shell/create_object",
            &serde_json::json!({ "name": "Persistent Asset Object" }),
        )
        .expect("create object");
    let object_id = object["id"].as_str().unwrap();

    for component_type in [
        "MeshRenderer",
        "Skybox",
        "Sprite2D",
        "TileMap",
        "AnimationPlayer",
        "SkinnedMeshRenderer",
        "AudioSource",
        "AudioStreamPlayer2D",
        "AudioStreamPlayer3D",
    ] {
        host.handle(
            "shell/add_component",
            &serde_json::json!({ "id": object_id, "component_type": component_type }),
        )
        .unwrap_or_else(|error| panic!("add {component_type}: {error}"));
    }

    for (component_type, data) in [
        (
            "MeshRenderer",
            serde_json::json!({
                "mesh": model_guid,
                "material": { "asset": material_guid, "builtin": null },
            }),
        ),
        ("Skybox", serde_json::json!({ "cubemap": skybox_guid })),
        ("Sprite2D", serde_json::json!({ "texture": sprite_guid })),
        ("TileMap", serde_json::json!({ "tileset": tileset_guid })),
        ("AnimationPlayer", serde_json::json!({ "clip": animation_guid })),
        (
            "SkinnedMeshRenderer",
            serde_json::json!({
                "mesh": model_guid,
                "material": { "asset": material_guid, "builtin": null },
            }),
        ),
        ("AudioSource", serde_json::json!({ "clip": audio_guid })),
        ("AudioStreamPlayer2D", serde_json::json!({ "clip": audio_guid })),
        ("AudioStreamPlayer3D", serde_json::json!({ "clip": audio_guid })),
    ] {
        host.handle(
            "shell/update_component",
            &serde_json::json!({
                "id": object_id,
                "component_type": component_type,
                "data": data,
            }),
        )
        .unwrap_or_else(|error| panic!("update {component_type}: {error}"));
    }

    host.handle("shell/save_scene", &serde_json::json!({}))
        .expect("save scene");
    host.handle("shell/close_project", &serde_json::json!({}))
        .expect("close project");
    open_project(&mut host, &tmp);

    let reopened = host
        .handle("shell/get_entity", &serde_json::json!({ "id": object_id }))
        .expect("get entity after reopen");
    let component_data = |component_type: &str| -> serde_json::Value {
        reopened["components"]
            .as_array()
            .unwrap()
            .iter()
            .find(|component| component["type"] == component_type)
            .unwrap_or_else(|| panic!("{component_type} should persist after reopen"))["data"]
            .clone()
    };

    assert_eq!(component_data("MeshRenderer")["mesh"], model_guid);
    assert_eq!(
        component_data("MeshRenderer")["material"]["asset"],
        material_guid
    );
    assert_eq!(component_data("Skybox")["cubemap"], skybox_guid);
    assert_eq!(component_data("Sprite2D")["texture"], sprite_guid);
    assert_eq!(component_data("TileMap")["tileset"], tileset_guid);
    assert_eq!(component_data("AnimationPlayer")["clip"], animation_guid);
    assert_eq!(component_data("SkinnedMeshRenderer")["mesh"], model_guid);
    assert_eq!(
        component_data("SkinnedMeshRenderer")["material"]["asset"],
        material_guid
    );
    assert_eq!(component_data("AudioSource")["clip"], audio_guid);
    assert_eq!(component_data("AudioStreamPlayer2D")["clip"], audio_guid);
    assert_eq!(component_data("AudioStreamPlayer3D")["clip"], audio_guid);

    let assert_scene_reference = |host: &mut EditorHost, path: &str, label: &str, detail: &str| {
        let references = host
            .handle(
                "project/list_asset_references",
                &serde_json::json!({ "path": path }),
            )
            .unwrap_or_else(|error| panic!("list references for {path}: {error}"));
        let rows = references["references"].as_array().unwrap();
        assert!(
            rows.iter().any(|row| row["kind"] == "scene"
                && row["label"] == label
                && row["detail"].as_str().is_some_and(|value| value.contains(detail))),
            "{path} should report scene reference {label} / {detail}, got {rows:#?}"
        );
    };

    assert_scene_reference(&mut host, "models/hero.gltf", "MeshRenderer mesh", "MeshRenderer.mesh");
    assert_scene_reference(
        &mut host,
        "models/hero.gltf",
        "SkinnedMeshRenderer mesh",
        "SkinnedMeshRenderer.mesh",
    );
    assert_scene_reference(
        &mut host,
        "materials/hero.material.json",
        "MeshRenderer material",
        "MeshRenderer.material",
    );
    assert_scene_reference(
        &mut host,
        "materials/hero.material.json",
        "SkinnedMeshRenderer material",
        "SkinnedMeshRenderer.material",
    );
    assert_scene_reference(&mut host, "textures/skybox.png", "Skybox cubemap", "Skybox.cubemap");
    assert_scene_reference(&mut host, "textures/sprite.png", "Sprite2D texture", "Sprite2D.texture");
    assert_scene_reference(&mut host, "textures/tileset.png", "TileMap tileset", "TileMap.tileset");
    assert_scene_reference(
        &mut host,
        "animations/run.anim.json",
        "AnimationPlayer clip",
        "AnimationPlayer.clip",
    );
    assert_scene_reference(&mut host, "audio/jump.wav", "AudioSource clip", "AudioSource.clip");
    assert_scene_reference(
        &mut host,
        "audio/jump.wav",
        "AudioStreamPlayer2D clip",
        "AudioStreamPlayer2D.clip",
    );
    assert_scene_reference(
        &mut host,
        "audio/jump.wav",
        "AudioStreamPlayer3D clip",
        "AudioStreamPlayer3D.clip",
    );
}

#[test]
fn diagnostics_health_check_reports_real_project_and_ai_gateway_state() {
    let mut host = create_host();
    let tmp = create_project(&mut host);
    open_project(&mut host, &tmp);

    let report = host
        .handle("diagnostics/run_health_check", &serde_json::json!({}))
        .expect("run diagnostics health check");

    assert!(
        report["summary"]["total"].as_u64().unwrap() > 0,
        "diagnostics should report concrete capability checks"
    );
    let groups = report["groups"].as_array().unwrap();
    assert!(
        groups.iter().any(|group| group["id"] == "project_scene"),
        "project/scene group should be present: {groups:?}"
    );
    for required_group in [
        "editor_workspace",
        "assets_scripts",
        "settings_language",
        "quest_lab",
        "build_export",
    ] {
        assert!(
            groups.iter().any(|group| group["id"] == required_group),
            "diagnostics should report {required_group} group: {groups:?}"
        );
    }
    assert!(
        groups.iter().any(|group| group["id"] == "ai_gateway"),
        "AI gateway group should be present: {groups:?}"
    );
    let project_items = groups
        .iter()
        .find(|group| group["id"] == "project_scene")
        .unwrap()["items"]
        .as_array()
        .unwrap();
    assert!(
        project_items
            .iter()
            .any(|item| item["id"] == "project.open" && item["status"] == "ok"),
        "opened project should be recognized by diagnostics: {project_items:?}"
    );
    let ai_items = groups
        .iter()
        .find(|group| group["id"] == "ai_gateway")
        .unwrap()["items"]
        .as_array()
        .unwrap();
    assert!(
        ai_items.iter().any(|item| item["id"] == "ai.gateway"
            && item["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|evidence| evidence == "provider=stub")),
        "default test host should clearly report the stub AI provider: {ai_items:?}"
    );
    let editor_items = groups
        .iter()
        .find(|group| group["id"] == "editor_workspace")
        .unwrap()["items"]
        .as_array()
        .unwrap();
    assert!(
        editor_items.iter().any(|item| item["id"] == "editor.scene_tree"
            && item["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|evidence| evidence.as_str().is_some_and(|value| value.starts_with("objects=")))),
        "editor workspace should expose real scene-tree evidence: {editor_items:?}"
    );
    let settings_items = groups
        .iter()
        .find(|group| group["id"] == "settings_language")
        .unwrap()["items"]
        .as_array()
        .unwrap();
    assert!(
        settings_items.iter().any(|item| item["id"] == "settings.locale"
            && item["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|evidence| evidence == "default_locale=zh")),
        "settings diagnostics should record Chinese default locale: {settings_items:?}"
    );
    let quest_items = groups
        .iter()
        .find(|group| group["id"] == "quest_lab")
        .unwrap()["items"]
        .as_array()
        .unwrap();
    assert!(
        quest_items.iter().any(|item| item["id"] == "quest.workflow"
            && item["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|evidence| evidence.as_str().is_some_and(|value| value.contains("quest/execute")))),
        "quest lab diagnostics should expose executable workflow RPC evidence: {quest_items:?}"
    );
    let build_items = groups
        .iter()
        .find(|group| group["id"] == "build_export")
        .unwrap()["items"]
        .as_array()
        .unwrap();
    assert!(
        build_items.iter().any(|item| item["id"] == "build.desktop_folder"
            && item["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|evidence| evidence.as_str().is_some_and(|value| value.starts_with("target=")))),
        "build diagnostics should expose concrete desktop package target: {build_items:?}"
    );
}

#[test]
fn diagnostics_apply_fix_clears_console_and_rescans_assets() {
    let mut host = create_host();
    let tmp = create_project(&mut host);
    let project_root = open_project(&mut host, &tmp);
    let project_root = std::path::PathBuf::from(project_root);

    host.handle(
        "console/push_entry",
        &serde_json::json!({
            "level": "error",
            "subsystem": "test",
            "message": "synthetic diagnostics error",
        }),
    )
    .expect("push console entry");
    let before_clear = host
        .handle("console/get_entries", &serde_json::json!({}))
        .expect("get console entries before fix");
    assert!(
        before_clear["entries"].as_array().unwrap().iter().any(|entry| entry["message"] == "synthetic diagnostics error"),
        "synthetic diagnostics entry should be present before clear: {before_clear:?}"
    );

    let clear_result = host
        .handle(
            "diagnostics/apply_fix",
            &serde_json::json!({ "fix_id": "clear_console" }),
        )
        .expect("clear console through diagnostics fix");
    assert_eq!(clear_result["applied"], true);
    let after_clear = host
        .handle("console/get_entries", &serde_json::json!({}))
        .expect("get console entries after fix");
    assert!(after_clear["entries"].as_array().unwrap().is_empty());

    write_test_asset(&project_root, "textures/new_texture.png", b"fake png payload");
    let rescan_result = host
        .handle(
            "diagnostics/apply_fix",
            &serde_json::json!({ "fix_id": "rescan_assets" }),
        )
        .expect("rescan assets through diagnostics fix");
    assert_eq!(rescan_result["applied"], true);
    let assets = host
        .handle("project/list_assets", &serde_json::json!({}))
        .expect("list assets after diagnostics rescan");
    assert!(
        assets["assets"].as_array().unwrap().iter().any(|asset| asset["source_path"] == "textures/new_texture.png"),
        "rescan fix should make newly written assets visible: {assets:?}"
    );
}

#[test]
fn shell_update_component_accepts_asset_guid_strings_from_inspector() {
    let mut host = create_host();
    let tmp = create_project(&mut host);
    open_project(&mut host, &tmp);

    let material = host
        .handle(
            "project/create_material",
            &serde_json::json!({ "name": "selectable_material" }),
        )
        .expect("create material");
    let material_path = material["path"].as_str().unwrap();
    let assets = host
        .handle("project/list_assets", &serde_json::json!({}))
        .expect("list assets");
    let guid = assets["assets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|asset| asset["source_path"] == material_path)
        .and_then(|asset| asset["guid"].as_str())
        .expect("created asset has guid");

    let object = host
        .handle(
            "shell/create_object",
            &serde_json::json!({ "name": "Renderable Object" }),
        )
        .expect("create object");
    let object_id = object["id"].as_str().unwrap();
    host.handle(
        "shell/add_component",
        &serde_json::json!({ "id": object_id, "component_type": "MeshRenderer" }),
    )
    .expect("add mesh renderer");

    host.handle(
        "shell/update_component",
        &serde_json::json!({
            "id": object_id,
            "component_type": "MeshRenderer",
            "data": { "mesh": guid },
        }),
    )
    .expect("set mesh from asset picker guid string");

    let entity = host
        .handle("shell/get_entity", &serde_json::json!({ "id": object_id }))
        .expect("get entity");
    let mesh = entity["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|component| component["type"] == "MeshRenderer")
        .and_then(|component| component["data"]["mesh"].as_str())
        .expect("mesh asset id serialized as stable hex string");
    assert_eq!(mesh, guid);
}

#[test]
fn shell_update_component_accepts_material_refs_from_inspector() {
    let mut host = create_host();
    let tmp = create_project(&mut host);
    open_project(&mut host, &tmp);

    let material = host
        .handle(
            "project/create_material",
            &serde_json::json!({ "name": "selectable_surface" }),
        )
        .expect("create material");
    let material_path = material["path"].as_str().unwrap();
    let assets = host
        .handle("project/list_assets", &serde_json::json!({}))
        .expect("list assets");
    let guid = assets["assets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|asset| asset["source_path"] == material_path)
        .and_then(|asset| asset["guid"].as_str())
        .expect("created material has guid");

    let object = host
        .handle(
            "shell/create_object",
            &serde_json::json!({ "name": "Material Object" }),
        )
        .expect("create object");
    let object_id = object["id"].as_str().unwrap();
    host.handle(
        "shell/add_component",
        &serde_json::json!({ "id": object_id, "component_type": "MeshRenderer" }),
    )
    .expect("add mesh renderer");

    host.handle(
        "shell/update_component",
        &serde_json::json!({
            "id": object_id,
            "component_type": "MeshRenderer",
            "data": { "material": { "asset": guid, "builtin": null } },
        }),
    )
    .expect("set material from inspector asset picker");

    let entity = host
        .handle("shell/get_entity", &serde_json::json!({ "id": object_id }))
        .expect("get entity");
    let material = entity["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|component| component["type"] == "MeshRenderer")
        .and_then(|component| component["data"]["material"].as_object())
        .expect("MeshRenderer material should stay an object");

    assert_eq!(material["asset"], guid);
    assert!(material["builtin"].is_null());

    host.handle("shell/save_scene", &serde_json::json!({}))
        .expect("save scene");
    host.handle("shell/close_project", &serde_json::json!({}))
        .expect("close project");
    open_project(&mut host, &tmp);

    let reopened = host
        .handle("shell/get_entity", &serde_json::json!({ "id": object_id }))
        .expect("get entity after reopen");
    let reopened_material = reopened["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|component| component["type"] == "MeshRenderer")
        .and_then(|component| component["data"]["material"].as_object())
        .expect("MeshRenderer material after reopen");

    assert_eq!(reopened_material["asset"], guid);
    assert!(reopened_material["builtin"].is_null());
}

#[test]
fn console_push_and_retrieve_entries() {
    let mut host = create_host();

    // Push an error entry
    host.handle(
        "console/push_entry",
        &serde_json::json!({
            "level": "error",
            "message": "test error message",
            "subsystem": "test",
        }),
    )
    .expect("push entry");

    // Retrieve entries
    let entries = host
        .handle("console/get_entries", &serde_json::json!({}))
        .expect("get entries");
    let list = entries["entries"].as_array().unwrap();
    assert!(!list.is_empty(), "should have at least one entry");

    let entry = &list[0];
    assert_eq!(entry["level"], "error");
    assert_eq!(entry["message"], "test error message");
}

#[test]
fn console_clear_removes_entries() {
    let mut host = create_host();

    host.handle(
        "console/push_entry",
        &serde_json::json!({
            "level": "info",
            "message": "temp message",
            "subsystem": "test",
        }),
    )
    .expect("push entry");

    host.handle("console/clear", &serde_json::json!({}))
        .expect("clear");

    let entries = host
        .handle("console/get_entries", &serde_json::json!({}))
        .expect("get entries");
    let list = entries["entries"].as_array().unwrap();
    assert!(list.is_empty(), "console should be empty after clear");
}

#[test]
fn console_push_trace_debug_warn_levels() {
    let mut host = create_host();

    for (level, label) in &[
        ("trace", "trace"),
        ("debug", "debug"),
        ("warn", "warn"),
        ("error", "error"),
        ("info", "info"),
    ] {
        host.handle(
            "console/push_entry",
            &serde_json::json!({
                "level": level,
                "message": format!("{label} message"),
                "subsystem": "test",
            }),
        )
        .expect("push entry");
    }

    let entries = host
        .handle("console/get_entries", &serde_json::json!({}))
        .expect("get entries");
    let list = entries["entries"].as_array().unwrap();
    assert_eq!(list.len(), 5);
    assert_eq!(list[0]["level"], "trace");
    assert_eq!(list[1]["level"], "debug");
    assert_eq!(list[2]["level"], "warn");
    assert_eq!(list[3]["level"], "error");
    assert_eq!(list[4]["level"], "info");
}

#[test]
fn unknown_rpc_method_returns_error() {
    let mut host = create_host();
    let result = host.handle("no/such_method", &serde_json::json!({}));
    assert!(result.is_err(), "unknown method should error");
    assert!(
        result.unwrap_err().to_string().contains("unknown method"),
        "error should mention unknown method"
    );
}

#[test]
fn missing_required_params_returns_error() {
    let mut host = create_host();

    // hub/create_project without name
    let result = host.handle("hub/create_project", &serde_json::json!({}));
    assert!(result.is_err(), "missing name should error");
}

#[test]
fn shell_get_state_before_project_open() {
    let mut host = create_host();
    let state = host
        .handle("shell/get_state", &serde_json::json!({}))
        .expect("get shell state");
    assert!(!state["has_project"].as_bool().unwrap(), "no project open");
    assert_eq!(
        state["project_name"],
        serde_json::Value::Null,
        "no project name"
    );
    assert!(!state["can_undo"].as_bool().unwrap());
    assert!(!state["can_redo"].as_bool().unwrap());
}

#[test]
fn hub_get_desktop_integration() {
    let mut host = create_host();
    let di = host
        .handle("app/get_desktop_integration", &serde_json::json!({}))
        .expect("get desktop integration");
    assert!(di["desktop_environment"].is_string());
    assert!(di["prefers_native_chrome"].is_boolean());
    assert!(di["window_background"].is_string());
}

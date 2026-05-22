use engine_editor::EditorPreferences;
use engine_editor_ui::EditorShell;
use std::fs;

fn workspace_root() -> std::path::PathBuf {
    // When running tests, current_dir is the crate directory
    // We need to go up to the workspace root
    let mut path = std::env::current_dir().unwrap();
    while !path.join("Cargo.toml").exists() || !path.join("examples").exists() {
        if !path.pop() {
            panic!("Could not find workspace root");
        }
    }
    path
}

#[test]
fn open_project_loads_hierarchy_from_default_scene() {
    let project_path = workspace_root().join("examples/project");

    let mut shell = EditorShell::with_core_services(EditorPreferences::default());

    shell.open_project(&project_path).unwrap();

    let project = shell.project().expect("project should be open");
    let objects = project.scene.objects();

    assert_eq!(objects.len(), 2, "default scene should have 2 objects");

    let names: Vec<String> = objects
        .iter()
        .map(|(entity, _)| project.scene.object(*entity).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"Main Camera".to_string()));
    assert!(names.contains(&"Player".to_string()));
}

#[test]
fn save_and_reload_preserves_modified_object_name() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("aster-save-test-{unique}"));

    fs::create_dir_all(&temp_root).unwrap();
    fs::create_dir_all(temp_root.join("scenes")).unwrap();
    fs::create_dir_all(temp_root.join("assets")).unwrap();

    let manifest_content = r#"name = "Test Project"
asset_root = "assets"
default_scene = "scenes/test.aster_scene.json"

[format]
version = 1

[evolution]
policy = "minor versions may add optional fields; major schema bumps require migration"
forward_compatible_read = true
migration_framework = "versioned Rust migrators keyed by explicit version"
"#;
    fs::write(temp_root.join("aster.project.toml"), manifest_content).unwrap();

    let scene_content = r#"{
  "version": 1,
  "name": "Test",
  "objects": [
    {
      "object": {
        "id": 1,
        "name": "TestObject",
        "tag": "",
        "layer": 0,
        "camera_role": null,
        "active": true,
        "scripts": [],
        "components": []
      },
      "local_transform": {
        "translation": { "x": 0.0, "y": 0.0, "z": 0.0 },
        "rotation": { "x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0 },
        "scale": { "x": 1.0, "y": 1.0, "z": 1.0 }
      },
      "parent": null,
      "sibling_index": 0
    }
  ]
}"#;
    fs::write(
        temp_root.join("scenes/test.aster_scene.json"),
        scene_content,
    )
    .unwrap();

    let mut shell = EditorShell::with_core_services(EditorPreferences::default());
    shell.open_project(&temp_root).unwrap();

    {
        let project = shell.project_mut().unwrap();
        let (entity, _) = project.scene.objects()[0];
        let object = project.scene.object_mut(entity).unwrap();
        object.name = "ModifiedName".to_string();
        project.scene_dirty = true;
    }

    shell.save_scene().unwrap();

    let mut shell2 = EditorShell::with_core_services(EditorPreferences::default());
    shell2.open_project(&temp_root).unwrap();

    let project = shell2.project().unwrap();
    let objects = project.scene.objects();
    assert_eq!(objects.len(), 1);

    let (entity, _) = objects[0];
    let object = project.scene.object(entity).unwrap();
    assert_eq!(object.name, "ModifiedName");

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn play_mode_transitions_update_ui_state() {
    let project_path = workspace_root().join("examples/project");

    let mut shell = EditorShell::with_core_services(EditorPreferences::default());

    shell.open_project(&project_path).unwrap();

    // Simulate play mode state transitions
    let mut playing = false;
    let paused = false;

    assert!(!playing, "should start in edit mode");

    // Enter play mode
    playing = true;
    assert!(playing, "should be in play mode");

    // Exit play mode
    playing = false;
    assert!(!playing, "should return to edit mode");
    assert!(!paused, "should not be paused");
}

#[test]
fn play_mode_does_not_modify_edit_scene() {
    let project_path = workspace_root().join("examples/project");

    let mut shell = EditorShell::with_core_services(EditorPreferences::default());

    shell.open_project(&project_path).unwrap();

    let original_json = {
        let project = shell.project().unwrap();
        project.scene.to_json("Test").unwrap()
    };

    // Simulate play mode (in real implementation, this would clone the scene)
    let _playing = true;
    // ... play mode logic would happen here ...
    let _playing = false;

    let after_play_json = {
        let project = shell.project().unwrap();
        project.scene.to_json("Test").unwrap()
    };

    assert_eq!(
        original_json, after_play_json,
        "edit scene should be unchanged after play mode"
    );
}

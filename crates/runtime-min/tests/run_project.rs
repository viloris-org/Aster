//! Integration test for run_project command.

use runtime_min::{RuntimeServices, load_runtime_project};
use std::path::PathBuf;
use std::time::Duration;

/// Verifies that load_runtime_project can load the example project.
#[test]
fn load_runtime_project_loads_example() {
    let workspace_root = find_workspace_root();
    let project_path = workspace_root.join("examples/project/fps_arena");

    let project = load_runtime_project(&project_path).expect("load example project");

    assert_eq!(project.manifest.name, "FPS Arena");
    assert_eq!(project.manifest.asset_root, "assets");
    assert_eq!(project.manifest.default_scene, "scenes/fps_arena.vscene");
    assert!(
        !project.scene.objects().is_empty(),
        "scene should have objects"
    );
}

/// Verifies that nested example games can be loaded as independent projects.
#[test]
fn load_runtime_project_loads_jump_jump_example() {
    let workspace_root = find_workspace_root();
    let project_path = workspace_root.join("examples/project/jump_jump");

    let project = load_runtime_project(&project_path).expect("load jump jump project");

    assert_eq!(project.manifest.name, "Jump Jump");
    assert_eq!(project.manifest.asset_root, "assets");
    assert_eq!(project.manifest.default_scene, "scenes/jump_jump.vscene");
    assert!(
        !project.scene.objects().is_empty(),
        "scene should have objects"
    );
}

/// Verifies that the VargCraft capability probe loads as a normal runtime project.
#[test]
fn load_runtime_project_loads_vargcraft_prototype() {
    let workspace_root = find_workspace_root();
    let project_path = workspace_root.join("examples/project/vargcraft_prototype");

    let project = load_runtime_project(&project_path).expect("load vargcraft prototype");
    let world = runtime_min::extract_render_world(&project.scene);
    let visibility = engine_render::select_visibility(&world, 16.0 / 9.0);

    assert_eq!(project.manifest.name, "VargCraft Prototype");
    assert_eq!(project.manifest.asset_root, "assets");
    assert_eq!(
        project.manifest.default_scene,
        "scenes/vargcraft_prototype.vscene"
    );
    assert!(project.scene.find_by_name("Player").is_some());
    assert!(world.camera.is_some(), "prototype should extract a camera");
    assert!(
        !visibility.visible_indices.is_empty(),
        "prototype camera should see at least one renderable object before runtime block spawning"
    );
}

/// Verifies that the VargCraft prototype's runtime script creates editable block entities.
#[test]
fn vargcraft_prototype_spawns_blocks_on_first_frame() {
    let workspace_root = find_workspace_root();
    let project_path = workspace_root.join("examples/project/vargcraft_prototype");

    let project = load_runtime_project(&project_path).expect("load vargcraft prototype");
    let mut services = RuntimeServices::minimal(Default::default());
    services.set_project_root(&project_path);
    services.scene = project.scene;

    services
        .tick_game_frame(Duration::from_millis(16), false)
        .expect("tick first vargcraft frame");

    let block_count = services
        .scene
        .objects()
        .into_iter()
        .filter(|(_, object)| {
            object.tag == "GrassBlock" || object.tag == "DirtBlock" || object.tag == "StoneBlock"
        })
        .count();

    assert!(
        block_count > 40,
        "prototype should spawn enough block entities to stress runtime paths, got {block_count}; diagnostics={:?}",
        services.diagnostics
    );
}

/// Verifies that the example project's game camera can see runtime geometry.
#[test]
fn load_runtime_project_example_camera_sees_scene() {
    let workspace_root = find_workspace_root();
    let project_path = workspace_root.join("examples/project/fps_arena");

    let project = load_runtime_project(&project_path).expect("load example project");
    let world = runtime_min::extract_render_world(&project.scene);
    let visibility = engine_render::select_visibility(&world, 16.0 / 9.0);

    assert!(world.camera.is_some(), "scene should extract a game camera");
    assert!(
        !world.objects.is_empty(),
        "scene should extract renderable objects"
    );
    assert!(
        !visibility.visible_indices.is_empty(),
        "game camera should see at least one renderable object"
    );
}

/// Verifies that load_runtime_project returns an error for non-existent projects.
#[test]
fn load_runtime_project_fails_for_missing_project() {
    let result = load_runtime_project("/nonexistent/project");
    assert!(result.is_err(), "should fail for missing project");
}

/// Verifies that load_runtime_project returns an error for invalid manifest.
#[test]
fn load_runtime_project_fails_for_invalid_manifest() {
    let temp_dir = std::env::temp_dir().join("varg_test_invalid_manifest");
    let _ = std::fs::create_dir_all(&temp_dir);
    let manifest_path = temp_dir.join("Varg.toml");
    std::fs::write(&manifest_path, "invalid toml content {{{").unwrap();

    let result = load_runtime_project(&temp_dir);
    assert!(result.is_err(), "should fail for invalid manifest");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn find_workspace_root() -> PathBuf {
    let mut current = std::env::current_dir().expect("get current dir");
    loop {
        if current.join("Cargo.toml").is_file()
            && current
                .join("examples/project/fps_arena/Varg.toml")
                .is_file()
        {
            return current;
        }
        if !current.pop() {
            panic!("could not find workspace root");
        }
    }
}

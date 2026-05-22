use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("examples").exists())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn build_command_creates_output_directory() {
    let workspace = workspace_root();
    let example_project = workspace.join("examples/project");
    let output_dir = workspace.join("target/test-build-output");

    // Clean up any previous test output
    let _ = std::fs::remove_dir_all(&output_dir);

    // Run the build command
    let status = Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg("aster")
        .arg("--")
        .arg("build")
        .arg(example_project.to_str().unwrap())
        .arg("--output")
        .arg(output_dir.to_str().unwrap())
        .current_dir(&workspace)
        .status()
        .expect("failed to execute build command");

    assert!(status.success(), "build command should succeed");

    // Verify output directory structure
    assert!(output_dir.exists(), "output directory should exist");
    assert!(
        output_dir.join("bin").exists(),
        "bin directory should exist"
    );
    assert!(
        output_dir.join("scenes").exists(),
        "scenes directory should exist"
    );

    // Verify binary was copied
    let binary_name = if cfg!(target_os = "windows") {
        "aster.exe"
    } else {
        "aster"
    };
    assert!(
        output_dir.join("bin").join(binary_name).exists(),
        "runtime binary should exist"
    );

    // Verify build_info.json was created
    let build_info_path = output_dir.join("build_info.json");
    assert!(build_info_path.exists(), "build_info.json should exist");

    let build_info_content = std::fs::read_to_string(&build_info_path).unwrap();
    let build_info: serde_json::Value = serde_json::from_str(&build_info_content).unwrap();

    assert!(build_info["timestamp"].is_number());
    assert_eq!(build_info["target"], "native");
    assert_eq!(build_info["release"], false);
    assert!(build_info["engine_version"].is_string());

    // Verify assets_manifest.json was created
    assert!(
        output_dir.join("assets_manifest.json").exists(),
        "assets_manifest.json should exist"
    );

    // Verify scene was copied
    assert!(
        output_dir
            .join("scenes")
            .join("example.aster_scene.json")
            .exists(),
        "scene file should be copied"
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&output_dir);
}

#[test]
fn build_command_fails_for_nonexistent_project() {
    let workspace = workspace_root();

    let output = Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg("aster")
        .arg("--")
        .arg("build")
        .arg("/nonexistent/project")
        .current_dir(&workspace)
        .output()
        .expect("failed to execute build command");

    assert!(
        !output.status.success(),
        "build command should fail for nonexistent project"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not exist") || stderr.contains("not a directory"),
        "error message should mention missing directory"
    );
}

#[test]
fn build_command_validates_build_configuration() {
    let workspace = workspace_root();
    let temp_dir = workspace.join("target/test-invalid-config");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create a minimal project with invalid build config
    let project_dir = temp_dir.join("test-project");
    std::fs::create_dir_all(&project_dir).unwrap();

    // Write a valid manifest
    let manifest_content = r#"
name = "Test Project"
default_scene = "scenes/main.json"

[format]
version = 1

[evolution]
policy = "none"
forward_compatible_read = true
migration_framework = "none"
"#;
    std::fs::write(project_dir.join("aster.project.toml"), manifest_content).unwrap();

    // Write an invalid build config (empty target)
    let build_config_content = r#"
target = ""
release = false
features = []

[format]
version = 1

[evolution]
policy = "none"
forward_compatible_read = true
migration_framework = "none"
"#;
    std::fs::write(
        project_dir.join("build.runtime-min.toml"),
        build_config_content,
    )
    .unwrap();

    let output = Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg("aster")
        .arg("--")
        .arg("build")
        .arg(project_dir.to_str().unwrap())
        .current_dir(&workspace)
        .output()
        .expect("failed to execute build command");

    assert!(
        !output.status.success(),
        "build command should fail for invalid config"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("validation") || stderr.contains("cannot be empty"),
        "error message should mention validation failure"
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}

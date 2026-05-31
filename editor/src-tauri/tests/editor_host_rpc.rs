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

#[test]
fn host_initializes_with_empty_hub() {
    let mut host = create_host();
    let state = host
        .handle("hub/get_state", &serde_json::json!({}))
        .expect("get state");
    assert_eq!(state["page"], "projects", "starts on projects page");
    assert_eq!(state["theme"], "system", "default theme is system");
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

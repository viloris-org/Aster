mod tests {
    use std::collections::HashMap;

    use engine_core::math::{Transform, Vec3};
    use engine_ecs::ComponentData;

    use crate::*;

    #[test]
    fn accepts_valid_script_lifecycle() {
        let diagnostics = diagnose_source(
            "scripts/player.varg",
            r#"script PlayerController {
    @export var speed: Float = 6.0

    func update(_ dt: Float) {
        log("tick")
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn rejects_invalid_update_signature() {
        let diagnostics = diagnose_source(
            "scripts/player.varg",
            r#"script PlayerController {
    func update() {
    }
}
"#,
        );

        assert_eq!(diagnostics[0].code, "VARG3001");
    }

    #[test]
    fn rejects_scene_loops() {
        let diagnostics = diagnose_source(
            "scenes/main.vscene",
            r#"scene MainScene {
    for i in 0..<100 {
        spawnTree(i)
    }
}
"#,
        );

        assert_eq!(diagnostics[0].code, "VARG4001");
    }

    #[test]
    fn extracts_exported_properties() {
        let (ast, diagnostics) = parse_source(
            "scripts/player.varg",
            r#"script PlayerController {
    @export var jumpForce: Float = 8.0
}
"#,
        );

        assert!(diagnostics.is_empty());
        let ast = ast.unwrap();
        assert_eq!(ast.declarations[0].exports[0].name, "jumpForce");
    }

    #[test]
    fn compiles_vscene_to_native_scene_file() {
        let source = r##"scene Example {
    camera "Main Camera" {
        transform {
            position: Vec3(0, 1.5, -6)
        }

        perspective {
            fov: 60
            near: 0.01
            far: 1000
        }

        primary: true
    }

    entity "Player" {
        tag: "Player"

        transform {
            position: Vec3(0, 0, 0)
        }

        mesh: Box(size: Vec3(1, 1, 1))

        material {
            baseColor: Color("#7aa2ff")
            roughness: 0.7
        }

        rigidbody {
            mode: kinematic
        }

        collider {
            shape: box
            size: Vec3(1, 1, 1)
        }

        script PlayerController {
            source: "scripts/player_controller.varg"
            speed: 6.0
            jumpForce: 8.0
        }
    }
}
"##;

        let (file, diagnostics) =
            compile_vscene_source_to_scene_file("scenes/example.vscene", source);

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let file = file.unwrap();
        assert_eq!(file.name, "Example");
        assert_eq!(file.objects.len(), 2);
        assert_eq!(file.objects[1].object.name, "Player");
        let script = file.objects[1]
            .object
            .components
            .iter()
            .find_map(|component| match component {
                ComponentData::Script(script) => Some(script),
                _ => None,
            })
            .expect("player should have script component");
        assert_eq!(script.source, "scripts/player_controller.varg");
    }

    #[test]
    fn compiles_vscene_rotation_vec3_as_xyz_euler_axes() {
        let source = r##"scene CameraRig {
    camera "Main Camera" {
        transform {
            position: Vec3(9.5, 11.5, -14.0)
            rotation: Vec3(-42, 0, 0)
        }
    }
}
"##;

        let (file, diagnostics) =
            compile_vscene_source_to_scene_file("scenes/camera.vscene", source);

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let file = file.unwrap();
        let forward = file.objects[0]
            .local_transform
            .rotation
            .rotate(Vec3::new(0.0, 0.0, -1.0));
        assert!(
            forward.y < -0.6,
            "negative x rotation should pitch the camera down, got {forward:?}"
        );
        assert!(
            forward.x.abs() < 0.01,
            "x rotation should not yaw the camera sideways, got {forward:?}"
        );
    }

    #[test]
    fn compiles_declarative_scene_geometry_concepts_to_vscene_mesh_renderers() {
        let source = r##"scene GeometryMigration {
    entity BoxActor {
        mesh: Box(size: Vec3(1, 2, 3))
        material {
            builtin: "debug/red"
        }
    }

    entity SphereActor {
        geometry {
            type: sphere
        }
        material: "debug/blue"
    }

    entity PlaneActor {
        mesh: plane
    }

    entity ModelActor {
        geometry {
            path: "models/ship.gltf"
        }
    }
}
"##;

        let (file, diagnostics) =
            compile_vscene_source_to_scene_file("scenes/geometry.vscene", source);

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let file = file.unwrap();
        assert_eq!(file.objects.len(), 4);

        let mesh_for = |name: &str| {
            file.objects
                .iter()
                .find(|record| record.object.name == name)
                .and_then(|record| {
                    record
                        .object
                        .components
                        .iter()
                        .find_map(|component| match component {
                            ComponentData::MeshRenderer(mesh) => Some(mesh),
                            _ => None,
                        })
                })
                .expect("object should have mesh renderer")
        };

        let box_mesh = mesh_for("BoxActor");
        assert_eq!(box_mesh.builtin_mesh.as_deref(), Some("debug/cube"));
        assert_eq!(box_mesh.material.builtin.as_deref(), Some("debug/red"));
        assert_eq!(
            file.objects
                .iter()
                .find(|record| record.object.name == "BoxActor")
                .unwrap()
                .local_transform
                .scale,
            Vec3::new(1.0, 2.0, 3.0)
        );

        let sphere_mesh = mesh_for("SphereActor");
        assert_eq!(sphere_mesh.builtin_mesh.as_deref(), Some("debug/sphere"));
        assert_eq!(sphere_mesh.material.builtin.as_deref(), Some("debug/blue"));

        assert_eq!(
            mesh_for("PlaneActor").builtin_mesh.as_deref(),
            Some("debug/plane")
        );
        assert_eq!(
            mesh_for("ModelActor").builtin_mesh.as_deref(),
            Some("model:models/ship.gltf")
        );
    }

    #[test]
    fn vscene_primitive_size_scales_visual_mesh_without_doubling_explicit_colliders() {
        let source = r##"scene PrimitiveSize {
    entity Platform {
        mesh: Box(size: Vec3(3, 0.5, 2))
        collider {
            shape: box
            size: Vec3(3, 0.5, 2)
        }
    }

    entity Beacon {
        mesh: Cylinder(radius: 0.4, height: 2.5)
    }

    entity Crown {
        mesh: Cone(radius: 0.8, height: 1.2)
    }
}
"##;

        let (file, diagnostics) =
            compile_vscene_source_to_scene_file("scenes/primitive_size.vscene", source);

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let file = file.unwrap();
        let record = |name: &str| {
            file.objects
                .iter()
                .find(|record| record.object.name == name)
                .expect("object should compile")
        };

        let platform = record("Platform");
        assert_eq!(platform.local_transform.scale, Vec3::new(3.0, 0.5, 2.0));
        let collider = platform
            .object
            .components
            .iter()
            .find_map(|component| match component {
                ComponentData::Collider(collider) => Some(collider),
                _ => None,
            })
            .expect("platform should have collider");
        assert_eq!(collider.size, Vec3::ONE);

        assert_eq!(
            record("Beacon").local_transform.scale,
            Vec3::new(0.8, 2.5, 0.8)
        );
        assert_eq!(
            record("Crown").local_transform.scale,
            Vec3::new(1.6, 1.2, 1.6)
        );
        assert_eq!(
            record("Beacon")
                .object
                .components
                .iter()
                .find_map(|component| match component {
                    ComponentData::MeshRenderer(mesh) => mesh.builtin_mesh.as_deref(),
                    _ => None,
                }),
            Some("debug/cylinder")
        );
        assert_eq!(
            record("Crown")
                .object
                .components
                .iter()
                .find_map(|component| match component {
                    ComponentData::MeshRenderer(mesh) => mesh.builtin_mesh.as_deref(),
                    _ => None,
                }),
            Some("debug/cone")
        );
    }

    #[test]
    fn compiles_top_level_light_blocks_to_light_objects() {
        let source = r##"scene Lighting {
    light "Sun" {
        kind: directional
        intensity: 3.5
        color: Vec3(1.0, 0.94, 0.84)
        rotation: Vec3(-50, 35, 0)
    }
}
"##;

        let (file, diagnostics) =
            compile_vscene_source_to_scene_file("scenes/lighting.vscene", source);

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let file = file.unwrap();
        assert_eq!(file.objects.len(), 1);
        assert_eq!(file.objects[0].object.name, "Sun");
        assert_eq!(file.objects[0].object.tag, "Light");
        let light = file.objects[0]
            .object
            .components
            .iter()
            .find_map(|component| match component {
                ComponentData::Light(light) => Some(light),
                _ => None,
            })
            .expect("Sun should have light component");
        assert_eq!(light.kind, "directional");
        assert_eq!(light.intensity, 3.5);

        let serialized = serialize_scene_file_to_vscene(&file).unwrap();
        assert!(serialized.contains("light \"Sun\""));
        assert!(serialized.contains("kind: directional"));
    }

    #[test]
    fn rejects_scene_imports() {
        let diagnostics = diagnose_source("scenes/main.vscene", "import \"scripts/combat.varg\"\n");

        assert_eq!(diagnostics[0].code, "VARG1005");
    }

    #[test]
    fn rejects_non_varg_import_targets() {
        let diagnostics = diagnose_source("scripts/player.varg", "import \"scenes/main.vscene\"\n");

        assert_eq!(diagnostics[0].code, "VARG1006");
    }

    #[test]
    fn rejects_unclosed_blocks() {
        let diagnostics = diagnose_source("scripts/player.varg", "script Player {\n");

        assert_eq!(diagnostics[0].code, "VARG1004");
    }

    #[test]
    fn rejects_missing_declaration_name() {
        let diagnostics = diagnose_source("scripts/player.varg", "script {\n}\n");

        assert_eq!(diagnostics[0].code, "VARG1007");
    }

    #[test]
    fn rejects_malformed_export() {
        let diagnostics = diagnose_source(
            "scripts/player.varg",
            r#"script Player {
    @export var speed = 6.0
}
"#,
        );

        assert_eq!(diagnostics[0].code, "VARG3002");
    }

    #[test]
    fn rejects_unsupported_runtime_statement_with_source_location() {
        let diagnostics = diagnose_source(
            "scripts/player.varg",
            r#"script Player {
    func update(_ dt: Float) {
        emit("coin_collected")
    }
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code, "VARG4100");
        assert_eq!(diagnostic.line, Some(3));
        assert_eq!(diagnostic.column, Some(9));
        assert!(diagnostic.message.contains("emit"));
        assert!(diagnostic.expected.contains("MVP runtime"));
        assert!(diagnostic.suggestion.contains("not wired"));
        assert_eq!(
            diagnostic.source_line.as_deref(),
            Some(r#"        emit("coin_collected")"#)
        );
    }

    #[test]
    fn rejects_spec_api_that_runtime_does_not_execute() {
        let diagnostics = diagnose_source(
            "scripts/player.varg",
            r#"script Player {
    @export var jumpForce: Float = 8.0

    func update(_ dt: Float) {
        entity.velocity.y = jumpForce
    }
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code, "VARG4100");
        assert_eq!(diagnostic.line, Some(5));
        assert_eq!(diagnostic.column, Some(9));
        assert!(diagnostic.message.contains("entity.velocity"));
        assert!(diagnostic.suggestion.contains("position.y"));
    }

    #[test]
    fn compile_rejects_unsupported_runtime_statement() {
        let (script, diagnostics) = compile_script_source(
            "scripts/player.varg",
            r#"script Player {
    func update(_ dt: Float) {
        spawnEnemy()
    }
}
"#,
        );

        assert!(script.is_none());
        assert_eq!(diagnostics[0].code, "VARG4100");
        assert!(diagnostics[0].blocking);
        assert!(
            diagnostics[0]
                .suggestion
                .contains("supported MVP script API")
        );
    }

    #[test]
    fn compiled_script_exposes_metadata_for_exports_and_hooks() {
        let (script, diagnostics) = compile_script_source(
            "scripts/player.varg",
            r#"script Player {
    @export var speed: Float = 6.0

    func start() {
        log("ready")
    }

    func update(_ dt: Float) {
        entity.translate(Vec3(speed * dt, 0.0, 0.0))
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        assert!(script.has_hook("start"));
        assert!(script.has_hook("update"));
        assert!(!script.has_hook("lateUpdate"));
        assert_eq!(script.hook_names(), vec!["start", "update"]);

        let metadata = script.metadata();
        assert_eq!(metadata.name, "Player");
        assert_eq!(metadata.exports.len(), 1);
        assert_eq!(metadata.exports[0].name, "speed");
        assert_eq!(metadata.exports[0].type_name, "Float");
        assert_eq!(
            metadata
                .hooks
                .iter()
                .map(|hook| hook.name.as_str())
                .collect::<Vec<_>>(),
            vec!["start", "update"]
        );
    }

    #[test]
    fn script_api_registry_exposes_supported_runtime_modules() {
        let registry = varg_script_api_registry();
        let module_names = registry
            .iter()
            .map(|module| module.name)
            .collect::<Vec<_>>();

        assert!(module_names.contains(&"Input"));
        assert!(module_names.contains(&"scene"));
        assert!(module_names.contains(&"Audio"));
        assert!(module_names.contains(&"render"));
        assert!(module_names.contains(&"ui"));

        let input = registry
            .iter()
            .find(|module| module.name == "Input")
            .unwrap();
        assert!(input.items.iter().any(|item| item.name == "Input.value"));
        assert!(
            input
                .items
                .iter()
                .any(|item| item.name == "Input.captureMouse")
        );

        let scene = registry
            .iter()
            .find(|module| module.name == "scene")
            .unwrap();
        assert!(
            scene
                .items
                .iter()
                .any(|item| item.name == "scene.spawnSphere")
        );

        let ui = registry.iter().find(|module| module.name == "ui").unwrap();
        assert!(ui.items.iter().any(|item| item.name == "ui.button"));
        assert!(ui.items.iter().any(|item| item.name == "ui.texture"));
        assert!(ui.items.iter().any(|item| item.name == "ui.screenWidth"));
        assert!(ui.items.iter().any(|item| item.name == "ui.screenHeight"));
    }

    #[test]
    fn rejects_unsupported_condition_calls() {
        let diagnostics = diagnose_source(
            "scripts/player.varg",
            r#"script Player {
    func update(_ dt: Float) {
        if target.has(Health) {
            log("hit")
        }
    }
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "VARG4100");
        assert!(diagnostics[0].message.contains("if"));
    }

    #[test]
    fn runtime_supports_else_and_comparisons() {
        let (script, diagnostics) = compile_script_source(
            "scripts/health.varg",
            r#"script Health {
    var hp: Int = 2

    func update(_ dt: Float) {
        if state.hp <= 0 {
            state.dead = 1
        } else {
            state.dead = 0
            state.hp -= 2
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let context = VargRuntimeContext {
            transform: Transform::default(),
            input: engine_platform::InputState::default(),
            delta_time: 0.016,
            total_time: 0.016,
            frame_index: 1,
            screen_size: (800.0, 600.0),
            exported_values: HashMap::new(),
            state: HashMap::new(),
            scene: VargSceneContext::default(),
        };
        let output = script.run_hook("update", context);
        assert_eq!(
            output.state.get("dead").and_then(|value| value.as_f64()),
            Some(0.0)
        );
        assert_eq!(
            output.state.get("hp").and_then(|value| value.as_f64()),
            Some(0.0)
        );

        let context = VargRuntimeContext {
            transform: Transform::default(),
            input: engine_platform::InputState::default(),
            delta_time: 0.016,
            total_time: 0.032,
            frame_index: 2,
            screen_size: (800.0, 600.0),
            exported_values: HashMap::new(),
            state: output.state,
            scene: VargSceneContext::default(),
        };
        let output = script.run_hook("update", context);
        assert_eq!(
            output.state.get("dead").and_then(|value| value.as_f64()),
            Some(1.0)
        );
    }

    #[test]
    fn runtime_emits_ui_draw_commands() {
        let (script, diagnostics) = compile_script_source(
            "scripts/hud.varg",
            r#"script Hud {
    var score: Int = 10

    func update(_ dt: Float) {
        ui.rect("health_bg", 12.0, 16.0, 120.0, 10.0, 0.1, 0.1, 0.1, 0.8)
        ui.label("score", "Score: " + score, 12.0, 32.0)
        ui.label("math", 1 + 2, 12.0, 48.0)
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let output = script.unwrap().run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 1.0,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(
            output.ui_commands,
            vec![
                VargUiCommand::Rect {
                    id: "health_bg".to_string(),
                    x: 12.0,
                    y: 16.0,
                    width: 120.0,
                    height: 10.0,
                    color: [0.1, 0.1, 0.1, 0.8],
                },
                VargUiCommand::Label {
                    id: "score".to_string(),
                    text: "Score: 10".to_string(),
                    x: 12.0,
                    y: 32.0,
                },
                VargUiCommand::Label {
                    id: "math".to_string(),
                    text: "3".to_string(),
                    x: 12.0,
                    y: 48.0,
                },
            ]
        );
    }

    #[test]
    fn runtime_ui_can_read_screen_size_and_emit_textures() {
        let (script, diagnostics) = compile_script_source(
            "scripts/responsive_hud.varg",
            r#"script ResponsiveHud {
    func update(_ dt: Float) {
        let cx: Float = ui.screenWidth() * 0.5
        let y: Float = ui.screenHeight() - 32.0
        ui.texture("hotbar", "vargcraft:hotbar", cx - 91.0, y, 182.0, 22.0)
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let output = script.unwrap().run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 1.0,
                frame_index: 1,
                screen_size: (1280.0, 720.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(
            output.ui_commands,
            vec![VargUiCommand::Texture {
                id: "hotbar".to_string(),
                texture: "vargcraft:hotbar".to_string(),
                x: 549.0,
                y: 688.0,
                width: 182.0,
                height: 22.0,
                color: [1.0, 1.0, 1.0, 1.0],
            }]
        );
    }

    #[test]
    fn runtime_emits_procedural_audio_commands() {
        let (script, diagnostics) = compile_script_source(
            "scripts/sfx.varg",
            r#"script Sfx {
    func update(_ dt: Float) {
        Audio.playTone("square", 880.0, 0.08, 0.35)
        Audio.playTone3D("noise", 220.0, 0.05, 0.2)
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let output = script.unwrap().run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform {
                    translation: Vec3::new(1.0, 2.0, 3.0),
                    ..Transform::IDENTITY
                },
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 1.0,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(
            output.audio_commands,
            vec![
                VargAudioCommand::PlayTone {
                    waveform: "square".to_string(),
                    frequency_hz: 880.0,
                    duration_seconds: 0.08,
                    volume: 0.35,
                    spatial: false,
                    position: Vec3::new(1.0, 2.0, 3.0),
                },
                VargAudioCommand::PlayTone {
                    waveform: "noise".to_string(),
                    frequency_hz: 220.0,
                    duration_seconds: 0.05,
                    volume: 0.2,
                    spatial: true,
                    position: Vec3::new(1.0, 2.0, 3.0),
                },
            ]
        );
    }

    #[test]
    fn runtime_emits_procedural_audio_loop_commands() {
        let (script, diagnostics) = compile_script_source(
            "scripts/bgm.varg",
            r#"script Bgm {
    func start() {
        Audio.startLoop("main", "triangle", "C4 E4 G4 R", 120.0, 0.5, 0.18)
        Audio.stopLoop("old")
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let output = script.unwrap().run_hook(
            "start",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 1.0,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(
            output.audio_commands,
            vec![
                VargAudioCommand::StartLoop {
                    id: "main".to_string(),
                    waveform: "triangle".to_string(),
                    pattern: "C4 E4 G4 R".to_string(),
                    bpm: 120.0,
                    beats_per_note: 0.5,
                    volume: 0.18,
                },
                VargAudioCommand::StopLoop {
                    id: "old".to_string(),
                },
            ]
        );
    }

    #[test]
    fn runtime_supports_locals_boolean_conditions_and_position_assignment() {
        let (script, diagnostics) = compile_script_source(
            "scripts/movement.varg",
            r#"script Movement {
    @export var speed: Float = 3.0
    var ticks: Int = 0

    func update(_ dt: Float) {
        let moveX: Float = Input.actionValue("MoveX")
        let distance: Float = moveX * speed

        if Input.down("moveRight") && !Input.down("jump") {
            position.x = distance
        }

        if state.ticks == 0 || position.x >= 3.0 {
            state.ready = 1
        }

        ticks += 1
        position = Vec3(position.x, 2.0, 0.0)
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let mut input = engine_platform::InputState::default();
        input.bind_default_player_actions();
        input.apply_event(engine_platform::InputEvent::KeyDown(
            engine_platform::KeyCode::Character('d'),
        ));
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input,
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(output.transform.translation.x, 3.0);
        assert_eq!(output.transform.translation.y, 2.0);
        assert_eq!(
            output.state.get("ready").and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            output.state.get("ticks").and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert!(!output.state.contains_key("moveX"));
        assert!(!output.state.contains_key("distance"));
    }

    #[test]
    fn runtime_supports_action_pressed_aliases() {
        let (script, diagnostics) = compile_script_source(
            "scripts/input.varg",
            r#"script InputProbe {
    func update(_ dt: Float) {
        if Input.actionPressed("Fire") {
            state.fired = 1
        }

        if Input.actionReleased("Fire") {
            state.released = 1
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let mut input = engine_platform::InputState::default();
        input.apply_event(engine_platform::InputEvent::KeyDown(
            engine_platform::KeyCode::Character('f'),
        ));
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input,
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );
        assert_eq!(
            output.state.get("fired").and_then(|value| value.as_f64()),
            Some(1.0)
        );

        let mut input = engine_platform::InputState::default();
        input.apply_event(engine_platform::InputEvent::KeyDown(
            engine_platform::KeyCode::Character('f'),
        ));
        input.end_frame();
        input.apply_event(engine_platform::InputEvent::KeyUp(
            engine_platform::KeyCode::Character('f'),
        ));
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input,
                delta_time: 0.016,
                total_time: 0.032,
                frame_index: 2,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: output.state,
                scene: VargSceneContext::default(),
            },
        );
        assert_eq!(
            output
                .state
                .get("released")
                .and_then(|value| value.as_f64()),
            Some(1.0)
        );
    }

    #[test]
    fn runtime_supports_preferred_explicit_input_and_bool_state() {
        let (script, diagnostics) = compile_script_source(
            "scripts/preferred_input.varg",
            r#"script PreferredInput {
    var canFire: Bool = true
    var fired: Int = 0
    var released: Int = 0

    func update(_ dt: Float) {
        let moveX: Float = Input.value("MoveX")
        position.x = moveX

        if Input.pressed("Fire") && canFire {
            fired += 1
            canFire = false
        }

        if Input.released("Fire") {
            released += 1
            canFire = true
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let mut input = engine_platform::InputState::default();
        input.apply_event(engine_platform::InputEvent::KeyDown(
            engine_platform::KeyCode::Character('f'),
        ));
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input,
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );
        assert_eq!(
            output.state.get("fired").and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            output
                .state
                .get("canFire")
                .and_then(|value| value.as_bool()),
            Some(false)
        );

        let mut input = engine_platform::InputState::default();
        input.apply_event(engine_platform::InputEvent::KeyDown(
            engine_platform::KeyCode::Character('f'),
        ));
        input.end_frame();
        input.apply_event(engine_platform::InputEvent::KeyUp(
            engine_platform::KeyCode::Character('f'),
        ));
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: output.transform,
                input,
                delta_time: 0.016,
                total_time: 0.032,
                frame_index: 2,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: output.state,
                scene: VargSceneContext::default(),
            },
        );
        assert_eq!(
            output
                .state
                .get("released")
                .and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            output
                .state
                .get("canFire")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn runtime_supports_zero_arg_script_function_calls() {
        let (script, diagnostics) = compile_script_source(
            "scripts/function_calls.varg",
            r#"script FunctionCalls {
    var count: Int = 0

    func update(_ dt: Float) {
        increment()
        increment()
    }

    func increment() {
        count += 1
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(
            output.state.get("count").and_then(|value| value.as_f64()),
            Some(2.0)
        );
    }

    #[test]
    fn runtime_w_key_moves_first_person_script_forward() {
        let (script, diagnostics) = compile_script_source(
            "scripts/fps_move_probe.varg",
            r#"script FpsMoveProbe {
    @export var moveSpeed: Float = 4.8

    func update(_ dt: Float) {
        let yawRad: Float = 0.0
        let forwardX: Float = 0.0 - sin(yawRad)
        let forwardZ: Float = cos(yawRad)
        let rightX: Float = cos(yawRad)
        let rightZ: Float = sin(yawRad)
        let moveX: Float = Input.value("MoveX")
        let moveZ: Float = Input.value("MoveY")
        let strafeX: Float = rightX * moveX
        let forwardMoveX: Float = forwardX * moveZ
        let strafeZ: Float = rightZ * moveX
        let forwardMoveZ: Float = forwardZ * moveZ
        let deltaX: Float = strafeX + forwardMoveX
        let deltaZ: Float = strafeZ + forwardMoveZ
        position.x += deltaX * moveSpeed * dt
        position.z += deltaZ * moveSpeed * dt
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let mut input = engine_platform::InputState::default();
        input.bind_default_player_actions();
        input.apply_event(engine_platform::InputEvent::KeyDown(
            engine_platform::KeyCode::Character('w'),
        ));
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input,
                delta_time: 1.0,
                total_time: 1.0,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert!(output.transform.translation.z > 4.7);
        assert!(output.transform.translation.x.abs() < 0.001);
    }

    #[test]
    fn runtime_emits_spawn_requests() {
        let (script, diagnostics) = compile_script_source(
            "scripts/spawner.varg",
            r#"script Spawner {
    func update(_ dt: Float) {
        scene.spawnBox("Step", "Platform", Vec3(3.0, 0.0, 8.0), Vec3(2.0, 0.5, 2.0), "")
        scene.spawnSphere("Gem", "Collectible", Vec3(3.0, 1.1, 8.0), 0.35, "scripts/bobber.varg")
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(output.spawn_requests.len(), 2);
        assert_eq!(output.spawn_requests[0].name, "Step");
        assert_eq!(output.spawn_requests[0].tag, "Platform");
        assert_eq!(output.spawn_requests[0].builtin_mesh, "debug/cube");
        assert_eq!(output.spawn_requests[0].collider_shape, "box");
        assert_eq!(output.spawn_requests[0].position, Vec3::new(3.0, 0.0, 8.0));
        assert_eq!(output.spawn_requests[0].size, Vec3::new(2.0, 0.5, 2.0));
        assert_eq!(output.spawn_requests[0].script, None);
        assert_eq!(output.spawn_requests[1].name, "Gem");
        assert_eq!(output.spawn_requests[1].builtin_mesh, "debug/sphere");
        assert_eq!(output.spawn_requests[1].collider_shape, "sphere");
        assert_eq!(output.spawn_requests[1].size, Vec3::new(0.7, 0.7, 0.7));
        assert_eq!(
            output.spawn_requests[1].script.as_deref(),
            Some("scripts/bobber.varg")
        );
    }

    #[test]
    fn runtime_can_query_tag_bounds_distance() {
        let (script, diagnostics) = compile_script_source(
            "scripts/landing.varg",
            r#"script Landing {
    func update(_ dt: Float) {
        state.centerDistance = scene.distanceToTag("Platform")
        state.boundsDistance = scene.distanceToTagBounds("Platform")
        state.footprintDistance = scene.horizontalDistanceToTagBounds("Platform")
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let mut scene = VargSceneContext::default();
        scene
            .positions_by_tag
            .insert("Platform".to_string(), vec![Vec3::ZERO]);
        scene.bounds_by_tag.insert(
            "Platform".to_string(),
            vec![VargSceneBounds::from_center_size(
                Vec3::ZERO,
                Vec3::new(2.0, 0.5, 2.0),
            )],
        );

        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform {
                    translation: Vec3::new(2.4, 1.1, 0.0),
                    ..Transform::default()
                },
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene,
            },
        );

        let center_distance = output
            .state
            .get("centerDistance")
            .and_then(|value| value.as_f64())
            .unwrap();
        assert!(
            (center_distance - 2.640076).abs() < 0.001,
            "center distance should be spherical distance to object origin: {center_distance}"
        );
        let bounds_distance = output
            .state
            .get("boundsDistance")
            .and_then(|value| value.as_f64())
            .unwrap();
        let footprint_distance = output
            .state
            .get("footprintDistance")
            .and_then(|value| value.as_f64())
            .unwrap();
        assert!(
            bounds_distance > 1.5,
            "3D bounds distance should include height separation: {bounds_distance}"
        );
        assert!(
            (footprint_distance - 1.4).abs() < 0.001,
            "horizontal bounds distance should measure platform edge miss: {footprint_distance}"
        );
    }

    #[test]
    fn runtime_emits_render_gi_commands() {
        let (script, diagnostics) = compile_script_source(
            "scripts/lighting.varg",
            r#"script Lighting {
    func update(_ dt: Float) {
        render.gi.useScreenSpace()
        render.gi.useProbeVolume(Vec3(1.0, 2.0, 3.0), Vec3(20.0, 8.0, 20.0), Vec3(4.0, 3.0, 2.0), 1.75)
        render.gi.setIntensity(0.5)
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(output.render_commands.len(), 3);
        assert_eq!(
            output.render_commands[0],
            VargRenderCommand::UseScreenSpaceGi
        );
        assert_eq!(
            output.render_commands[1],
            VargRenderCommand::UseProbeVolumeGi {
                center: Vec3::new(1.0, 2.0, 3.0),
                extent: Vec3::new(20.0, 8.0, 20.0),
                counts: Vec3::new(4.0, 3.0, 2.0),
                intensity: 1.75,
            }
        );
        assert_eq!(
            output.render_commands[2],
            VargRenderCommand::SetGiIntensity(0.5)
        );
    }

    #[test]
    fn runtime_emits_weather_commands() {
        let (script, diagnostics) = compile_script_source(
            "scripts/weather.varg",
            r#"script Weather {
    func update(_ dt: Float) {
        render.weather.set("storm")
        render.weather.setTimeOfDay(18.5)
        render.weather.setCloudCover(0.85)
        render.weather.setPrecipitation(0.7)
        render.weather.setWind(Vec3(3.0, 0.0, -1.0))
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(output.render_commands.len(), 5);
        assert_eq!(
            output.render_commands[0],
            VargRenderCommand::SetWeatherPreset("storm".to_string())
        );
        assert_eq!(
            output.render_commands[1],
            VargRenderCommand::SetWeatherTimeOfDay(18.5)
        );
        assert_eq!(
            output.render_commands[2],
            VargRenderCommand::SetWeatherCloudCover(0.85)
        );
        assert_eq!(
            output.render_commands[3],
            VargRenderCommand::SetWeatherPrecipitation(0.7)
        );
        assert_eq!(
            output.render_commands[4],
            VargRenderCommand::SetWeatherWind(Vec3::new(3.0, 0.0, -1.0))
        );
    }

    #[test]
    fn runtime_emits_destroy_nearest_requests() {
        let (script, diagnostics) = compile_script_source(
            "scripts/collector.varg",
            r#"script Collector {
    func update(_ dt: Float) {
        scene.destroyNearestWithTag("Collectible", 1.5)
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform {
                    translation: Vec3::new(2.0, 0.0, 3.0),
                    ..Transform::default()
                },
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(output.destroy_nearest_requests.len(), 1);
        assert_eq!(output.destroy_nearest_requests[0].tag, "Collectible");
        assert_eq!(output.destroy_nearest_requests[0].radius, 1.5);
        assert_eq!(
            output.destroy_nearest_requests[0].origin,
            Vec3::new(2.0, 0.0, 3.0)
        );
    }

    #[test]
    fn runtime_supports_migrated_declarative_entity_queries_and_destroy() {
        let (script, diagnostics) = compile_script_source(
            "scripts/hazard.varg",
            r#"script Hazard {
    func update(_ dt: Float) {
        if entity.hasTag("Enemy") && scene.distanceTo("Player") <= 5.0 {
            state.nearPlayer = 1
        }

        if playerDistance() <= 5.0 {
            state.playerDistanceMatched = 1
        }

        state.playerX = scene.xOf("Player")
        state.playerZ = scene.zOf("Player")

        if scene.distanceToTag("Treasure") < 3.0 {
            entity.destroy()
        }

        state.afterDestroy = 1
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let mut scene = VargSceneContext {
            entity_name: "EnemyA".to_string(),
            entity_tag: "Enemy".to_string(),
            ..VargSceneContext::default()
        };
        scene
            .positions_by_name
            .insert("Player".to_string(), Vec3::new(3.0, 0.0, 4.0));
        scene
            .positions_by_tag
            .insert("Player".to_string(), vec![Vec3::new(3.0, 0.0, 4.0)]);
        scene
            .positions_by_tag
            .insert("Treasure".to_string(), vec![Vec3::new(1.0, 0.0, 0.0)]);

        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene,
            },
        );

        assert_eq!(
            output
                .state
                .get("nearPlayer")
                .and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            output
                .state
                .get("playerDistanceMatched")
                .and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            output.state.get("playerX").and_then(|value| value.as_f64()),
            Some(3.0)
        );
        assert_eq!(
            output.state.get("playerZ").and_then(|value| value.as_f64()),
            Some(4.0)
        );
        assert!(output.destroy_self);
        assert!(!output.state.contains_key("afterDestroy"));
    }

    #[test]
    fn compiles_behavior_declaration_to_varg_behavior_ir() {
        let (behavior, diagnostics) = compile_behavior_source(
            "scripts/enemy_ai.varg",
            r#"behavior EnemyAI {
    selector {
        sequence "chase branch" {
            when playerDistance() < 10
            action chase("Player", speed: 4.0)
        }

        repeat 3 {
            action patrol(points: ["A", "B", "C"], speed: 2.0)
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let behavior = behavior.expect("behavior should compile");
        assert_eq!(behavior.name, "EnemyAI");
        let VargBehaviorNode::Selector { children, .. } = behavior.root else {
            panic!("expected selector root");
        };
        assert_eq!(children.len(), 2);
        match &children[0] {
            VargBehaviorNode::Sequence { name, children } => {
                assert_eq!(name.as_deref(), Some("chase branch"));
                assert_eq!(
                    children,
                    &vec![
                        VargBehaviorNode::Condition {
                            expression: "playerDistance() < 10".to_string()
                        },
                        VargBehaviorNode::Action {
                            expression: "chase(\"Player\", speed: 4.0)".to_string()
                        }
                    ]
                );
            }
            other => panic!("expected sequence, got {other:#?}"),
        }
        match &children[1] {
            VargBehaviorNode::Repeat { count, child } => {
                assert_eq!(*count, Some(3));
                assert_eq!(
                    **child,
                    VargBehaviorNode::Action {
                        expression: "patrol(points: [\"A\", \"B\", \"C\"], speed: 2.0)".to_string()
                    }
                );
            }
            other => panic!("expected repeat, got {other:#?}"),
        }
    }

    #[test]
    fn compiles_behavior_decorators() {
        let (behavior, diagnostics) = compile_behavior_source(
            "scripts/decorators.varg",
            r#"behavior Decorators {
    sequence {
        invert {
            when entity.hasTag("Frozen")
        }
        succeed {
            action idle()
        }
        repeat forever {
            action wait(1.0)
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let behavior = behavior.unwrap();
        let VargBehaviorNode::Sequence { children, .. } = behavior.root else {
            panic!("expected sequence root");
        };
        assert!(matches!(children[0], VargBehaviorNode::Invert { .. }));
        assert!(matches!(children[1], VargBehaviorNode::Succeed { .. }));
        assert!(matches!(
            children[2],
            VargBehaviorNode::Repeat { count: None, .. }
        ));
    }

    #[test]
    fn rejects_empty_behavior_declaration() {
        let (behavior, diagnostics) = compile_behavior_source(
            "scripts/empty.varg",
            r#"behavior Empty {
}
"#,
        );

        assert!(behavior.is_none());
        assert_eq!(diagnostics[0].code, "VARG5003");
    }

    #[test]
    fn rejects_decorator_with_multiple_children() {
        let (behavior, diagnostics) = compile_behavior_source(
            "scripts/bad.varg",
            r#"behavior Bad {
    invert {
        when entity.hasTag("Frozen")
        action idle()
    }
}
"#,
        );

        assert!(behavior.is_none());
        assert_eq!(diagnostics[0].code, "VARG5005");
    }

    #[test]
    fn checked_in_examples_compile() {
        for (path, source) in [
            (
                "examples/scripts/loop_demo.varg",
                include_str!("../../../examples/scripts/loop_demo.varg"),
            ),
            (
                "examples/scripts/particle_system.varg",
                include_str!("../../../examples/scripts/particle_system.varg"),
            ),
            (
                "examples/scripts/timed_sequence.varg",
                include_str!("../../../examples/scripts/timed_sequence.varg"),
            ),
            (
                "examples/scripts/wave_spawner.varg",
                include_str!("../../../examples/scripts/wave_spawner.varg"),
            ),
            (
                "examples/scripts/weapon_cooldown.varg",
                include_str!("../../../examples/scripts/weapon_cooldown.varg"),
            ),
            (
                "examples/project/fps_arena/scripts/fps_player.varg",
                include_str!("../../../examples/project/fps_arena/scripts/fps_player.varg"),
            ),
            (
                "examples/project/fps_arena/scripts/fps_camera.varg",
                include_str!("../../../examples/project/fps_arena/scripts/fps_camera.varg"),
            ),
            (
                "examples/project/fps_arena/scripts/target_drift.varg",
                include_str!("../../../examples/project/fps_arena/scripts/target_drift.varg"),
            ),
            (
                "examples/project/fps_arena/scripts/drone_part_drift.varg",
                include_str!("../../../examples/project/fps_arena/scripts/drone_part_drift.varg"),
            ),
            (
                "examples/project/jump_jump/scripts/jump_player.varg",
                include_str!("../../../examples/project/jump_jump/scripts/jump_player.varg"),
            ),
            (
                "examples/project/jump_jump/scripts/first_person_camera.varg",
                include_str!(
                    "../../../examples/project/jump_jump/scripts/first_person_camera.varg"
                ),
            ),
            (
                "examples/project/fps_arena/scripts/bobber.varg",
                include_str!("../../../examples/project/fps_arena/scripts/bobber.varg"),
            ),
            (
                "examples/project/fps_arena/scripts/despawn_far.varg",
                include_str!("../../../examples/project/fps_arena/scripts/despawn_far.varg"),
            ),
            (
                "examples/project/jump_jump/scripts/bobber.varg",
                include_str!("../../../examples/project/jump_jump/scripts/bobber.varg"),
            ),
            (
                "examples/project/jump_jump/scripts/despawn_far.varg",
                include_str!("../../../examples/project/jump_jump/scripts/despawn_far.varg"),
            ),
        ] {
            let (script, diagnostics) = compile_script_source(path, source);
            assert!(script.is_some(), "{path} did not compile: {diagnostics:#?}");
            assert!(
                diagnostics.is_empty(),
                "{path} diagnostics: {diagnostics:#?}"
            );
        }
    }

    #[test]
    fn runtime_supports_for_loops_with_range() {
        let (script, diagnostics) = compile_script_source(
            "scripts/counter.varg",
            r#"script Counter {
    var sum: Int = 0

    func update(_ dt: Float) {
        for i in 1..5 {
            state.sum += i
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        // 1 + 2 + 3 + 4 = 10
        assert_eq!(
            output.state.get("sum").and_then(|value| value.as_f64()),
            Some(10.0)
        );
    }

    #[test]
    fn runtime_supports_for_loops_with_inclusive_range() {
        let (script, diagnostics) = compile_script_source(
            "scripts/counter.varg",
            r#"script Counter {
    var sum: Int = 0

    func update(_ dt: Float) {
        for i in 1..=5 {
            state.sum += i
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        // 1 + 2 + 3 + 4 + 5 = 15
        assert_eq!(
            output.state.get("sum").and_then(|value| value.as_f64()),
            Some(15.0)
        );
    }

    #[test]
    fn runtime_supports_for_loops_with_count() {
        let (script, diagnostics) = compile_script_source(
            "scripts/spawner.varg",
            r#"script Spawner {
    var count: Int = 0

    func update(_ dt: Float) {
        for i in count(3) {
            state.count += 1
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(
            output.state.get("count").and_then(|value| value.as_f64()),
            Some(3.0)
        );
    }

    #[test]
    fn runtime_supports_while_loops() {
        let (script, diagnostics) = compile_script_source(
            "scripts/countdown.varg",
            r#"script Countdown {
    var counter: Int = 5

    func update(_ dt: Float) {
        while state.counter > 0 {
            state.counter -= 1
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(
            output.state.get("counter").and_then(|value| value.as_f64()),
            Some(0.0)
        );
    }

    #[test]
    fn runtime_supports_break_in_loops() {
        let (script, diagnostics) = compile_script_source(
            "scripts/breaker.varg",
            r#"script Breaker {
    var sum: Int = 0

    func update(_ dt: Float) {
        for i in 0..10 {
            if i >= 5 {
                break
            }
            state.sum += i
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        // 0 + 1 + 2 + 3 + 4 = 10
        assert_eq!(
            output.state.get("sum").and_then(|value| value.as_f64()),
            Some(10.0)
        );
    }

    #[test]
    fn runtime_supports_continue_in_loops() {
        let (script, diagnostics) = compile_script_source(
            "scripts/skipper.varg",
            r#"script Skipper {
    var sum: Int = 0

    func update(_ dt: Float) {
        for i in 0..10 {
            if i == 2 || i == 5 {
                continue
            }
            state.sum += i
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        // 0 + 1 + 3 + 4 + 6 + 7 + 8 + 9 = 38
        assert_eq!(
            output.state.get("sum").and_then(|value| value.as_f64()),
            Some(38.0)
        );
    }

    #[test]
    fn runtime_supports_return_early() {
        let (script, diagnostics) = compile_script_source(
            "scripts/early_exit.varg",
            r#"script EarlyExit {
    var executed: Int = 0

    func update(_ dt: Float) {
        state.executed = 1
        if state.executed == 1 {
            return
        }
        state.executed = 2
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(
            output
                .state
                .get("executed")
                .and_then(|value| value.as_f64()),
            Some(1.0)
        );
    }

    #[test]
    fn runtime_supports_nested_loops() {
        let (script, diagnostics) = compile_script_source(
            "scripts/nested.varg",
            r#"script Nested {
    var sum: Int = 0

    func update(_ dt: Float) {
        for i in 0..3 {
            for j in 0..2 {
                state.sum += i + j
            }
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        // (0+0) + (0+1) + (1+0) + (1+1) + (2+0) + (2+1) = 0 + 1 + 1 + 2 + 2 + 3 = 9
        assert_eq!(
            output.state.get("sum").and_then(|value| value.as_f64()),
            Some(9.0)
        );
    }

    #[test]
    fn runtime_supports_time_and_math_for_wave_motion() {
        let (script, diagnostics) = compile_script_source(
            "scripts/buoy.varg",
            r#"script Buoy {
    @export var amplitude: Float = 2.0
    @export var frequency: Float = 3.1415927

    func update(_ dt: Float) {
        let wave: Float = sin(Time.time * frequency) * amplitude
        let lift: Float = clamp(wave, -1.0, 1.0)
        position.y = lerp(position.y, lift, 1.0)
        state.frame = Time.frame
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.5,
                frame_index: 7,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert!((output.transform.translation.y - 1.0).abs() < 0.0001);
        assert_eq!(
            output.state.get("frame").and_then(|value| value.as_f64()),
            Some(7.0)
        );
    }

    #[test]
    fn runtime_supports_wait_for_simple_delays() {
        let (script, diagnostics) = compile_script_source(
            "scripts/delayed.varg",
            r#"script Delayed {
    func update(_ dt: Float) {
        if state.executed == 1 {
            return
        }
        wait(1.0)
        state.executed = 1
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();

        // First frame: wait starts, executed should not be set
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        // Wait timer should be created, but executed should not be 1
        assert!(output.state.get("__wait_timer").is_some());
        assert_ne!(
            output.state.get("executed").and_then(|v| v.as_f64()),
            Some(1.0)
        );

        // Simulate frames during wait (0.5 seconds passed)
        let mut state = output.state;
        for _ in 0..30 {
            let output = script.run_hook(
                "update",
                VargRuntimeContext {
                    transform: Transform::default(),
                    input: engine_platform::InputState::default(),
                    delta_time: 0.016,
                    total_time: 0.5,
                    frame_index: 30,
                    screen_size: (800.0, 600.0),
                    exported_values: HashMap::new(),
                    state: state.clone(),
                    scene: VargSceneContext::default(),
                },
            );
            state = output.state;
        }

        // Still waiting
        assert_ne!(
            state.get("executed").and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert!(state.get("__wait_timer").is_some());

        // Simulate more frames (total > 1.0 second)
        for _ in 0..40 {
            let output = script.run_hook(
                "update",
                VargRuntimeContext {
                    transform: Transform::default(),
                    input: engine_platform::InputState::default(),
                    delta_time: 0.016,
                    total_time: 1.2,
                    frame_index: 70,
                    screen_size: (800.0, 600.0),
                    exported_values: HashMap::new(),
                    state: state.clone(),
                    scene: VargSceneContext::default(),
                },
            );
            state = output.state;
        }

        // Wait finished, code after wait executed
        assert_eq!(
            state.get("executed").and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert!(state.get("__wait_timer").is_none());
    }

    #[test]
    fn runtime_supports_wait_with_expressions() {
        let (script, diagnostics) = compile_script_source(
            "scripts/dynamic_wait.varg",
            r#"script DynamicWait {
    @export var cooldown: Float = 0.5

    func update(_ dt: Float) {
        if state.fired == 1 {
            return
        }
        wait(cooldown)
        state.fired = 1
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();

        let mut exported = HashMap::new();
        exported.insert("cooldown".to_string(), serde_json::Value::from(0.5));

        // First frame
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: exported.clone(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        // Count should not be set yet
        assert_ne!(
            output.state.get("fired").and_then(|v| v.as_f64()),
            Some(1.0)
        );

        // Simulate 0.5 seconds of frames
        let mut state = output.state;
        for _ in 0..32 {
            let output = script.run_hook(
                "update",
                VargRuntimeContext {
                    transform: Transform::default(),
                    input: engine_platform::InputState::default(),
                    delta_time: 0.016,
                    total_time: 0.5,
                    frame_index: 32,
                    screen_size: (800.0, 600.0),
                    exported_values: exported.clone(),
                    state: state.clone(),
                    scene: VargSceneContext::default(),
                },
            );
            state = output.state;
        }

        // After 0.5 seconds, fired should be set
        assert_eq!(
            state.get("fired").and_then(|value| value.as_f64()),
            Some(1.0)
        );
    }

    #[test]
    fn runtime_scripts_can_capture_mouse_and_read_mouse_delta() {
        let (script, diagnostics) = compile_script_source(
            "scripts/input_capture.varg",
            r#"script InputCapture {
    var dx: Float = 0.0
    var dy: Float = 0.0

    func update(_ dt: Float) {
        Input.captureMouse(true)
        state.dx = Input.mouseDeltaX()
        state.dy = Input.mouseDeltaY
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let mut input = engine_platform::InputState::default();
        input.apply_event(engine_platform::InputEvent::MouseDelta { x: 12.5, y: -4.0 });

        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input,
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(output.mouse_capture, Some(true));
        assert_eq!(
            output.state.get("dx").and_then(|value| value.as_f64()),
            Some(12.5)
        );
        assert_eq!(
            output.state.get("dy").and_then(|value| value.as_f64()),
            Some(-4.0)
        );
    }

    #[test]
    fn runtime_scripts_can_create_clickable_buttons() {
        let (script, diagnostics) = compile_script_source(
            "scripts/button.varg",
            r#"script ButtonProbe {
    func update(_ dt: Float) {
        if ui.button("continue", "Continue", 100.0, 80.0, 220.0, 64.0) {
            state.clicked = 1
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let input = engine_platform::InputState::default();
        let output = script.run_hook_borrowed(
            "update",
            VargRuntimeContextRef {
                transform: Transform::default(),
                input: &input,
                pointer_pressed: &[],
                pointer_released: &[(140.0, 120.0)],
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: &HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(
            output.state.get("clicked").and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(output.ui_commands.len(), 2);
    }

    #[test]
    fn runtime_scripts_can_use_minimal_interactive_ui_controls() {
        let (script, diagnostics) = compile_script_source(
            "scripts/controls.varg",
            r#"script Controls {
    var enabled: Bool = false
    var volume: Float = 0.0
    var x: Float = 0.0

    func update(_ dt: Float) {
        state.enabled = ui.toggle("enabled", state.enabled, 10.0, 10.0, 48.0, 24.0)
        state.volume = ui.slider("volume", state.volume, 10.0, 40.0, 100.0, 24.0, 0.0, 1.0)
        state.x += ui.dragX("drag", 10.0, 80.0, 80.0, 32.0)
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let mut input = engine_platform::InputState::default();
        input.apply_event(engine_platform::InputEvent::MouseMove { x: 75.0, y: 52.0 });
        input.apply_event(engine_platform::InputEvent::MouseButtonDown(
            engine_platform::MouseButton::Left,
        ));
        let output = script.run_hook_borrowed(
            "update",
            VargRuntimeContextRef {
                transform: Transform::default(),
                input: &input,
                pointer_pressed: &[(75.0, 52.0)],
                pointer_released: &[(20.0, 20.0)],
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: &HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(
            output.state.get("enabled").and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert!(
            output
                .state
                .get("volume")
                .and_then(|value| value.as_f64())
                .is_some_and(|value| (value - 0.65).abs() < 0.0001)
        );
        assert_eq!(output.ui_commands.len(), 5);

        let mut state = output.state;
        let mut input = engine_platform::InputState::default();
        input.apply_event(engine_platform::InputEvent::MouseMove { x: 20.0, y: 90.0 });
        input.apply_event(engine_platform::InputEvent::MouseButtonDown(
            engine_platform::MouseButton::Left,
        ));
        input.apply_event(engine_platform::InputEvent::MouseMove { x: 36.0, y: 90.0 });
        let output = script.run_hook_borrowed(
            "update",
            VargRuntimeContextRef {
                transform: Transform::default(),
                input: &input,
                pointer_pressed: &[(20.0, 90.0)],
                pointer_released: &[],
                delta_time: 0.016,
                total_time: 0.032,
                frame_index: 2,
                screen_size: (800.0, 600.0),
                exported_values: &HashMap::new(),
                state: std::mem::take(&mut state),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(
            output.state.get("x").and_then(|value| value.as_f64()),
            Some(16.0)
        );
    }

    #[test]
    fn runtime_scripts_can_use_single_line_ui_input() {
        let (script, diagnostics) = compile_script_source(
            "scripts/input.varg",
            r#"script InputProbe {
    func update(_ dt: Float) {
        state.name = ui.input("name", "Name", 20.0, 20.0, 160.0, 32.0)
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let mut input = engine_platform::InputState::default();
        input.apply_event(engine_platform::InputEvent::KeyDown(
            engine_platform::KeyCode::Character('A'),
        ));
        let output = script.run_hook_borrowed(
            "update",
            VargRuntimeContextRef {
                transform: Transform::default(),
                input: &input,
                pointer_pressed: &[],
                pointer_released: &[(32.0, 28.0)],
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: &HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(
            output.state.get("name").and_then(|value| value.as_str()),
            Some("a")
        );
        assert_eq!(output.ui_commands.len(), 2);
    }

    #[test]
    fn runtime_scripts_can_use_micro_animation_helpers() {
        let (script, diagnostics) = compile_script_source(
            "scripts/easing.varg",
            r#"script Easing {
    func update(_ dt: Float) {
        state.smooth = smoothstep(0.0, 1.0, 0.5)
        state.out = easeOut(0.5)
        state.inout = easeInOut(0.5)
        state.pulse = pulse(0.25, 1.0)
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(
            output.state.get("smooth").and_then(|value| value.as_f64()),
            Some(0.5)
        );
        assert_eq!(
            output.state.get("out").and_then(|value| value.as_f64()),
            Some(0.75)
        );
        assert_eq!(
            output.state.get("inout").and_then(|value| value.as_f64()),
            Some(0.5)
        );
        assert!(
            output
                .state
                .get("pulse")
                .and_then(|value| value.as_f64())
                .is_some_and(|value| (value - 1.0).abs() < 0.0001)
        );
    }

    #[test]
    fn runtime_scripts_can_release_mouse_capture_with_escape() {
        let (script, diagnostics) = compile_script_source(
            "scripts/input_capture.varg",
            r#"script InputCapture {
    func update(_ dt: Float) {
        if Input.pressed("Escape") {
            Input.captureMouse(false)
        } else {
            Input.captureMouse(true)
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let mut input = engine_platform::InputState::default();
        input.apply_event(engine_platform::InputEvent::KeyDown(
            engine_platform::KeyCode::Escape,
        ));

        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input,
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        assert_eq!(output.mouse_capture, Some(false));
    }

    #[test]
    fn runtime_scripts_can_drive_transform_rotation() {
        let (script, diagnostics) = compile_script_source(
            "scripts/look.varg",
            r#"script Look {
    var yaw: Float = 10.0

    func update(_ dt: Float) {
        yaw += 25.0
        rotation = Vec3(-12.0, yaw, 0.0)
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let script = script.unwrap();
        let output = script.run_hook(
            "update",
            VargRuntimeContext {
                transform: Transform::default(),
                input: engine_platform::InputState::default(),
                delta_time: 0.016,
                total_time: 0.016,
                frame_index: 1,
                screen_size: (800.0, 600.0),
                exported_values: HashMap::new(),
                state: HashMap::new(),
                scene: VargSceneContext::default(),
            },
        );

        let forward = output.transform.rotation.rotate(Vec3::new(0.0, 0.0, -1.0));
        assert!(
            forward.y < -0.15,
            "negative x rotation should pitch the view down, got {forward:?}"
        );
        assert!(
            forward.x.abs() > 0.25,
            "non-zero y rotation should yaw the view sideways, got {forward:?}"
        );
    }
}

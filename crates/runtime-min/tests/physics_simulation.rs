use engine_core::{EngineConfig, math::Transform, math::Vec3};
use engine_ecs::{ColliderComponentData, ComponentData, RigidbodyComponentData, Scene};
use runtime_min::headless_services_from_scene;
use std::time::Duration;

/// Test that a dynamic rigidbody with gravity falls over 60 fixed timesteps.
/// This verifies the full game loop with physics integration works correctly.
#[test]
#[cfg(feature = "physics")]
fn windowless_scene_simulation_with_physics() {
    // Create a scene with a ground plane and a falling cube
    let mut scene = Scene::default();

    // Ground plane (static rigidbody at y = -0.5)
    let ground = scene.create_object("Ground").unwrap();
    scene
        .upsert_component(
            ground,
            ComponentData::Rigidbody(RigidbodyComponentData {
                body_type: "static".to_string(),
                mass: 1.0,
                use_gravity: false,
                linear_damping: 0.0,
                angular_damping: 0.05,
                lock_position: [false; 3],
                lock_rotation: [false; 3],
            }),
        )
        .unwrap();
    scene
        .upsert_component(
            ground,
            ComponentData::Collider(ColliderComponentData {
                shape: "box".to_string(),
                size: Vec3::new(10.0, 0.1, 10.0), // Large flat plane
                is_trigger: false,
                mask: !0,
                physics_material: "default".to_string(),
            }),
        )
        .unwrap();
    // Set ground position
    scene.transforms_mut().set_local(
        ground,
        Transform {
            translation: Vec3::new(0.0, -0.5, 0.0),
            ..Transform::IDENTITY
        },
    );

    // Falling cube (dynamic rigidbody at y = 5.0)
    let cube = scene.create_object("FallingCube").unwrap();
    scene
        .upsert_component(
            cube,
            ComponentData::Rigidbody(RigidbodyComponentData {
                body_type: "dynamic".to_string(),
                mass: 1.0,
                use_gravity: true,
                linear_damping: 0.0,
                angular_damping: 0.05,
                lock_position: [false; 3],
                lock_rotation: [false; 3],
            }),
        )
        .unwrap();
    scene
        .upsert_component(
            cube,
            ComponentData::Collider(ColliderComponentData {
                shape: "box".to_string(),
                size: Vec3::ONE,
                is_trigger: false,
                mask: !0,
                physics_material: "default".to_string(),
            }),
        )
        .unwrap();
    // Set cube initial position high above ground
    scene.transforms_mut().set_local(
        cube,
        Transform {
            translation: Vec3::new(0.0, 5.0, 0.0),
            ..Transform::IDENTITY
        },
    );

    // Record initial position
    let initial_y = scene.transforms().local(cube).unwrap().translation.y;
    assert_eq!(initial_y, 5.0, "Initial position should be y=5.0");

    // Create headless runtime services with physics
    let mut services = headless_services_from_scene(
        EngineConfig::default(),
        std::env::current_dir().unwrap(),
        &scene,
    )
    .unwrap();

    // Run 60 fixed timesteps (1 second of simulated time at 60 Hz)
    let fixed_dt = Duration::from_secs_f32(1.0 / 60.0);
    for _ in 0..60 {
        services.run_frame(fixed_dt, false).unwrap();
    }

    // Verify the cube has fallen due to gravity
    let final_transform = services.scene.transforms().local(cube).unwrap();
    let final_y = final_transform.translation.y;

    // The cube should have fallen significantly (gravity pulls it down)
    assert!(
        final_y < initial_y,
        "Cube should have fallen: initial_y={}, final_y={}",
        initial_y,
        final_y
    );

    // The cube should have fallen at least 1 meter (conservative check)
    assert!(
        final_y < 4.0,
        "Cube should have fallen at least 1 meter: final_y={}",
        final_y
    );

    // Verify no NaN or infinity in transform components
    assert!(
        final_transform.translation.x.is_finite(),
        "Translation X should be finite"
    );
    assert!(
        final_transform.translation.y.is_finite(),
        "Translation Y should be finite"
    );
    assert!(
        final_transform.translation.z.is_finite(),
        "Translation Z should be finite"
    );
    assert!(
        final_transform.rotation.x.is_finite(),
        "Rotation X should be finite"
    );
    assert!(
        final_transform.rotation.y.is_finite(),
        "Rotation Y should be finite"
    );
    assert!(
        final_transform.rotation.z.is_finite(),
        "Rotation Z should be finite"
    );
    assert!(
        final_transform.rotation.w.is_finite(),
        "Rotation W should be finite"
    );
    assert!(
        final_transform.scale.x.is_finite(),
        "Scale X should be finite"
    );
    assert!(
        final_transform.scale.y.is_finite(),
        "Scale Y should be finite"
    );
    assert!(
        final_transform.scale.z.is_finite(),
        "Scale Z should be finite"
    );

    // Verify frame counter advanced
    assert_eq!(
        services.frame_index(),
        60,
        "Frame index should be 60 after 60 frames"
    );

    // Verify physics steps were executed
    assert!(
        services.stats.physics_steps > 0,
        "Physics steps should have been executed"
    );
}

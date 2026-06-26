//! Headless Varg script runtime benchmark.
//!
//! Run with:
//! `cargo run -p runtime-min --release --example script_runtime_benchmark`
//!
//! Optional environment overrides:
//! - `VARG_BENCH_ENTITIES=1000`
//! - `VARG_BENCH_TARGETS=128`
//! - `VARG_BENCH_WARMUP_FRAMES=30`
//! - `VARG_BENCH_FRAMES=240`

use std::{hint::black_box, time::Duration};

use engine_core::{
    EngineConfig,
    math::{Transform, Vec3},
};
use engine_ecs::{ComponentData, ScriptComponent};
use runtime_min::RuntimeServices;

const SCRIPT_SOURCE: &str = r#"script BenchAgent {
    var ticks: Int = 0
    var playerX: Float = 0.0
    var nearestTarget: Float = 0.0

    func update(_ dt: Float) {
        state.ticks += 1
        state.playerX = scene.xOf("Player")
        state.nearestTarget = scene.distanceToTag("Target")
        if playerDistance() >= 0.0 {
            position.x += 0.001
        }
    }
}
"#;

fn main() {
    let entity_count = env_usize("VARG_BENCH_ENTITIES", 1_000);
    let target_count = env_usize("VARG_BENCH_TARGETS", 128);
    let warmup_frames = env_usize("VARG_BENCH_WARMUP_FRAMES", 30);
    let measured_frames = env_usize("VARG_BENCH_FRAMES", 240);

    let root = tempfile::tempdir().expect("create temp benchmark project");
    let scripts = root.path().join("scripts");
    std::fs::create_dir_all(&scripts).expect("create scripts dir");
    std::fs::write(scripts.join("bench_agent.varg"), SCRIPT_SOURCE).expect("write bench script");

    let mut services = RuntimeServices::minimal(EngineConfig::default());
    services.set_project_root(root.path());
    seed_scene(&mut services, entity_count, target_count);

    for _ in 0..warmup_frames {
        services
            .tick_game_frame(Duration::from_millis(16), false)
            .expect("warmup frame");
    }

    let start = std::time::Instant::now();
    for _ in 0..measured_frames {
        services
            .tick_game_frame(Duration::from_millis(16), false)
            .expect("measured frame");
        black_box(services.stats.entity_count);
    }
    let elapsed = start.elapsed();

    let total_frames = measured_frames.max(1) as f64;
    let total_script_invocations = (entity_count * measured_frames.max(1)) as f64;
    let frame_ms = elapsed.as_secs_f64() * 1_000.0 / total_frames;
    let invocation_us = elapsed.as_secs_f64() * 1_000_000.0 / total_script_invocations;

    println!("Varg script runtime benchmark");
    println!("  scripted entities : {entity_count}");
    println!("  target entities   : {target_count}");
    println!("  warmup frames     : {warmup_frames}");
    println!("  measured frames   : {measured_frames}");
    println!(
        "  total elapsed     : {:.3} ms",
        elapsed.as_secs_f64() * 1_000.0
    );
    println!("  mean frame        : {frame_ms:.4} ms");
    println!("  mean invocation   : {invocation_us:.4} us");
    println!("  final entities    : {}", services.stats.entity_count);
}

fn seed_scene(services: &mut RuntimeServices, entity_count: usize, target_count: usize) {
    let player = services
        .scene
        .create_object("Player")
        .expect("create player");
    services
        .scene
        .object_mut(player)
        .expect("player object")
        .tag = "Player".to_string();
    services.scene.transforms_mut().set_local(
        player,
        Transform {
            translation: Vec3::new(0.0, 0.0, 0.0),
            ..Transform::IDENTITY
        },
    );

    for index in 0..target_count {
        let target = services
            .scene
            .create_object(format!("Target {index}"))
            .expect("create target");
        services
            .scene
            .object_mut(target)
            .expect("target object")
            .tag = "Target".to_string();
        services.scene.transforms_mut().set_local(
            target,
            Transform {
                translation: Vec3::new(index as f32 * 0.25, 0.0, 8.0),
                ..Transform::IDENTITY
            },
        );
    }

    for index in 0..entity_count {
        let entity = services
            .scene
            .create_object(format!("Agent {index}"))
            .expect("create agent");
        services.scene.object_mut(entity).expect("agent object").tag = "Agent".to_string();
        services.scene.transforms_mut().set_local(
            entity,
            Transform {
                translation: Vec3::new(index as f32 * 0.01, 0.0, 4.0),
                ..Transform::IDENTITY
            },
        );
        services
            .scene
            .upsert_component(
                entity,
                ComponentData::Script(ScriptComponent::new("scripts/bench_agent.varg")),
            )
            .expect("attach script");
    }
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
}

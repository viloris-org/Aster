#![forbid(unsafe_code)]

use std::process::{Command, ExitCode};

use engine_core::{EngineError, EngineResult};
use engine_packager::{
    PackageChannel, PackageFormat, PackageRequest, PackageTarget, package_project,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> EngineResult<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("runtime-min") => build_profile(Profile::RuntimeMin, false),
        Some("build-editor") => build_profile(Profile::Editor, false),
        Some("agent-smoke") => cargo([
            "test",
            "-p",
            "engine-editor",
            "--no-default-features",
            "--features",
            "agent-tools",
        ]),
        Some("vscene") => vscene(args.collect()),
        Some("varg-lsp") => cargo(["run", "-p", "engine-script-varg", "--bin", "varg-lsp"]),
        Some("viewport-perf") => cargo([
            "run",
            "-p",
            "engine-render-wgpu",
            "--release",
            "--example",
            "viewport_path_perf",
        ]),
        Some("package") => package(args.collect()),
        Some("test") => cargo(["test", "--workspace"]),
        Some("check") => cargo(["check", "--workspace", "--all-features"]),
        Some(command) => Err(EngineError::config(format!(
            "unknown xtask command `{command}`"
        ))),
        None => Err(EngineError::config(
            "expected xtask command: runtime-min, build-editor, agent-smoke, vscene, varg-lsp, viewport-perf, package, test, or check",
        )),
    }
}

fn vscene(args: Vec<String>) -> EngineResult<()> {
    let mut iter = args.into_iter();
    match iter.next().as_deref() {
        Some("compile") => vscene_compile(iter.collect()),
        Some("--help") | Some("-h") | None => {
            print_vscene_help();
            Ok(())
        }
        Some(command) => Err(EngineError::config(format!(
            "unknown vscene command `{command}`"
        ))),
    }
}

fn vscene_compile(args: Vec<String>) -> EngineResult<()> {
    let mut input = None;
    let mut output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" | "-o" => {
                let Some(value) = iter.next() else {
                    return Err(EngineError::config("--out requires a value"));
                };
                output = Some(std::path::PathBuf::from(value));
            }
            "--help" | "-h" => {
                print_vscene_help();
                return Ok(());
            }
            value if input.is_none() => input = Some(std::path::PathBuf::from(value)),
            other => {
                return Err(EngineError::config(format!(
                    "unknown vscene compile argument `{other}`"
                )));
            }
        }
    }

    let Some(input) = input else {
        return Err(EngineError::config("vscene compile requires an input file"));
    };
    let source = std::fs::read_to_string(&input).map_err(|source| EngineError::Filesystem {
        path: input.clone(),
        source,
    })?;
    let (file, diagnostics) =
        engine_script_varg::compile_vscene_source_to_scene_file(&input, &source);
    for diagnostic in &diagnostics {
        eprintln!(
            "{}:{}:{}: {}: {}",
            input.display(),
            diagnostic.line.unwrap_or(1),
            diagnostic.column.unwrap_or(1),
            diagnostic.code,
            diagnostic.message
        );
    }
    let Some(file) = file else {
        return Err(EngineError::config(".vscene compilation failed"));
    };
    let scene_name = file.name.clone();
    let scene = engine_ecs::Scene::from_scene_file(file)?;
    let json = scene.to_json(scene_name)?;

    if let Some(output) = output {
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|source| EngineError::Filesystem {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::write(&output, json).map_err(|source| EngineError::Filesystem {
            path: output.clone(),
            source,
        })?;
        println!("Compiled {} -> {}", input.display(), output.display());
    } else {
        println!("{json}");
    }
    Ok(())
}

fn print_vscene_help() {
    println!("usage: cargo xtask vscene compile INPUT.vscene [--out OUTPUT.scene.json]");
}

fn package(args: Vec<String>) -> EngineResult<()> {
    let mut project = std::path::PathBuf::from("examples/project/fps_arena");
    let mut target = PackageTarget::current_desktop();
    let mut format = PackageFormat::Folder;
    let mut channel = PackageChannel::Debug;
    let mut output_dir = None;
    let mut optimize_assets = true;
    let mut include_debug_symbols = false;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--project" | "-p" => {
                let Some(value) = iter.next() else {
                    return Err(EngineError::config("--project requires a value"));
                };
                project = value.into();
            }
            "--target" => {
                let Some(value) = iter.next() else {
                    return Err(EngineError::config("--target requires a value"));
                };
                target = PackageTarget::parse(&value)?;
            }
            "--format" => {
                let Some(value) = iter.next() else {
                    return Err(EngineError::config("--format requires a value"));
                };
                format = PackageFormat::parse(&value)?;
            }
            "--channel" => {
                let Some(value) = iter.next() else {
                    return Err(EngineError::config("--channel requires a value"));
                };
                channel = PackageChannel::parse(&value)?;
            }
            "--release" => channel = PackageChannel::Release,
            "--debug" => channel = PackageChannel::Debug,
            "--output" | "-o" => {
                let Some(value) = iter.next() else {
                    return Err(EngineError::config("--output requires a value"));
                };
                output_dir = Some(value.into());
            }
            "--no-optimize-assets" => optimize_assets = false,
            "--include-debug-symbols" => include_debug_symbols = true,
            "--help" | "-h" => {
                print_package_help();
                return Ok(());
            }
            other => {
                return Err(EngineError::config(format!(
                    "unknown package argument `{other}`"
                )));
            }
        }
    }

    let output = package_project(&PackageRequest {
        project,
        repo_root: workspace_root()?,
        target,
        format,
        channel,
        optimize_assets,
        include_debug_symbols,
        output_dir,
    })?;
    println!("Packaged {}", output.project);
    println!("  target: {}", output.target);
    println!("  format: {}", output.format);
    println!("  channel: {}", output.channel);
    println!("  output: {}", output.path.display());
    if let Some(binary) = output.binary {
        println!("  binary: {}", binary.display());
    }
    if let Some(launcher) = output.launcher {
        println!("  launcher: {}", launcher.display());
    }
    println!(
        "  assets: {} ({})",
        output.asset_count,
        output.assets_manifest.display()
    );
    Ok(())
}

fn print_package_help() {
    println!(
        "usage: cargo xtask package [--project PATH] [--target TARGET] [--format FORMAT] [--debug|--release] [--output PATH]"
    );
    println!(
        "targets: native, linux-x64, windows-x64, macos-universal, android-arm64, ios-universal"
    );
    println!("formats: folder, apk, aab, ipa, appimage, deb, rpm, exe, msi, nsis, dmg");
}

fn workspace_root() -> EngineResult<std::path::PathBuf> {
    std::env::current_dir().map_err(|source| EngineError::Filesystem {
        path: ".".into(),
        source,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Profile {
    RuntimeMin,
    Editor,
}

impl Profile {
    const fn name(self) -> &'static str {
        match self {
            Self::RuntimeMin => "runtime-min",
            Self::Editor => "editor",
        }
    }
}

fn build_profile(profile: Profile, release: bool) -> EngineResult<()> {
    let mut base_args = vec![
        "build",
        "-p",
        "runtime-min",
        "--no-default-features",
        "--features",
        profile.name(),
    ];
    if release {
        base_args.push("--release");
    }
    cargo_vec(&base_args)
}

fn cargo_vec(args: &[&str]) -> EngineResult<()> {
    let status = Command::new("cargo")
        .args(args)
        .status()
        .map_err(|source| EngineError::Filesystem {
            path: "cargo".into(),
            source,
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(EngineError::other(format!("cargo exited with {status}")))
    }
}

fn cargo<const N: usize>(args: [&str; N]) -> EngineResult<()> {
    let status = Command::new("cargo")
        .args(args)
        .status()
        .map_err(|source| EngineError::Filesystem {
            path: "cargo".into(),
            source,
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(EngineError::other(format!("cargo exited with {status}")))
    }
}

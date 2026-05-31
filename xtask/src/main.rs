#![forbid(unsafe_code)]

use std::process::{Command, ExitCode};

use engine_core::{EngineError, EngineResult};

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
        Some("test") => cargo(["test", "--workspace"]),
        Some("check") => cargo(["check", "--workspace", "--all-features"]),
        Some(command) => Err(EngineError::config(format!(
            "unknown xtask command `{command}`"
        ))),
        None => Err(EngineError::config(
            "expected xtask command: runtime-min, build-editor, agent-smoke, test, or check",
        )),
    }
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

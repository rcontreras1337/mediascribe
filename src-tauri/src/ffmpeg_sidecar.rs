//! Thin wrappers around the bundled `ffmpeg` and `ffprobe` sidecars.
//!
//! Sidecar binaries are placed at `src-tauri/binaries/ffmpeg-<target>` and
//! `src-tauri/binaries/ffprobe-<target>` by `scripts/fetch-ffmpeg.ps1`. They
//! ship inside the final installer thanks to `bundle.externalBin` in
//! `tauri.conf.json`.
//!
//! Naming follows Tauri's sidecar convention: when you ask for `binaries/ffmpeg`,
//! Tauri appends the host's `target_triple` automatically (e.g.
//! `binaries/ffmpeg-x86_64-pc-windows-msvc.exe` on Windows).

use tauri::AppHandle;
use tauri_plugin_shell::{process::CommandEvent, ShellExt};
use thiserror::Error;

/// Sidecar identifiers as configured in `tauri.conf.json` and `capabilities/default.json`.
const FFMPEG_SIDECAR: &str = "binaries/ffmpeg";
const FFPROBE_SIDECAR: &str = "binaries/ffprobe";

/// What we got back from running a sidecar to completion.
#[derive(Debug)]
pub struct SidecarOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

impl SidecarOutput {
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }
}

/// Errors that can come out of running a sidecar.
#[derive(Debug, Error)]
pub enum SidecarError {
    /// Tauri couldn't find the sidecar or couldn't construct the command (config / packaging issue).
    #[error("failed to set up sidecar `{name}`: {source}")]
    Spawn {
        name: &'static str,
        #[source]
        source: tauri_plugin_shell::Error,
    },

    /// The sidecar ran but exited non-zero. `stderr` carries ffmpeg's own
    /// error message, which is what the user actually needs to see.
    #[error("sidecar `{name}` exited with code {exit_code:?}\nstderr:\n{stderr}")]
    NonZero {
        name: &'static str,
        exit_code: Option<i32>,
        stderr: String,
    },
}

/// Runs the ffmpeg sidecar with `args`, returning combined output once it exits.
pub async fn run_ffmpeg(app: &AppHandle, args: &[String]) -> Result<SidecarOutput, SidecarError> {
    run_sidecar(app, FFMPEG_SIDECAR, "ffmpeg", args).await
}

/// Runs the ffprobe sidecar with `args`, returning combined output once it exits.
pub async fn run_ffprobe(app: &AppHandle, args: &[String]) -> Result<SidecarOutput, SidecarError> {
    run_sidecar(app, FFPROBE_SIDECAR, "ffprobe", args).await
}

/// Internal: spawn a sidecar, drain its event stream, and assemble the output.
///
/// We don't stream events to the frontend here; the orchestrator will, for the
/// long-running ffmpeg extraction step. This function is the low-level "run to
/// completion and tell me what came out" primitive.
async fn run_sidecar(
    app: &AppHandle,
    sidecar_id: &str,
    name: &'static str,
    args: &[String],
) -> Result<SidecarOutput, SidecarError> {
    let (mut rx, _child) = app
        .shell()
        .sidecar(sidecar_id)
        .map_err(|e| SidecarError::Spawn { name, source: e })?
        .args(args)
        .spawn()
        .map_err(|e| SidecarError::Spawn { name, source: e })?;

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code: Option<i32> = None;

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(line) => {
                stdout.push_str(&String::from_utf8_lossy(&line));
                stdout.push('\n');
            }
            CommandEvent::Stderr(line) => {
                stderr.push_str(&String::from_utf8_lossy(&line));
                stderr.push('\n');
            }
            CommandEvent::Terminated(payload) => {
                exit_code = payload.code;
            }
            CommandEvent::Error(err) => {
                stderr.push_str(&err);
                stderr.push('\n');
            }
            _ => {}
        }
    }

    let out = SidecarOutput {
        stdout,
        stderr,
        exit_code,
    };

    if out.success() {
        Ok(out)
    } else {
        Err(SidecarError::NonZero {
            name,
            exit_code: out.exit_code,
            stderr: out.stderr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pure tests only: actual sidecar invocation needs a Tauri AppHandle and
    // the binaries present. Those are integration-level and live in tests/
    // (not run in CI).

    #[test]
    fn sidecar_output_success_is_zero_exit() {
        let ok = SidecarOutput {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0),
        };
        assert!(ok.success());

        let bad = SidecarOutput {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(1),
        };
        assert!(!bad.success());

        let killed = SidecarOutput {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: None,
        };
        assert!(!killed.success());
    }

    #[test]
    fn sidecar_error_display_contains_name_and_stderr() {
        let err = SidecarError::NonZero {
            name: "ffmpeg",
            exit_code: Some(1),
            stderr: "Invalid argument".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("ffmpeg"));
        assert!(s.contains("Invalid argument"));
    }
}

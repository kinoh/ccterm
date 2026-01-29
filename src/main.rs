mod hooks;
mod sessions;

use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_MESSAGE: &str = "hello from ccterm";
const DEFAULT_TIMEOUT_SECS: u64 = 180;
const DEFAULT_PREFIX: &str = "ccterm";
const DEFAULT_CLAUDE_CMD: &str = "claude";

fn main() -> Result<()> {
    let mut args: Vec<String> = env::args().collect();
    let _bin = args.remove(0);
    if args.is_empty() {
        print_usage();
        return Ok(());
    }

    match args[0].as_str() {
        "hook" => run_hook(&args[1..]),
        "run" => run_session(&args[1..]),
        "help" | "-h" | "--help" => {
            print_usage();
            Ok(())
        }
        _ => {
            print_usage();
            Ok(())
        }
    }
}

fn run_hook(args: &[String]) -> Result<()> {
    let mut out_path: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                let value = args.get(i + 1).context("--out requires a value")?;
                out_path = Some(PathBuf::from(value));
                i += 2;
            }
            "--help" | "-h" => {
                print_hook_usage();
                return Ok(());
            }
            other => {
                return Err(anyhow::anyhow!("unknown hook argument: {other}"));
            }
        }
    }

    let out_path = out_path.context("--out is required")?;
    hooks::append_stdin_to_file(&out_path)
}

fn run_session(args: &[String]) -> Result<()> {
    let mut message = DEFAULT_MESSAGE.to_string();
    let mut timeout_secs = DEFAULT_TIMEOUT_SECS;
    let mut prefix = DEFAULT_PREFIX.to_string();
    let mut claude_cmd = DEFAULT_CLAUDE_CMD.to_string();
    let mut hook_path = sessions::default_hook_path()?;
    let mut cwd = sessions::default_cwd()?;
    let mut keep_session = false;
    let mut accept_trust = false;
    let mut startup_wait_ms: u64 = 1500;
    let mut post_trust_wait_ms: u64 = 1500;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--message" => {
                let value = args.get(i + 1).context("--message requires a value")?;
                message = value.to_string();
                i += 2;
            }
            "--timeout" => {
                let value = args.get(i + 1).context("--timeout requires a value")?;
                timeout_secs = value.parse().context("invalid --timeout")?;
                i += 2;
            }
            "--prefix" => {
                let value = args.get(i + 1).context("--prefix requires a value")?;
                prefix = value.to_string();
                i += 2;
            }
            "--claude-cmd" => {
                let value = args.get(i + 1).context("--claude-cmd requires a value")?;
                claude_cmd = value.to_string();
                i += 2;
            }
            "--hook-path" => {
                let value = args.get(i + 1).context("--hook-path requires a value")?;
                hook_path = PathBuf::from(value);
                i += 2;
            }
            "--cwd" => {
                let value = args.get(i + 1).context("--cwd requires a value")?;
                cwd = PathBuf::from(value);
                i += 2;
            }
            "--keep-session" => {
                keep_session = true;
                i += 1;
            }
            "--accept-trust" => {
                accept_trust = true;
                i += 1;
            }
            "--startup-wait-ms" => {
                let value = args
                    .get(i + 1)
                    .context("--startup-wait-ms requires a value")?;
                startup_wait_ms = value.parse().context("invalid --startup-wait-ms")?;
                i += 2;
            }
            "--post-trust-wait-ms" => {
                let value = args
                    .get(i + 1)
                    .context("--post-trust-wait-ms requires a value")?;
                post_trust_wait_ms = value.parse().context("invalid --post-trust-wait-ms")?;
                i += 2;
            }
            "--help" | "-h" => {
                print_run_usage();
                return Ok(());
            }
            other => {
                return Err(anyhow::anyhow!("unknown run argument: {other}"));
            }
        }
    }

    sessions::ensure_tmux_available()?;
    sessions::ensure_claude_available(&claude_cmd)?;
    sessions::ensure_dir(&hook_path)?;

    let session_name = sessions::timestamp_session_name(&prefix)?;
    let manager = sessions::TmuxSessionManager::new(&claude_cmd, &cwd);

    manager
        .spawn(&session_name)
        .with_context(|| format!("failed to spawn tmux session {session_name}"))?;

    std::thread::sleep(Duration::from_millis(startup_wait_ms));
    if accept_trust {
        manager.send_enter(&session_name)?;
        std::thread::sleep(Duration::from_millis(post_trust_wait_ms));
    }

    manager
        .send(&session_name, &message)
        .with_context(|| format!("failed to send message to {session_name}"))?;

    let mut follower = hooks::HookFollower::open(&hook_path)?;
    let line = follower.wait_for_line(Duration::from_secs(timeout_secs))?;
    println!("hook: {line}");

    if !keep_session {
        manager
            .stop(&session_name)
            .with_context(|| format!("failed to stop session {session_name}"))?;
    }

    Ok(())
}

fn print_usage() {
    eprintln!("ccterm usage:\n  ccterm run [options]\n  ccterm hook --out <path>");
}

fn print_run_usage() {
    eprintln!(
        "ccterm run options:\n  --message <text>\n  --timeout <secs>\n  --prefix <session-prefix>\n  --claude-cmd <command>\n  --hook-path <path>\n  --cwd <path>\n  --keep-session\n  --accept-trust\n  --startup-wait-ms <ms>\n  --post-trust-wait-ms <ms>"
    );
}

fn print_hook_usage() {
    eprintln!("ccterm hook --out <path>");
}

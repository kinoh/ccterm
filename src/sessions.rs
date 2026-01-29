use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct TmuxSessionManager {
    claude_cmd: String,
    cwd: PathBuf,
}

impl TmuxSessionManager {
    pub fn new(claude_cmd: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            claude_cmd: claude_cmd.into(),
            cwd: cwd.into(),
        }
    }

    pub fn spawn(&self, session_name: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                session_name,
                "-c",
                self.cwd
                    .to_str()
                    .context("failed to convert cwd to string")?,
                &self.claude_cmd,
            ])
            .status()
            .context("failed to start tmux session")?;

        if !status.success() {
            bail!("tmux new-session failed with status: {status}");
        }
        Ok(())
    }

    pub fn send(&self, session_name: &str, text: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args([
                "send-keys",
                "-t",
                session_name,
                text,
            ])
            .status()
            .context("failed to send keys to tmux")?;

        if !status.success() {
            bail!("tmux send-keys failed with status: {status}");
        }
        std::thread::sleep(Duration::from_millis(5));
        self.send_enter(session_name)?;
        Ok(())
    }

    fn send_enter(&self, session_name: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["send-keys", "-t", session_name, "C-m"])
            .status()
            .context("failed to send enter to tmux")?;

        if !status.success() {
            bail!("tmux send-keys C-m failed with status: {status}");
        }
        Ok(())
    }

    pub fn stop(&self, session_name: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["kill-session", "-t", session_name])
            .status()
            .context("failed to stop tmux session")?;

        if !status.success() {
            bail!("tmux kill-session failed with status: {status}");
        }
        Ok(())
    }

    pub fn capture_pane(&self, session_name: &str, lines: usize) -> Result<String> {
        let line_arg = format!("-{}", lines);
        let output = Command::new("tmux")
            .args(["capture-pane", "-t", session_name, "-p", "-S", &line_arg])
            .output()
            .context("failed to capture tmux pane")?;

        if !output.status.success() {
            bail!("tmux capture-pane failed with status: {}", output.status);
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

pub fn timestamp_session_name(prefix: &str) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before UNIX_EPOCH")?;
    Ok(format!("{prefix}-{}", now.as_secs()))
}

pub fn ensure_tmux_available() -> Result<()> {
    let status = Command::new("tmux")
        .arg("-V")
        .status()
        .context("failed to run tmux -V")?;
    if !status.success() {
        bail!("tmux -V failed with status: {status}");
    }
    Ok(())
}

pub fn ensure_claude_available(command: &str) -> Result<()> {
    let status = Command::new("sh")
        .args(["-lc", &format!("command -v {}", command)])
        .status()
        .context("failed to check claude command")?;
    if !status.success() {
        bail!("command not found: {command}");
    }
    Ok(())
}

pub fn default_cwd() -> Result<PathBuf> {
    std::env::current_dir().context("failed to read current dir")
}

pub fn default_hook_path() -> Result<PathBuf> {
    let cwd = default_cwd()?;
    Ok(cwd.join(".claude/hooks/events.jsonl"))
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create dir: {}", parent.display()))?;
    }
    Ok(())
}

pub fn wait_for_prompt(
    manager: &TmuxSessionManager,
    session_name: &str,
    timeout: Duration,
    poll: Duration,
) -> Result<()> {
    let start = std::time::Instant::now();
    loop {
        let pane = manager.capture_pane(session_name, 200)?;
        if prompt_ready(&pane) {
            return Ok(());
        }
        if start.elapsed() > timeout {
            bail!("timed out waiting for input prompt");
        }
        std::thread::sleep(poll);
    }
}

fn prompt_ready(pane: &str) -> bool {
    let lines: Vec<String> = pane
        .lines()
        .map(|line| line.replace('\u{00A0}', " "))
        .collect();

    for line in lines.iter().rev().take(20) {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix('❯') {
            if rest.contains("esc to interrupt") {
                return false;
            }
            return true;
        }
        if let Some(rest) = trimmed.strip_prefix('>') {
            if rest.contains("esc to interrupt") {
                return false;
            }
            return true;
        }
    }
    false
}

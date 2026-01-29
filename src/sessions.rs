use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
                "C-m",
            ])
            .status()
            .context("failed to send keys to tmux")?;

        if !status.success() {
            bail!("tmux send-keys failed with status: {status}");
        }
        Ok(())
    }

    pub fn send_enter(&self, session_name: &str) -> Result<()> {
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

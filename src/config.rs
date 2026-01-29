use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub slack: SlackConfig,
    #[serde(default)]
    pub claude: ClaudeConfig,
    #[serde(default)]
    pub tmux: TmuxConfig,
    #[serde(default)]
    pub hooks: HooksConfig,
    #[serde(default)]
    pub coordinator: CoordinatorConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SlackConfig {
    pub bot_token: String,
    pub app_token: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ClaudeConfig {
    #[serde(default = "default_claude_cmd")]
    pub command: String,
    #[serde(default = "default_cwd")]
    pub cwd: PathBuf,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TmuxConfig {
    #[serde(default = "default_session_prefix")]
    pub session_prefix: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HooksConfig {
    #[serde(default = "default_hooks_path")]
    pub events_path: PathBuf,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CoordinatorConfig {
    #[serde(default = "default_hook_timeout_secs")]
    pub hook_timeout_secs: u64,
    #[serde(default = "default_prompt_timeout_ms")]
    pub prompt_timeout_ms: u64,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            command: default_claude_cmd(),
            cwd: default_cwd(),
        }
    }
}

impl Default for TmuxConfig {
    fn default() -> Self {
        Self {
            session_prefix: default_session_prefix(),
        }
    }
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            events_path: default_hooks_path(),
        }
    }
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            hook_timeout_secs: default_hook_timeout_secs(),
            prompt_timeout_ms: default_prompt_timeout_ms(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read config: {}", path.display()))?;
        let cfg: Config = toml::from_str(&content).context("failed to parse config toml")?;

        if cfg.slack.bot_token.trim().is_empty() || cfg.slack.app_token.trim().is_empty() {
            bail!("slack.bot_token and slack.app_token are required");
        }
        Ok(cfg)
    }
}

fn default_claude_cmd() -> String {
    "claude".to_string()
}

fn default_cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn default_session_prefix() -> String {
    "ccterm".to_string()
}

fn default_hooks_path() -> PathBuf {
    default_cwd().join(".claude/hooks/events.jsonl")
}

fn default_hook_timeout_secs() -> u64 {
    10
}

fn default_prompt_timeout_ms() -> u64 {
    10_000
}

use crate::config::Config;
use crate::context;
use crate::hooks::{self, HookEvent};
use crate::sessions::{self, TmuxSessionManager};
use crate::slack_adapter::SlackAdapter;
use crate::types::{IncomingMessage, OutgoingMessage};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ConversationKey {
    conversation_id: String,
    thread_id: Option<String>,
}

#[derive(Debug, Clone)]
struct SessionEntry {
    session_name: String,
    last_transcript_path: Option<PathBuf>,
    last_sent_message_uuid: Option<String>,
}

pub struct Coordinator {
    config: Config,
    sessions: TmuxSessionManager,
    slack: SlackAdapter,
    hook_tx: mpsc::UnboundedSender<HookEvent>,
    hook_rx: mpsc::UnboundedReceiver<HookEvent>,
    sessions_by_key: HashMap<ConversationKey, SessionEntry>,
    key_by_cwd: HashMap<PathBuf, ConversationKey>,
    main_by_conversation: HashMap<String, ConversationKey>,
    hook_paths_by_cwd: HashMap<PathBuf, PathBuf>,
    settings_template: String,
    base_cwd: PathBuf,
    ccterm_path: PathBuf,
}

impl Coordinator {
    pub fn new(config: Config, sessions: TmuxSessionManager, slack: SlackAdapter) -> Result<Self> {
        let base_cwd = normalize_path(config.claude.cwd.clone());
        let settings_path = base_cwd.join(".claude/settings.json");
        let settings_template = std::fs::read_to_string(&settings_path).with_context(|| {
            format!(
                "failed to read base settings.json: {}",
                settings_path.display()
            )
        })?;
        let ccterm_path = std::env::current_exe()
            .context("failed to resolve ccterm path")?;
        let ccterm_path = ccterm_path.canonicalize().unwrap_or(ccterm_path);

        let (hook_tx, hook_rx) = mpsc::unbounded_channel();
        Ok(Self {
            config,
            sessions,
            slack,
            hook_tx,
            hook_rx,
            sessions_by_key: HashMap::new(),
            key_by_cwd: HashMap::new(),
            main_by_conversation: HashMap::new(),
            hook_paths_by_cwd: HashMap::new(),
            settings_template,
            base_cwd,
            ccterm_path,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        let prompt_timeout = Duration::from_millis(self.config.coordinator.prompt_timeout_ms);
        let _hook_timeout = Duration::from_secs(self.config.coordinator.hook_timeout_secs);

        loop {
            tokio::select! {
                maybe_msg = self.slack.incoming().recv() => {
                    let msg = match maybe_msg {
                        Some(m) => m,
                        None => break,
                    };
                    eprintln!(
                        "coordinator: incoming slack message channel={} thread={} text_len={}",
                        msg.conversation_id,
                        msg.thread_id.as_deref().unwrap_or("-"),
                        msg.text.len()
                    );
                    if let Err(err) = self.handle_incoming(msg, prompt_timeout).await {
                        eprintln!("incoming error: {err}");
                    }
                }
                maybe_hook = self.hook_rx.recv() => {
                    if let Some(hook) = maybe_hook {
                        if let Err(err) = self.handle_hook(hook).await {
                            eprintln!("hook error: {err}");
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_incoming(&mut self, msg: IncomingMessage, prompt_timeout: Duration) -> Result<()> {
        if msg.thread_id.is_none() {
            let entry = self.ensure_main_session(&msg, prompt_timeout)?;
            self.enqueue_send(&entry, msg.text, prompt_timeout)?;
        } else {
            let entry = self.ensure_thread_session(&msg, prompt_timeout)?;
            self.enqueue_send(&entry, msg.text, prompt_timeout)?;
        }

        Ok(())
    }

    fn ensure_main_session(
        &mut self,
        msg: &IncomingMessage,
        prompt_timeout: Duration,
    ) -> Result<SessionEntry> {
        let key = self
            .main_by_conversation
            .entry(msg.conversation_id.clone())
            .or_insert_with(|| ConversationKey {
                conversation_id: msg.conversation_id.clone(),
                thread_id: None,
            })
            .clone();

        if let Some(entry) = self.sessions_by_key.get(&key) {
            return Ok(entry.clone());
        }

        let cwd = self.base_cwd.clone();
        let hook_path = self.hook_path_for_cwd(&cwd);
        self.register_hook_receiver(&cwd, &hook_path)?;

        let session_name = sessions::timestamp_session_name(&self.config.tmux.session_prefix)?;
        self.sessions
            .spawn_in(&session_name, &cwd)
            .with_context(|| format!("failed to spawn main session {session_name}"))?;
        sessions::wait_for_prompt(
            &self.sessions,
            &session_name,
            prompt_timeout,
            Duration::from_millis(200),
        )?;

        let entry = SessionEntry {
            session_name: session_name.clone(),
            last_transcript_path: None,
            last_sent_message_uuid: None,
        };
        self.sessions_by_key.insert(key.clone(), entry.clone());
        self.key_by_cwd.insert(cwd, key);
        Ok(entry)
    }

    fn ensure_thread_session(
        &mut self,
        msg: &IncomingMessage,
        prompt_timeout: Duration,
    ) -> Result<SessionEntry> {
        let key = ConversationKey {
            conversation_id: msg.conversation_id.clone(),
            thread_id: msg.thread_id.clone(),
        };

        if let Some(entry) = self.sessions_by_key.get(&key) {
            return Ok(entry.clone());
        }

        let thread_id = msg
            .thread_id
            .as_deref()
            .context("thread id missing")?;
        let cwd = self.ensure_thread_dir(thread_id)?;
        let hook_path = self.hook_path_for_cwd(&cwd);
        self.register_hook_receiver(&cwd, &hook_path)?;

        let session_name = sessions::timestamp_session_name(&self.config.tmux.session_prefix)?;
        self.sessions
            .spawn_in(&session_name, &cwd)
            .with_context(|| format!("failed to spawn thread session {session_name}"))?;

        sessions::wait_for_prompt(
            &self.sessions,
            &session_name,
            prompt_timeout,
            Duration::from_millis(200),
        )?;

        self.ensure_thread_context(&cwd, msg)?;

        let entry = SessionEntry {
            session_name: session_name.clone(),
            last_transcript_path: None,
            last_sent_message_uuid: None,
        };
        self.sessions_by_key.insert(key.clone(), entry.clone());
        self.key_by_cwd.insert(cwd, key);
        Ok(entry)
    }

    fn build_thread_context(&self, msg: &IncomingMessage) -> Result<Option<String>> {
        let main_key = self.main_by_conversation.get(&msg.conversation_id);
        let main_key = match main_key {
            Some(key) => key,
            None => return Ok(None),
        };
        let main_entry = match self.sessions_by_key.get(main_key) {
            Some(entry) => entry,
            None => return Ok(None),
        };
        let transcript_path = match &main_entry.last_transcript_path {
            Some(path) => path,
            None => return Ok(None),
        };

        let cutoff = msg.timestamp.as_deref();
        let history = context::read_history(transcript_path, cutoff)?;
        Ok(context::format_history_context(&history))
    }

    fn enqueue_send(
        &mut self,
        entry: &SessionEntry,
        text: String,
        prompt_timeout: Duration,
    ) -> Result<()> {
        sessions::wait_for_prompt(
            &self.sessions,
            &entry.session_name,
            prompt_timeout,
            Duration::from_millis(200),
        )?;
        self.sessions
            .send(&entry.session_name, &text)
            .with_context(|| format!("failed to send to {}", entry.session_name))?;
        Ok(())
    }

    fn ensure_thread_context(&self, cwd: &Path, msg: &IncomingMessage) -> Result<()> {
        let context = match self.build_thread_context(msg)? {
            Some(context) => context,
            None => return Ok(()),
        };
        let path = cwd.join("CLAUDE.md");
        if path.exists() {
            return Ok(());
        }
        std::fs::write(&path, context)
            .with_context(|| format!("failed to write CLAUDE.md: {}", path.display()))?;
        Ok(())
    }

    async fn handle_hook(&mut self, hook: HookEvent) -> Result<()> {
        if hook.event_name != "Stop" {
            return Ok(());
        }

        let cwd = normalize_path(hook.cwd.clone());
        let key = match self.key_by_cwd.get(&cwd) {
            Some(k) => k.clone(),
            None => {
                eprintln!("hook cwd not registered: {}", cwd.display());
                return Ok(());
            }
        };

        let entry = match self.sessions_by_key.get_mut(&key) {
            Some(entry) => entry,
            None => {
                eprintln!("hook session not registered: {}", hook.session_id);
                return Ok(());
            }
        };
        entry.last_transcript_path = Some(hook.transcript_path.clone());

        let latest = match context::latest_assistant_text_uuid(&hook.transcript_path)? {
            Some(latest) => latest,
            None => return Ok(()),
        };
        if entry.last_sent_message_uuid.as_deref() == Some(latest.0.as_str()) {
            return Ok(());
        }

        let assistant_text = latest.1;

        let outgoing = OutgoingMessage {
            text: assistant_text,
            conversation_id: key.conversation_id.clone(),
            thread_id: key.thread_id.clone(),
        };

        self.slack.send(&outgoing).await?;
        entry.last_sent_message_uuid = Some(latest.0);
        Ok(())
    }

    fn hook_path_for_cwd(&self, cwd: &Path) -> PathBuf {
        if self.config.hooks.events_path.is_absolute() {
            self.config.hooks.events_path.clone()
        } else {
            cwd.join(&self.config.hooks.events_path)
        }
    }

    fn register_hook_receiver(&mut self, cwd: &Path, hook_path: &Path) -> Result<()> {
        let cwd = normalize_path(cwd.to_path_buf());
        if self.hook_paths_by_cwd.contains_key(&cwd) {
            return Ok(());
        }

        sessions::ensure_dir(hook_path)?;
        let receiver = hooks::spawn_hook_receiver(hook_path.to_path_buf());
        let tx = self.hook_tx.clone();
        tokio::spawn(async move {
            let mut rx = receiver;
            while let Some(event) = rx.recv().await {
                let _ = tx.send(event);
            }
        });

        self.hook_paths_by_cwd
            .insert(cwd, hook_path.to_path_buf());
        Ok(())
    }

    fn ensure_thread_dir(&self, thread_id: &str) -> Result<PathBuf> {
        let dir = self
            .base_cwd
            .join(".ccterm/threads")
            .join(sanitize_thread_id(thread_id));
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create thread dir: {}", dir.display()))?;

        let claude_dir = dir.join(".claude");
        std::fs::create_dir_all(&claude_dir)
            .with_context(|| format!("failed to create .claude dir: {}", claude_dir.display()))?;
        let settings_path = claude_dir.join("settings.json");
        if !settings_path.exists() {
            let settings = self.render_thread_settings()?;
            std::fs::write(&settings_path, settings).with_context(|| {
                format!(
                    "failed to write thread settings.json: {}",
                    settings_path.display()
                )
            })?;
        }
        Ok(normalize_path(dir))
    }

    fn render_thread_settings(&self) -> Result<String> {
        let mut settings: Value = serde_json::from_str(&self.settings_template)
            .context("failed to parse base settings.json")?;
        let exe_path = self.ccterm_path.to_string_lossy();
        rewrite_hook_commands(&mut settings, &exe_path);
        let mut out =
            serde_json::to_string_pretty(&settings).context("failed to render settings.json")?;
        out.push('\n');
        Ok(out)
    }
}

fn sanitize_thread_id(thread_id: &str) -> String {
    thread_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn normalize_path(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn rewrite_hook_commands(settings: &mut Value, exe_path: &str) {
    let hooks = match settings.get_mut("hooks").and_then(Value::as_object_mut) {
        Some(hooks) => hooks,
        None => return,
    };

    for entry in hooks.values_mut() {
        let Some(entries) = entry.as_array_mut() else {
            continue;
        };
        for entry in entries.iter_mut() {
            let Some(hook_list) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
                continue;
            };
            for hook in hook_list.iter_mut() {
                let Some(command_value) = hook.get_mut("command") else {
                    continue;
                };
                let Some(command) = command_value.as_str() else {
                    continue;
                };
                let updated = replace_ccterm_command(command, exe_path);
                if updated != command {
                    *command_value = Value::String(updated);
                }
            }
        }
    }
}

fn replace_ccterm_command(command: &str, exe_path: &str) -> String {
    let debug_path = "$CLAUDE_PROJECT_DIR/target/debug/ccterm";
    let release_path = "$CLAUDE_PROJECT_DIR/target/release/ccterm";
    if command.contains(debug_path) {
        return command.replace(debug_path, exe_path);
    }
    if command.contains(release_path) {
        return command.replace(release_path, exe_path);
    }
    command.to_string()
}

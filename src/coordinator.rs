use crate::config::Config;
use crate::context;
use crate::hooks::HookEvent;
use crate::sessions::{self, TmuxSessionManager};
use crate::slack_adapter::SlackAdapter;
use crate::types::{IncomingMessage, OutgoingMessage};
use anyhow::{Context, Result};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
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
}

#[derive(Debug)]
struct PendingRequest {
    key: ConversationKey,
    suppress_output: bool,
}

pub struct Coordinator {
    config: Config,
    sessions: TmuxSessionManager,
    slack: SlackAdapter,
    hooks_rx: mpsc::UnboundedReceiver<HookEvent>,
    pending: VecDeque<PendingRequest>,
    sessions_by_key: HashMap<ConversationKey, SessionEntry>,
    main_by_conversation: HashMap<String, ConversationKey>,
}

impl Coordinator {
    pub fn new(
        config: Config,
        sessions: TmuxSessionManager,
        slack: SlackAdapter,
        hooks_rx: mpsc::UnboundedReceiver<HookEvent>,
    ) -> Self {
        Self {
            config,
            sessions,
            slack,
            hooks_rx,
            pending: VecDeque::new(),
            sessions_by_key: HashMap::new(),
            main_by_conversation: HashMap::new(),
        }
    }

    pub async fn run(mut self) -> Result<()> {
        let hook_timeout = Duration::from_secs(self.config.coordinator.hook_timeout_secs);
        let prompt_timeout = Duration::from_millis(self.config.coordinator.prompt_timeout_ms);

        loop {
            tokio::select! {
                maybe_msg = self.slack.incoming().recv() => {
                    let msg = match maybe_msg {
                        Some(m) => m,
                        None => break,
                    };
                    if let Err(err) = self.handle_incoming(msg, prompt_timeout, hook_timeout).await {
                        eprintln!("incoming error: {err}");
                    }
                }
                maybe_hook = self.hooks_rx.recv() => {
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

    async fn handle_incoming(
        &mut self,
        msg: IncomingMessage,
        prompt_timeout: Duration,
        _hook_timeout: Duration,
    ) -> Result<()> {
        let key = ConversationKey {
            conversation_id: msg.conversation_id.clone(),
            thread_id: msg.thread_id.clone(),
        };

        if msg.thread_id.is_none() {
            let entry = self.ensure_main_session(&msg, prompt_timeout)?;
            self.enqueue_send(&entry.session_name, msg.text, &key, false, prompt_timeout)?;
        } else {
            let entry = self.ensure_thread_session(&msg, prompt_timeout)?;
            self.enqueue_send(&entry.session_name, msg.text, &key, false, prompt_timeout)?;
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

        let session_name = sessions::timestamp_session_name(&self.config.tmux.session_prefix)?;
        self.sessions
            .spawn(&session_name)
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
        };
        self.sessions_by_key.insert(key, entry.clone());
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

        let session_name = sessions::timestamp_session_name(&self.config.tmux.session_prefix)?;
        self.sessions
            .spawn(&session_name)
            .with_context(|| format!("failed to spawn thread session {session_name}"))?;

        sessions::wait_for_prompt(
            &self.sessions,
            &session_name,
            prompt_timeout,
            Duration::from_millis(200),
        )?;

        if let Some(seed) = self.build_thread_seed(msg)? {
            self.enqueue_send(
                &session_name,
                seed,
                &key,
                true,
                prompt_timeout,
            )?;
        }

        let entry = SessionEntry {
            session_name: session_name.clone(),
            last_transcript_path: None,
        };
        self.sessions_by_key.insert(key.clone(), entry.clone());
        Ok(entry)
    }

    fn build_thread_seed(&self, msg: &IncomingMessage) -> Result<Option<String>> {
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
        Ok(context::format_history_prompt(&history))
    }

    fn enqueue_send(
        &mut self,
        session_name: &str,
        text: String,
        key: &ConversationKey,
        suppress_output: bool,
        prompt_timeout: Duration,
    ) -> Result<()> {
        sessions::wait_for_prompt(
            &self.sessions,
            session_name,
            prompt_timeout,
            Duration::from_millis(200),
        )?;
        self.sessions
            .send(session_name, &text)
            .with_context(|| format!("failed to send to {session_name}"))?;
        self.pending.push_back(PendingRequest {
            key: key.clone(),
            suppress_output,
        });
        Ok(())
    }

    async fn handle_hook(&mut self, hook: HookEvent) -> Result<()> {
        let pending = match self.pending.pop_front() {
            Some(p) => p,
            None => {
                eprintln!(
                    "received hook with no pending request: {} ({})",
                    hook.event_name,
                    hook.session_id
                );
                return Ok(());
            }
        };

        if let Some(entry) = self.sessions_by_key.get_mut(&pending.key) {
            entry.last_transcript_path = Some(hook.transcript_path.clone());
        }

        if pending.suppress_output {
            return Ok(());
        }

        let assistant_text =
            context::latest_assistant_text(&hook.transcript_path)?.unwrap_or_default();

        if assistant_text.trim().is_empty() {
            return Ok(());
        }

        let outgoing = OutgoingMessage {
            text: assistant_text,
            conversation_id: pending.key.conversation_id.clone(),
            thread_id: pending.key.thread_id.clone(),
        };

        self.slack.send(&outgoing).await?;
        Ok(())
    }
}

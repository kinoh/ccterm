use crate::types::{IncomingMessage, OutgoingMessage};
use anyhow::{bail, Result};

const DEFAULT_CONVERSATION_ID: &str = "cli";

pub fn parse_input(line: &str) -> Result<IncomingMessage> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        bail!("empty input");
    }

    if let Some(rest) = trimmed.strip_prefix("thread:") {
        let rest = rest.trim_start();
        let mut parts = rest.splitn(2, char::is_whitespace);
        let thread_id = parts
            .next()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("thread id is required after thread:"))?;
        let text = parts
            .next()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("message text is required after thread id"))?;
        return Ok(IncomingMessage {
            text,
            conversation_id: DEFAULT_CONVERSATION_ID.to_string(),
            thread_id: Some(thread_id),
            timestamp: None,
        });
    }

    Ok(IncomingMessage {
        text: trimmed.to_string(),
        conversation_id: DEFAULT_CONVERSATION_ID.to_string(),
        thread_id: None,
        timestamp: None,
    })
}

pub fn pretty_outgoing(message: &OutgoingMessage) -> Result<String> {
    let json = serde_json::to_string_pretty(message)?;
    Ok(json)
}

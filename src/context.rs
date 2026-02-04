use crate::types::{Role, TranscriptMessage};
use anyhow::{Context, Result};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn read_history(path: &Path, cutoff_ts: Option<&str>) -> Result<Vec<TranscriptMessage>> {
    let file = File::open(path)
        .with_context(|| format!("failed to open transcript: {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut out = Vec::new();
    for line in reader.lines() {
        let line = line.context("failed to read transcript line")?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(&line).with_context(|| "failed to parse transcript JSON")?;
        let msg = parse_transcript_line(&value, cutoff_ts)?;
        if let Some(msg) = msg {
            out.push(msg);
        }
    }
    Ok(out)
}

pub fn latest_assistant_text(path: &Path) -> Result<Option<String>> {
    let history = read_history(path, None)?;
    let text = history
        .into_iter()
        .rev()
        .find(|msg| matches!(msg.role, Role::Assistant))
        .map(|msg| msg.text);
    Ok(text)
}

pub fn format_history_context(history: &[TranscriptMessage]) -> Option<String> {
    if history.is_empty() {
        return None;
    }

    let mut out = String::new();
    out.push_str("# Optional Conversation Context\n\n");
    out.push_str(
        "This file provides background context to help interpret the user's next message.\n",
    );
    out.push_str("You do not need to focus on it unless it is useful.\n\n");
    out.push_str("## Prior Messages\n");
    for msg in history {
        match msg.role {
            Role::User => {
                out.push_str("User: ");
                out.push_str(&msg.text);
            }
            Role::Assistant => {
                out.push_str("Assistant: ");
                out.push_str(&msg.text);
            }
        }
        out.push('\n');
    }
    out.push('\n');
    Some(out)
}

fn parse_transcript_line(value: &Value, cutoff_ts: Option<&str>) -> Result<Option<TranscriptMessage>> {
    let line_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if line_type != "user" && line_type != "assistant" {
        return Ok(None);
    }

    let timestamp = value.get("timestamp").and_then(Value::as_str);
    if let (Some(cutoff), Some(ts)) = (cutoff_ts, timestamp) {
        if ts > cutoff {
            return Ok(None);
        }
    }

    let message = value.get("message").unwrap_or(&Value::Null);
    let content = message.get("content").unwrap_or(&Value::Null);

    let text = match line_type {
        "user" => extract_user_text(content),
        "assistant" => extract_assistant_text(content),
        _ => None,
    };

    let text = match text {
        Some(text) if !text.trim().is_empty() => text,
        _ => return Ok(None),
    };

    let role = if line_type == "user" {
        Role::User
    } else {
        Role::Assistant
    };

    Ok(Some(TranscriptMessage {
        role,
        text,
    }))
}

fn extract_user_text(content: &Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }
    if let Some(items) = content.as_array() {
        let mut out = String::new();
        for item in items {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                out.push_str(text);
            }
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    None
}

fn extract_assistant_text(content: &Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }
    if let Some(items) = content.as_array() {
        let mut out = String::new();
        for item in items {
            if item.get("type").and_then(Value::as_str) != Some("text") {
                continue;
            }
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                out.push_str(text);
            }
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    None
}

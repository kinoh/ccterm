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
    let cutoff = cutoff_ts.and_then(parse_slack_ts_to_nanos);
    if cutoff_ts.is_some() && cutoff.is_none() {
        eprintln!("history cutoff ignored due to invalid Slack timestamp");
    }

    let mut out = Vec::new();
    for line in reader.lines() {
        let line = line.context("failed to read transcript line")?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(&line).with_context(|| "failed to parse transcript JSON")?;
        let msg = parse_transcript_line(&value, cutoff)?;
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

pub fn latest_assistant_text_uuid(path: &Path) -> Result<Option<(String, String)>> {
    let file = File::open(path)
        .with_context(|| format!("failed to open transcript: {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut latest: Option<(String, String)> = None;
    for line in reader.lines() {
        let line = line.context("failed to read transcript line")?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(&line).with_context(|| "failed to parse transcript JSON")?;
        let line_type = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if line_type != "assistant" {
            continue;
        }
        let message = value.get("message").unwrap_or(&Value::Null);
        let content = message.get("content").unwrap_or(&Value::Null);
        let text = extract_assistant_text(content);
        let text = match text {
            Some(text) if !text.trim().is_empty() => text,
            _ => continue,
        };
        let uuid = match value.get("uuid").and_then(Value::as_str) {
            Some(uuid) => uuid.to_string(),
            None => continue,
        };
        latest = Some((uuid, text));
    }
    Ok(latest)
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

fn parse_transcript_line(value: &Value, cutoff_ts: Option<i128>) -> Result<Option<TranscriptMessage>> {
    let line_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if line_type != "user" && line_type != "assistant" {
        return Ok(None);
    }

    let timestamp = value.get("timestamp").and_then(Value::as_str);
    if let (Some(cutoff), Some(ts)) = (cutoff_ts, timestamp) {
        if let Some(ts_nanos) = parse_iso_ts_to_nanos(ts) {
            if ts_nanos > cutoff {
                return Ok(None);
            }
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

fn parse_slack_ts_to_nanos(ts: &str) -> Option<i128> {
    let (secs, frac) = ts.split_once('.')?;
    let secs: i128 = secs.parse().ok()?;
    let nanos = parse_fractional_nanos(frac)?;
    Some(secs * 1_000_000_000 + nanos)
}

fn parse_iso_ts_to_nanos(ts: &str) -> Option<i128> {
    let ts = ts.strip_suffix('Z')?;
    let (date, time) = ts.split_once('T')?;
    let mut date_parts = date.splitn(3, '-');
    let year: i32 = date_parts.next()?.parse().ok()?;
    let month: i32 = date_parts.next()?.parse().ok()?;
    let day: i32 = date_parts.next()?.parse().ok()?;

    let (time_part, frac_part) = match time.split_once('.') {
        Some(parts) => parts,
        None => (time, ""),
    };
    let mut time_parts = time_part.splitn(3, ':');
    let hour: i32 = time_parts.next()?.parse().ok()?;
    let minute: i32 = time_parts.next()?.parse().ok()?;
    let second: i32 = time_parts.next()?.parse().ok()?;
    let nanos = if frac_part.is_empty() {
        0
    } else {
        parse_fractional_nanos(frac_part)?
    };

    let days = days_from_civil(year, month, day);
    let seconds = (days as i128) * 86_400
        + (hour as i128) * 3_600
        + (minute as i128) * 60
        + (second as i128);
    Some(seconds * 1_000_000_000 + nanos)
}

fn parse_fractional_nanos(frac: &str) -> Option<i128> {
    if frac.is_empty() {
        return Some(0);
    }
    let mut digits = frac.as_bytes();
    if digits.len() > 9 {
        digits = &digits[..9];
    }
    let mut value: i128 = 0;
    for &b in digits {
        if !(b'0'..=b'9').contains(&b) {
            return None;
        }
        value = value * 10 + i128::from(b - b'0');
    }
    let scale = 10_i128.pow((9 - digits.len()) as u32);
    Some(value * scale)
}

fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
    let mut y = year;
    let mut m = month;
    y -= if m <= 2 { 1 } else { 0 };
    m = if m > 2 { m - 3 } else { m + 9 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146097 + doe - 719468) as i64
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

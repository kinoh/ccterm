use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub fn append_stdin_to_file(out_path: &Path) -> Result<()> {
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create hook output dir: {}", parent.display()))?;
    }

    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("failed to read hook payload from stdin")?;

    if !input.ends_with('\n') {
        input.push('\n');
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(out_path)
        .with_context(|| format!("failed to open hook output: {}", out_path.display()))?;

    file.write_all(input.as_bytes())
        .context("failed to append hook payload")?;
    file.flush().context("failed to flush hook output")?;
    Ok(())
}

pub struct HookFollower {
    reader: BufReader<File>,
}

impl HookFollower {
    pub fn open(path: &Path, follow_from_end: bool) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create hook output dir: {}", parent.display()))?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)
            .with_context(|| format!("failed to open hook output: {}", path.display()))?;

        if follow_from_end {
            file.seek(SeekFrom::End(0))
                .context("failed to seek hook output")?;
        }

        Ok(Self {
            reader: BufReader::new(file),
        })
    }

    pub fn wait_for_line(&mut self, timeout: Duration) -> Result<String> {
        let start = Instant::now();
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = self
                .reader
                .read_line(&mut buf)
                .context("failed reading hook output")?;
            if read > 0 {
                return Ok(buf.trim_end().to_string());
            }
            if start.elapsed() > timeout {
                bail!("timed out waiting for hook event");
            }
            thread::sleep(Duration::from_millis(200));
        }
    }
}

#[derive(Debug, Clone)]
pub struct HookEvent {
    pub event_name: String,
    pub session_id: String,
    pub transcript_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct HookPayload {
    #[serde(rename = "hook_event_name")]
    event_name: String,
    session_id: String,
    transcript_path: Option<String>,
    agent_transcript_path: Option<String>,
}

pub fn parse_hook_line(line: &str) -> Result<HookEvent> {
    let payload: HookPayload =
        serde_json::from_str(line).context("failed to parse hook json")?;
    let transcript_path = payload
        .transcript_path
        .or(payload.agent_transcript_path)
        .context("missing transcript_path")?;

    Ok(HookEvent {
        event_name: payload.event_name,
        session_id: payload.session_id,
        transcript_path: PathBuf::from(transcript_path),
    })
}

pub fn spawn_hook_receiver(path: PathBuf) -> mpsc::UnboundedReceiver<HookEvent> {
    let (tx, rx) = mpsc::unbounded_channel();

    thread::spawn(move || {
        let mut follower = match HookFollower::open(&path, true) {
            Ok(f) => f,
            Err(err) => {
                eprintln!("hook receiver failed to open: {err}");
                return;
            }
        };

        loop {
            match follower.wait_for_line(Duration::from_secs(3600)) {
                Ok(line) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    match parse_hook_line(&line) {
                        Ok(event) => {
                            let _ = tx.send(event);
                        }
                        Err(err) => {
                            eprintln!("hook receiver parse error: {err}");
                        }
                    }
                }
                Err(err) => {
                    eprintln!("hook receiver error: {err}");
                }
            }
        }
    });

    rx
}

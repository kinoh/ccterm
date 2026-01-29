use anyhow::{bail, Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

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

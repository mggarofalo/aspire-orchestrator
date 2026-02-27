use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

/// The source of a log line: agent output or Aspire process output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    Agent,
    Aspire,
}

/// A log line event with the slot name, source, and the line text.
#[derive(Debug, Clone)]
pub struct LogLine {
    pub slot_name: String,
    pub source: LogSource,
    pub line: String,
}

/// Start an async file tailer that polls for new content and sends lines over a channel.
///
/// Returns a `JoinHandle` that can be aborted to stop tailing.
pub fn start_tailing(
    file_path: PathBuf,
    slot_name: String,
    source: LogSource,
    tx: mpsc::UnboundedSender<LogLine>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut last_position: u64 = 0;

        // Wait for file to exist
        while !file_path.exists() {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        let mut tick = interval(Duration::from_millis(250));
        loop {
            tick.tick().await;

            if let Ok(new_pos) =
                read_new_content(&file_path, last_position, &slot_name, source, &tx)
            {
                last_position = new_pos;
            }

            if tx.is_closed() {
                break;
            }
        }
    })
}

fn read_new_content(
    path: &Path,
    last_position: u64,
    slot_name: &str,
    source: LogSource,
    tx: &mpsc::UnboundedSender<LogLine>,
) -> std::io::Result<u64> {
    let mut file = std::fs::OpenOptions::new().read(true).open(path)?;

    let metadata = file.metadata()?;
    if metadata.len() <= last_position {
        return Ok(last_position);
    }

    file.seek(SeekFrom::Start(last_position))?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let new_position = file.stream_position()?;

    for line in content.lines() {
        if !line.is_empty() {
            let _ = tx.send(LogLine {
                slot_name: slot_name.to_string(),
                source,
                line: line.to_string(),
            });
        }
    }

    Ok(new_position)
}

/// Read the last N lines of a file (non-async, for initial load).
pub fn read_tail(file_path: &Path, line_count: usize) -> String {
    match std::fs::read_to_string(file_path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = lines.len().saturating_sub(line_count);
            lines[start..].join("\n")
        }
        Err(_) => String::new(),
    }
}

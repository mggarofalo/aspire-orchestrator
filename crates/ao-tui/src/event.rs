use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent};
use tokio::sync::mpsc;

use ao_core::models::RepoCandidate;
use ao_core::services::log_tailer::LogLine;

/// Events flowing into the main loop.
#[derive(Debug)]
pub enum AppEvent {
    /// A key press from the user.
    Key(KeyEvent),
    /// Periodic tick for refreshing slot state.
    Tick,
    /// A new log line from a slot's log tailer or aspire process.
    Log(LogLine),
    /// An async operation produced an error to display.
    Error(String),
    /// An async operation completed successfully with a message.
    Info(String),
    /// Repo scan completed with discovered candidates.
    RepoCandidatesLoaded(Vec<RepoCandidate>),
    /// Blueprint names loaded from disk.
    BlueprintNamesLoaded(Vec<String>),
    /// Progress update from a batch operation.
    BatchProgress {
        completed: usize,
        total: usize,
        current_slot: Option<String>,
        failure: Option<(String, String)>,
        done: bool,
    },
}

/// Spawn the crossterm input polling task.
pub fn spawn_input_task(tx: mpsc::UnboundedSender<AppEvent>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            // Poll crossterm events with 50ms timeout (non-blocking feel)
            let has_event = tokio::task::spawn_blocking(|| {
                event::poll(Duration::from_millis(50)).unwrap_or(false)
            })
            .await
            .unwrap_or(false);

            if has_event {
                if let Ok(Event::Key(key)) = tokio::task::spawn_blocking(event::read)
                    .await
                    .unwrap_or(Err(std::io::Error::other("spawn_blocking failed")))
                {
                    if tx.send(AppEvent::Key(key)).is_err() {
                        break;
                    }
                }
            }
        }
    })
}

/// Spawn the periodic tick task.
pub fn spawn_tick_task(tx: mpsc::UnboundedSender<AppEvent>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            if tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    })
}

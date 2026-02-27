//! Agent host process: creates a PTY, spawns the agent command inside it,
//! listens on localhost TCP, and streams output to connected clients and a log file.

use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::sync::Arc;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};

use ao_core::services::agent_host::{
    AgentHostInfo, CHANNEL_CONTROL, CHANNEL_PTY_INPUT, CHANNEL_PTY_OUTPUT,
};

/// Run the agent host process. This is the entry point when `--host-agent` is passed.
///
/// Creates a PTY, spawns `command` inside it, listens on a TCP port,
/// writes output to `log_file`, and streams to connected TUI clients.
pub async fn run_host(
    slot_name: &str,
    command: &[String],
    workdir: &str,
    log_file: &str,
    slots_dir: &str,
) -> color_eyre::Result<()> {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| color_eyre::eyre::eyre!("openpty: {e}"))?;

    // Build and spawn the command in the PTY
    let mut cmd = CommandBuilder::new(&command[0]);
    if command.len() > 1 {
        cmd.args(&command[1..]);
    }
    cmd.cwd(workdir);

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| color_eyre::eyre::eyre!("spawn command: {e}"))?;

    // Drop the slave after spawning — the master end is what we use.
    drop(pair.slave);

    // Set up TCP listener on localhost (OS-assigned port)
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let pid = std::process::id();

    // Write the host info file
    let info = AgentHostInfo { port, pid };
    let info_dir = PathBuf::from(slots_dir).join(slot_name);
    tokio::fs::create_dir_all(&info_dir).await?;
    let info_path = info_dir.join(".agent-host.json");
    let info_json = serde_json::to_string(&info)?;
    tokio::fs::write(&info_path, &info_json).await?;

    // Open the log file for appending
    let log_path = PathBuf::from(log_file);
    if let Some(parent) = log_path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }

    // Broadcast channel for PTY output → all connected clients
    let (output_tx, _) = broadcast::channel::<Vec<u8>>(256);

    // Read from PTY master in a blocking thread and broadcast + write to log
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| color_eyre::eyre::eyre!("clone reader: {e}"))?;
    let output_tx_clone = output_tx.clone();
    let log_path_clone = log_path.clone();

    let pty_read_task = tokio::task::spawn_blocking(move || {
        let mut reader = reader;
        let mut buf = [0u8; 4096];
        let mut log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path_clone)
            .ok();

        loop {
            match std::io::Read::read(&mut reader, &mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    // Write to log file
                    if let Some(ref mut f) = log_file {
                        let _ = f.write_all(&data);
                        let _ = f.flush();
                    }
                    // Broadcast to connected clients (ignore if no receivers)
                    let _ = output_tx_clone.send(data);
                }
                Err(_) => break,
            }
        }
    });

    // PTY writer (shared for input from any client)
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| color_eyre::eyre::eyre!("take writer: {e}"))?;
    let pty_writer = Arc::new(Mutex::new(writer));
    let pty_master = Arc::new(Mutex::new(pair.master));

    // Accept TCP connections and handle them
    let accept_task = tokio::spawn({
        let output_tx = output_tx.clone();
        let pty_writer = pty_writer.clone();
        let pty_master = pty_master.clone();
        async move {
            while let Ok((stream, _)) = listener.accept().await {
                let rx = output_tx.subscribe();
                let writer = pty_writer.clone();
                let master = pty_master.clone();

                tokio::spawn(async move {
                    handle_client(stream, rx, writer, master).await;
                });
            }
        }
    });

    // Wait for the child process to exit
    let exit_status = tokio::task::spawn_blocking(move || child.wait())
        .await
        .map_err(|e| color_eyre::eyre::eyre!("wait: {e}"))?
        .map_err(|e| color_eyre::eyre::eyre!("child wait: {e}"))?;

    let exit_code = exit_status.exit_code();

    // Brief delay to let remaining output flush
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Send exit notification to any connected clients
    let exit_json = format!("{{\"exited\":{exit_code}}}");
    let mut frame = vec![CHANNEL_CONTROL];
    let len = exit_json.len().min(u16::MAX as usize) as u16;
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(exit_json.as_bytes());
    let _ = output_tx.send(frame);

    // Clean up
    accept_task.abort();
    pty_read_task.abort();
    let _ = tokio::fs::remove_file(&info_path).await;

    Ok(())
}

/// Handle a single TUI client connection.
async fn handle_client(
    stream: tokio::net::TcpStream,
    mut output_rx: broadcast::Receiver<Vec<u8>>,
    pty_writer: Arc<Mutex<Box<dyn IoWrite + Send>>>,
    pty_master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
) {
    let (mut reader, mut writer) = stream.into_split();

    // Task: forward PTY output to client as framed messages
    let writer_handle = tokio::spawn(async move {
        while let Ok(data) = output_rx.recv().await {
            // Frame: [channel] [len u16 LE] [payload]
            let mut frame = Vec::with_capacity(3 + data.len());
            frame.push(CHANNEL_PTY_OUTPUT);
            let len = data.len().min(u16::MAX as usize) as u16;
            frame.extend_from_slice(&len.to_le_bytes());
            frame.extend_from_slice(&data[..len as usize]);
            if writer.write_all(&frame).await.is_err() {
                break;
            }
        }
    });

    // Read frames from client
    while let Ok(channel) = reader.read_u8().await {
        let Ok(len) = reader.read_u16_le().await else {
            break;
        };
        let mut payload = vec![0u8; len as usize];
        if reader.read_exact(&mut payload).await.is_err() {
            break;
        }

        match channel {
            CHANNEL_PTY_INPUT => {
                let mut w = pty_writer.lock().await;
                let _ = w.write_all(&payload);
                let _ = w.flush();
            }
            CHANNEL_CONTROL => {
                if let Ok(text) = std::str::from_utf8(&payload) {
                    handle_control_message(text, &pty_master).await;
                }
            }
            _ => {}
        }
    }

    writer_handle.abort();
}

/// Handle a JSON control message from a client.
async fn handle_control_message(
    text: &str,
    pty_master: &Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
) {
    let parsed: std::result::Result<serde_json::Value, _> = serde_json::from_str(text);
    if let Ok(value) = parsed {
        if let Some(resize) = value.get("resize") {
            if let Some(arr) = resize.as_array() {
                if arr.len() == 2 {
                    let cols = arr[0].as_u64().unwrap_or(120) as u16;
                    let rows = arr[1].as_u64().unwrap_or(40) as u16;
                    let master = pty_master.lock().await;
                    let _ = master.resize(PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    });
                }
            }
        } else if value.as_str() == Some("kill") {
            std::process::exit(1);
        }
    }
}

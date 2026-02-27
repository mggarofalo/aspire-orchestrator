use std::path::{Path, PathBuf};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::error::{OrchestratorError, Result};

/// Metadata written by the agent host process to `.agent-host.json`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AgentHostInfo {
    pub port: u16,
    pub pid: u32,
}

/// IPC channel identifiers for the binary framing protocol.
pub const CHANNEL_PTY_OUTPUT: u8 = 0x01;
pub const CHANNEL_PTY_INPUT: u8 = 0x02;
pub const CHANNEL_CONTROL: u8 = 0x03;

/// A connection to a running agent host process.
pub struct AgentConnection {
    stream: TcpStream,
}

impl AgentConnection {
    /// Read one frame: [channel: u8] [len: u16 LE] [payload].
    /// Returns (channel, payload).
    pub async fn read_frame(&mut self) -> Result<(u8, Vec<u8>)> {
        let channel = self
            .stream
            .read_u8()
            .await
            .map_err(|e| OrchestratorError::AgentHost(format!("read channel: {e}")))?;
        let len = self
            .stream
            .read_u16_le()
            .await
            .map_err(|e| OrchestratorError::AgentHost(format!("read len: {e}")))?;
        let mut buf = vec![0u8; len as usize];
        self.stream
            .read_exact(&mut buf)
            .await
            .map_err(|e| OrchestratorError::AgentHost(format!("read payload: {e}")))?;
        Ok((channel, buf))
    }

    /// Send PTY input bytes to the agent host.
    pub async fn send_input(&mut self, data: &[u8]) -> Result<()> {
        self.write_frame(CHANNEL_PTY_INPUT, data).await
    }

    /// Send a resize control command.
    pub async fn send_resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        let json = format!("{{\"resize\":[{cols},{rows}]}}");
        self.write_frame(CHANNEL_CONTROL, json.as_bytes()).await
    }

    /// Send a kill control command.
    pub async fn send_kill(&mut self) -> Result<()> {
        self.write_frame(CHANNEL_CONTROL, b"\"kill\"").await
    }

    async fn write_frame(&mut self, channel: u8, payload: &[u8]) -> Result<()> {
        let len = payload.len().min(u16::MAX as usize) as u16;
        self.stream
            .write_u8(channel)
            .await
            .map_err(|e| OrchestratorError::AgentHost(format!("write channel: {e}")))?;
        self.stream
            .write_u16_le(len)
            .await
            .map_err(|e| OrchestratorError::AgentHost(format!("write len: {e}")))?;
        self.stream
            .write_all(&payload[..len as usize])
            .await
            .map_err(|e| OrchestratorError::AgentHost(format!("write payload: {e}")))?;
        self.stream
            .flush()
            .await
            .map_err(|e| OrchestratorError::AgentHost(format!("flush: {e}")))?;
        Ok(())
    }
}

/// Spawn an agent host process (detached, survives parent exit).
///
/// This launches the same `ao-tui` binary with `--host-agent` arguments.
/// The host process writes its port and PID to `{slots_dir}/{name}/.agent-host.json`.
pub async fn spawn(
    name: &str,
    command: &[String],
    workdir: &str,
    log_file: &str,
    slots_dir: &Path,
) -> Result<()> {
    let exe = std::env::current_exe()
        .map_err(|e| OrchestratorError::AgentHost(format!("current_exe: {e}")))?;

    let host_dir = slots_dir.join(name);
    tokio::fs::create_dir_all(&host_dir)
        .await
        .map_err(|e| OrchestratorError::AgentHost(format!("create host dir: {e}")))?;

    // Use std::process::Command (not tokio) for detached spawning —
    // tokio's async pipe setup conflicts with DETACHED_PROCESS on Windows.
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("--host-agent")
        .arg("--slot")
        .arg(name)
        .arg("--workdir")
        .arg(workdir)
        .arg("--log-file")
        .arg(log_file)
        .arg("--slots-dir")
        .arg(slots_dir.to_string_lossy().as_ref())
        .arg("--");

    for arg in command {
        cmd.arg(arg);
    }

    // Detach: redirect stdio to null so the child survives parent exit.
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
        cmd.creation_flags(0x08000000 | 0x00000200);
    }

    cmd.spawn()
        .map_err(|e| OrchestratorError::AgentHost(format!("spawn host: {e}")))?;

    // Wait briefly for the host to write its info file.
    let info_path = host_dir.join(".agent-host.json");
    for _ in 0..40 {
        if info_path.exists() {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }

    Err(OrchestratorError::AgentHost(
        "host process did not start in time".into(),
    ))
}

/// Connect to a running agent host's TCP stream.
pub async fn connect(name: &str, slots_dir: &Path) -> Result<AgentConnection> {
    let info = read_host_info(name, slots_dir).await?;
    let stream = TcpStream::connect(format!("127.0.0.1:{}", info.port))
        .await
        .map_err(|e| OrchestratorError::AgentHost(format!("connect to host: {e}")))?;
    Ok(AgentConnection { stream })
}

/// List running agent host processes by scanning `.agent-host.json` files.
pub async fn list_running(slots_dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    let mut entries = tokio::fs::read_dir(slots_dir)
        .await
        .map_err(|e| OrchestratorError::AgentHost(format!("read slots dir: {e}")))?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| OrchestratorError::AgentHost(format!("read entry: {e}")))?
    {
        let path = entry.path();
        if path.is_dir() {
            let info_file = path.join(".agent-host.json");
            if info_file.exists() {
                if let Ok(info) = read_host_info_from_path(&info_file).await {
                    if is_pid_alive(info.pid) {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            names.push(name.to_string());
                        }
                    } else {
                        // Stale info file — clean up.
                        let _ = tokio::fs::remove_file(&info_file).await;
                    }
                }
            }
        }
    }

    Ok(names)
}

/// Kill an agent host process by PID.
pub async fn kill(name: &str, slots_dir: &Path) -> Result<()> {
    let info_path = slots_dir.join(name).join(".agent-host.json");
    if let Ok(info) = read_host_info_from_path(&info_path).await {
        kill_pid(info.pid);
    }
    let _ = tokio::fs::remove_file(&info_path).await;
    Ok(())
}

/// Check if a specific agent host is running.
pub async fn is_running(name: &str, slots_dir: &Path) -> Result<bool> {
    let info_path = slots_dir.join(name).join(".agent-host.json");
    if !info_path.exists() {
        return Ok(false);
    }
    match read_host_info_from_path(&info_path).await {
        Ok(info) => Ok(is_pid_alive(info.pid)),
        Err(_) => Ok(false),
    }
}

async fn read_host_info(name: &str, slots_dir: &Path) -> Result<AgentHostInfo> {
    let info_path = slots_dir.join(name).join(".agent-host.json");
    read_host_info_from_path(&info_path).await
}

async fn read_host_info_from_path(path: &PathBuf) -> Result<AgentHostInfo> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| OrchestratorError::AgentHost(format!("read host info: {e}")))?;
    serde_json::from_str(&content)
        .map_err(|e| OrchestratorError::AgentHost(format!("parse host info: {e}")))
}

fn is_pid_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::io::FromRawHandle;
        unsafe {
            let handle = windows_sys::Win32::System::Threading::OpenProcess(0x00100000, 0, pid); // SYNCHRONIZE
            if handle.is_null() {
                false
            } else {
                let _ = std::os::windows::io::OwnedHandle::from_raw_handle(handle as *mut _);
                true
            }
        }
    }
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
}

fn kill_pid(pid: u32) {
    #[cfg(windows)]
    {
        use std::os::windows::io::FromRawHandle;
        unsafe {
            let handle = windows_sys::Win32::System::Threading::OpenProcess(0x0001, 0, pid); // PROCESS_TERMINATE
            if !handle.is_null() {
                windows_sys::Win32::System::Threading::TerminateProcess(handle, 1);
                let _ = std::os::windows::io::OwnedHandle::from_raw_handle(handle as *mut _);
            }
        }
    }
    #[cfg(unix)]
    {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }
}

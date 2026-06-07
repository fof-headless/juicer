/// Manages the headless Blender subprocess and the TCP socket bridge to it.
///
/// Blender runs: `blender --background --python juicer_bridge.py`
/// The Python bridge opens a TCP server on 127.0.0.1:6789.
/// We send newline-delimited JSON commands and receive JSON responses.

use anyhow::{Context, Result};
use serde_json::Value;
use std::process::{Child, Command, Stdio};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    time::{sleep, Duration},
};

const BRIDGE_PORT: u16 = 6789;
const CONNECT_RETRIES: u8 = 15;
const RETRY_DELAY_MS: u64 = 500;

pub struct BlenderBridge {
    process: Option<Child>,
    connected: bool,
}

impl BlenderBridge {
    pub fn new() -> Self {
        Self {
            process: None,
            connected: false,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub async fn start(&mut self, blender_path: &str, bridge_script: &str) -> Result<()> {
        // Kill any previous instance
        self.stop();

        let child = Command::new(blender_path)
            .args(["--background", "--python", bridge_script])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to spawn Blender at '{blender_path}'"))?;

        self.process = Some(child);

        // Wait for the Python bridge to open its socket
        for i in 0..CONNECT_RETRIES {
            sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
            if self.ping().await.is_ok() {
                self.connected = true;
                return Ok(());
            }
            eprintln!("[Blender] Waiting for bridge... attempt {}/{CONNECT_RETRIES}", i + 1);
        }

        anyhow::bail!("Timed out waiting for Blender bridge on port {BRIDGE_PORT}")
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
        }
        self.connected = false;
    }

    async fn ping(&self) -> Result<()> {
        let cmd = serde_json::json!({ "op": "ping" });
        self.raw_send(&cmd.to_string()).await?;
        Ok(())
    }

    /// Send a JSON command to the Blender bridge and return the response.
    pub async fn send_command(&mut self, json_str: &str) -> Result<Value> {
        match self.raw_send(json_str).await {
            Ok(resp) => Ok(resp),
            Err(e) => {
                self.connected = false;
                Err(e)
            }
        }
    }

    async fn raw_send(&self, json_str: &str) -> Result<Value> {
        let mut stream = TcpStream::connect(("127.0.0.1", BRIDGE_PORT))
            .await
            .context("Cannot connect to Blender bridge — is Blender running?")?;

        let mut msg = json_str.to_string();
        msg.push('\n');
        stream.write_all(msg.as_bytes()).await?;
        stream.flush().await?;

        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response).await?;

        let val: Value = serde_json::from_str(response.trim())?;
        Ok(val)
    }
}

impl Drop for BlenderBridge {
    fn drop(&mut self) {
        self.stop();
    }
}

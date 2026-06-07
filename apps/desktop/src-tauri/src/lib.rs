mod blender;
mod mcp;
mod html_capture;

use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{Manager, State};
use blender::BlenderBridge;

pub type BridgeState = Arc<Mutex<BlenderBridge>>;

#[tauri::command]
async fn blender_cmd(
    bridge: State<'_, BridgeState>,
    command: String,
) -> Result<serde_json::Value, String> {
    let mut b = bridge.lock().await;
    b.send_command(&command).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn blender_status(bridge: State<'_, BridgeState>) -> Result<bool, String> {
    let b = bridge.lock().await;
    Ok(b.is_connected())
}

#[tauri::command]
async fn start_blender(
    bridge: State<'_, BridgeState>,
    blender_path: String,
    bridge_script: String,
) -> Result<String, String> {
    let mut b = bridge.lock().await;
    b.start(&blender_path, &bridge_script)
        .await
        .map_err(|e| e.to_string())?;
    Ok("started".into())
}

#[tauri::command]
async fn capture_html(
    html: String,
    width: u32,
    height: u32,
    output_path: String,
) -> Result<String, String> {
    html_capture::capture_html_to_png(&html, width, height, &output_path)
        .await
        .map_err(|e| e.to_string())?;
    Ok(output_path)
}

pub fn run() {
    let bridge = Arc::new(Mutex::new(BlenderBridge::new()));
    let bridge_for_mcp = bridge.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(bridge)
        .invoke_handler(tauri::generate_handler![
            blender_cmd,
            blender_status,
            start_blender,
            capture_html,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // Spawn the MCP server as a background task
            tokio::spawn(async move {
                if let Err(e) = mcp::run_mcp_server(bridge_for_mcp, app_handle).await {
                    eprintln!("[MCP] Server error: {e}");
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Juicer");
}

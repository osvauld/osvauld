use crate::application::services::P2PService;
use crate::types::CryptoResponse;
use std::sync::Arc;
use sys_locale::get_locale;
use tauri::State;

#[tauri::command]
pub async fn send_message(
    message: String,
    state: State<'_, Arc<P2PService>>,
) -> Result<CryptoResponse, String> {
    state.send_chat_message(message).await
}

#[tauri::command]
pub async fn get_ticket(state: State<'_, Arc<P2PService>>) -> Result<String, String> {
    state.get_connection_ticket().await
}

#[tauri::command]
pub async fn connect_with_ticket(
    ticket: String,
    state: State<'_, Arc<P2PService>>,
) -> Result<(), String> {
    state.connect_with_ticket(&ticket).await
}

#[tauri::command]
pub async fn start_p2p_listener(
    state: State<'_, Arc<P2PService>>,
) -> Result<CryptoResponse, String> {
    state.start_listening().await
}

#[tauri::command]
pub fn get_system_locale() -> String {
    get_locale().unwrap_or_else(|| String::from("en-US"))
}

#[tauri::command]
pub async fn send_snapshot(
    snapshot: String,
    state: State<'_, Arc<P2PService>>,
) -> Result<(), CryptoResponse> {
    log::info!("snapshot recived {:?}", snapshot);
    let _ = state
        .send_snapshot(snapshot)
        .await
        .map_err(|e| CryptoResponse::Error(e));
    Ok(())
}

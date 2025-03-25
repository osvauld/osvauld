use crate::types::CryptoResponse;
use crate::types::InitiateFirstConnectionInput;
use log::info;
use osvauld_core::models::p2p::ConnectionType;
use osvauld_services::UserService;
use p2p_service::P2PService;
use rendezvous_client::rendezvous_service::RendezvousService;
use std::fmt::format;
use std::sync::Arc;
use sys_locale::get_locale;
use tauri::State;

#[tauri::command]
pub async fn send_message(
    message: String,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<CryptoResponse, String> {
    // match p2p_service.send_chat_message(message).await {
    //     Ok(_) => Ok(CryptoResponse::Success),
    //     Err(e) => Err(format!("{}", e)), // Use format! to convert any error to String
    // }
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn get_ticket(state: State<'_, Arc<P2PService>>) -> Result<String, String> {
    state
        .get_connection_ticket()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn connect_with_device(
    ticket: String,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<(), String> {
    p2p_service
        .connect_with_ticket(&ticket, ConnectionType::Device, None)
        .await?;
    // p2p_service.start_device_sync().await
    Ok(())
}

#[tauri::command]
pub async fn start_p2p_listener(
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<CryptoResponse, String> {
    match p2p_service.start_listening().await {
        Ok(_) => Ok(CryptoResponse::Success),
        Err(e) => Err(format!("{}", e)), // Use format! to convert any error to String
    }
}

#[tauri::command]
pub fn get_system_locale() -> String {
    get_locale().unwrap_or_else(|| String::from("en-US"))
}

#[tauri::command]
pub async fn initiate_first_connection(
    input: InitiateFirstConnectionInput,
    rendezvous_service: State<'_, Arc<RendezvousService>>,
) -> Result<CryptoResponse, String> {
    info!("recived first connection request {:?}", input);
    let connection_id = format!("{}:{}", input.user.id, input.device.id);
    match rendezvous_service
        .mark_for_first_connection(&connection_id)
        .await
    {
        Ok(_) => info!("requested connection.."),
        Err(e) => info!("error requesting {:?}", e),
    }
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn connect_with_user(
    ticket: String,
    p2p_service: State<'_, Arc<P2PService>>,
    user_service: State<'_, Arc<UserService>>,
) -> Result<CryptoResponse, String> {
    // let user = user_service.get_current_user().await?;
    // p2p_service
    //     .connect_with_ticket(&ticket, ConnectionType::User)
    //     .await?;
    // p2p_service.start_user_sync(&user.id).await?;
    todo!()
}

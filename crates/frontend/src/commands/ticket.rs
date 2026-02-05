use crate::state::AppState;
use common::ticket::{BuyerInfo, NoBindBuyerInfo};
use tauri::State;

#[tauri::command]
pub fn set_ticket_id(state: State<'_, AppState>, ticket_id: String) -> Result<(), String> {
    let mut ticket = state
        .ticket
        .lock()
        .map_err(|_| "ticket lock failed".to_string())?;
    ticket.ticket_id = ticket_id;
    Ok(())
}

#[tauri::command]
pub fn set_grab_mode(state: State<'_, AppState>, mode: u8) -> Result<(), String> {
    let mut ticket = state
        .ticket
        .lock()
        .map_err(|_| "ticket lock failed".to_string())?;
    let mut config = state
        .config
        .lock()
        .map_err(|_| "config lock failed".to_string())?;

    ticket.grab_mode = mode;
    config.config.grab_mode = mode;
    // We don't save config here immediately? Original code only set `state.grab_mode`. 
    // `state.config.grab_mode` was updated in `save_settings`.
    // Wait, original:
    // state.grab_mode = mode;
    // Ok(())
    // So it seems it was only runtime.
    // I'll stick to updating TicketState grab_mode.
    // But I will also update config.config.grab_mode to keep them in sync in memory.
    
    Ok(())
}

#[tauri::command]
pub fn set_show_screen_info(state: State<'_, AppState>, uid: Option<i64>) -> Result<(), String> {
    let mut ticket = state
        .ticket
        .lock()
        .map_err(|_| "ticket lock failed".to_string())?;
    ticket.show_screen_info = uid;
    Ok(())
}

#[tauri::command]
pub fn set_confirm_ticket_info(
    state: State<'_, AppState>,
    uid: Option<String>,
) -> Result<(), String> {
    let mut ticket = state
        .ticket
        .lock()
        .map_err(|_| "ticket lock failed".to_string())?;
    ticket.confirm_ticket_info = uid;
    Ok(())
}

#[tauri::command]
pub fn set_show_add_buyer_window(
    state: State<'_, AppState>,
    uid: Option<String>,
) -> Result<(), String> {
    let mut ui = state
        .ui
        .lock()
        .map_err(|_| "ui lock failed".to_string())?;
    ui.show_add_buyer_window = uid;
    Ok(())
}

#[tauri::command]
pub fn set_show_orderlist_window(
    state: State<'_, AppState>,
    uid: Option<String>,
) -> Result<(), String> {
    let mut ui = state
        .ui
        .lock()
        .map_err(|_| "ui lock failed".to_string())?;
    ui.show_orderlist_window = uid;
    Ok(())
}

#[tauri::command]
pub fn set_selected_screen(
    state: State<'_, AppState>,
    id: Option<i64>,
) -> Result<(), String> {
    let mut ticket = state
        .ticket
        .lock()
        .map_err(|_| "ticket lock failed".to_string())?;
    ticket.selected_screen_id = id;
    Ok(())
}

#[tauri::command]
pub fn set_selected_ticket(state: State<'_, AppState>, id: Option<i64>) -> Result<(), String> {
    let mut ticket = state
        .ticket
        .lock()
        .map_err(|_| "ticket lock failed".to_string())?;
    ticket.selected_ticket_id = id;
    Ok(())
}

#[tauri::command]
pub fn set_selected_buyer_list(
    state: State<'_, AppState>,
    buyer_list: Option<Vec<BuyerInfo>>,
) -> Result<(), String> {
    let mut ticket = state
        .ticket
        .lock()
        .map_err(|_| "ticket lock failed".to_string())?;
    ticket.selected_buyer_list = buyer_list;
    Ok(())
}

#[tauri::command]
pub fn set_buyer_type(state: State<'_, AppState>, buyer_type: u8) -> Result<(), String> {
    let mut ticket = state
        .ticket
        .lock()
        .map_err(|_| "ticket lock failed".to_string())?;
    ticket.buyer_type = buyer_type;
    Ok(())
}

#[tauri::command]
pub fn set_no_bind_buyer_info(
    state: State<'_, AppState>,
    name: String,
    tel: String,
) -> Result<(), String> {
    let mut ticket = state
        .ticket
        .lock()
        .map_err(|_| "ticket lock failed".to_string())?;

    let no_bind_buyer_info = NoBindBuyerInfo {
        name,
        tel,
        uid: 0, 
    };

    ticket.selected_no_bind_buyer_info = Some(no_bind_buyer_info);
    Ok(())
}

#[tauri::command]
pub fn clear_no_bind_buyer_info(state: State<'_, AppState>) -> Result<(), String> {
    let mut ticket = state
        .ticket
        .lock()
        .map_err(|_| "ticket lock failed".to_string())?;
    ticket.selected_no_bind_buyer_info = None;
    Ok(())
}
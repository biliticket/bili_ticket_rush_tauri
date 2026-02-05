use crate::state::{AccountSwitch, AppState};
use common::account::{Account, add_account};
use common::config::BtrConfig as Config;
use tauri::State;

#[tauri::command]
pub fn get_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    let state = state
        .config
        .lock()
        .map_err(|_| "config lock failed".to_string())?;
    Ok(state.accounts.clone())
}

#[tauri::command]
pub fn reload_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    let mut state = state
        .config
        .lock()
        .map_err(|_| "config lock failed".to_string())?;
    let config = Config::load_config().map_err(|e| e.to_string())?;
    state.accounts = config.accounts.clone();
    state.config = config;
    Ok(state.accounts.clone())
}

#[tauri::command]
pub fn add_account_by_cookie(
    state: State<'_, AppState>,
    cookie: String,
) -> Result<Account, String> {
    let auth = state
        .auth
        .lock()
        .map_err(|_| "auth lock failed".to_string())?;
    let mut config = state
        .config
        .lock()
        .map_err(|_| "config lock failed".to_string())?;
        
    let account = add_account(&cookie, &auth.client, &auth.default_ua)?;
    config.config.add_account(account.clone());
    config
        .config
        .save_config()
        .map_err(|e| format!("save config failed: {}", e))?;
    config.accounts.push(account.clone());
    Ok(account)
}

#[tauri::command]
pub fn delete_account_by_uid(state: State<'_, AppState>, uid: i64) -> Result<bool, String> {
    let mut config = state
        .config
        .lock()
        .map_err(|_| "config lock failed".to_string())?;
    let before = config.accounts.len();
    config.accounts.retain(|account| account.uid != uid);
    config.config.delete_account(uid);
    Ok(before != config.accounts.len())
}

#[tauri::command]
pub fn set_account_active(
    state: State<'_, AppState>,
    uid: i64,
    active: bool,
) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|_| "config lock failed".to_string())?;
    if let Some(account) = config.accounts.iter_mut().find(|a| a.uid == uid) {
        account.is_active = active;
        let account_clone = account.clone();
        config.config.update_account(&account_clone);
        config.config.save_config().map_err(|e| e.to_string())?;
        return Ok(());
    }
    Err("account not found".to_string())
}

#[tauri::command]
pub fn set_selected_account(state: State<'_, AppState>, uid: Option<i64>) -> Result<(), String> {
    let mut ui = state
        .ui
        .lock()
        .map_err(|_| "ui lock failed".to_string())?;
    ui.selected_account_uid = uid;
    Ok(())
}

#[tauri::command]
pub fn set_delete_account(state: State<'_, AppState>, uid: Option<String>) -> Result<(), String> {
    let mut ui = state
        .ui
        .lock()
        .map_err(|_| "ui lock failed".to_string())?;
    ui.delete_account = uid;
    Ok(())
}

#[tauri::command]
pub fn set_account_switch(
    state: State<'_, AppState>,
    uid: String,
    switch: bool,
) -> Result<(), String> {
    let mut ui = state
        .ui
        .lock()
        .map_err(|_| "ui lock failed".to_string())?;
    ui.account_switch = Some(AccountSwitch { uid, switch });
    Ok(())
}
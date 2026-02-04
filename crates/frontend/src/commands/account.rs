use crate::state::{AccountSwitch, AppState};
use common::account::{Account, add_account};
use common::config::BtrConfig as Config;
use tauri::State;

#[tauri::command]
pub fn get_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    Ok(state.accounts.clone())
}

#[tauri::command]
pub fn reload_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
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
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    let account = add_account(&cookie, &state.client, &state.default_ua)?;
    state.config.add_account(account.clone());
    state
        .config
        .save_config()
        .map_err(|e| format!("save config failed: {}", e))?;
    state.accounts.push(account.clone());
    Ok(account)
}

#[tauri::command]
pub fn delete_account_by_uid(state: State<'_, AppState>, uid: i64) -> Result<bool, String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    let before = state.accounts.len();
    state.accounts.retain(|account| account.uid != uid);
    state.config.delete_account(uid);
    Ok(before != state.accounts.len())
}

#[tauri::command]
pub fn set_account_active(
    state: State<'_, AppState>,
    uid: i64,
    active: bool,
) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    if let Some(account) = state.accounts.iter_mut().find(|a| a.uid == uid) {
        account.is_active = active;
        let account_clone = account.clone();
        state.config.update_account(&account_clone);
        state.config.save_config().map_err(|e| e.to_string())?;
        return Ok(());
    }
    Err("account not found".to_string())
}

#[tauri::command]
pub fn set_selected_account(state: State<'_, AppState>, uid: Option<i64>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.selected_account_uid = uid;
    Ok(())
}

#[tauri::command]
pub fn set_delete_account(state: State<'_, AppState>, uid: Option<String>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.delete_account = uid;
    Ok(())
}

#[tauri::command]
pub fn set_account_switch(
    state: State<'_, AppState>,
    uid: String,
    switch: bool,
) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.account_switch = Some(AccountSwitch { uid, switch });
    Ok(())
}

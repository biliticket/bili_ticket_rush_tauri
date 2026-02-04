use tauri::State;
use serde_json::{json, Value};
use common::{GRAB_LOG_COLLECTOR, LOG_COLLECTOR};
use common::taskmanager::{TaskRequest, PushRequest};
use common::config::Project;
use common::record_log::GrabLogCollector;
use common::PushType;
use crate::state::AppState;
use crate::utils::{create_client, current_timestamp, decode_policy, decode_permissions, save_permissions};

#[tauri::command]
pub fn push_test(state: State<'_, AppState>, title: String, message: String) -> Result<(), String> {
    let push_config = {
        let state = state
            .inner
            .lock()
            .map_err(|_| "state lock failed".to_string())?;
        if !state.push_config.enabled {
            return Err("push is disabled".to_string());
        }
        state.push_config.clone()
    };

    let request = TaskRequest::PushRequest(PushRequest {
        title,
        message,
        jump_url: None,
        push_config,
        push_type: PushType::All,
    });

    {
        let state = state
            .inner
            .lock()
            .map_err(|_| "state lock failed".to_string())?;
        let result = state
            .task_manager
            .lock()
            .map_err(|_| "Failed to lock task manager".to_string())?
            .submit_task(request)
            .map(|_| ())
            .map_err(|e| format!("submit push failed: {}", e));
        result
    }
}

#[tauri::command]
pub async fn get_policy(state: State<'_, AppState>) -> Result<Value, String> {
    let (machine_id, app, version, client) = {
        let state = state
            .inner
            .lock()
            .map_err(|_| "state lock failed".to_string())?;
        (
            state.machine_id.clone(),
            state.app.clone(),
            state.version.clone(),
            state.client.clone(),
        )
    };

    let data = json!({
        "ts": current_timestamp(),
        "machine_id": machine_id
    });

    let url = format!(
        "https://policy.nexaorion.cn/api/client/{}/{}/dispatch.json",
        app, version
    );

    let resp = client
        .post(&url)
        .json(&data)
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;

    let value: Value = resp
        .json()
        .await
        .map_err(|e| format!("parse failed: {}", e))?;

    if value["code"].as_i64().unwrap_or(-1) != 0 {
        return Ok(json!({ "allow_run": true }));
    }

    let policy_token = value["data"]["data"].as_str().unwrap_or("");
    let public_key = {
        let state = state
            .inner
            .lock()
            .map_err(|_| "state lock failed".to_string())?;
        state.public_key.clone()
    };
    let policy = decode_policy(policy_token, &public_key)?;
    if let Some(permission_token) = value["data"]["permission"].as_str() {
        let permissions = decode_permissions(permission_token, &public_key)?;
        save_permissions(permission_token);
        {
            let mut state = state
                .inner
                .lock()
                .map_err(|_| "state lock failed".to_string())?;
            state.config.permissions = permissions;
        }
    }
    Ok(policy)
}

#[tauri::command]
pub fn get_logs(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    if let Some(logs) = LOG_COLLECTOR
        .lock()
        .ok()
        .and_then(|mut c| c.get_logs())
    {
        for log in logs {
            state.logs.push(log);
        }
    }
    if state.logs.len() > 5000 {
        state.logs.drain(0..2500);
    }
    Ok(state.logs.clone())
}

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> Result<Value, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    Ok(json!({
        "app": state.app,
        "version": state.version,
        "running_status": state.running_status,
        "machine_id": state.machine_id,
        "announce1": state.announce1,
        "announce2": state.announce2,
        "announce3": state.announce3,
        "announce4": state.announce4
    }))
}

#[tauri::command]
pub fn clear_grab_logs() -> Result<(), String> {
    if let Ok(mut collector) = GRAB_LOG_COLLECTOR.lock() {
        collector.clear_logs();
    }
    Ok(())
}

#[tauri::command]
pub fn set_show_qr_windows(state: State<'_, AppState>, qr_data: Option<String>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.show_qr_windows = qr_data;
    Ok(())
}

#[tauri::command]
pub fn set_skip_words(state: State<'_, AppState>, words: Option<Vec<String>>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.skip_words = words;
    Ok(())
}

#[tauri::command]
pub fn set_skip_words_input(state: State<'_, AppState>, input: String) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.skip_words_input = input;
    Ok(())
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> Result<Value, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    Ok(json!({
        "selected_tab": state.selected_tab,
        "is_loading": state.is_loading,
        "running_status": state.running_status,
        "show_log_window": state.show_log_window,
        "show_login_window": state.show_login_window,
        "login_method": state.login_method,
        "ticket_id": state.ticket_id,
        "status_delay": state.status_delay,
        "grab_mode": state.grab_mode,
        "selected_account_uid": state.selected_account_uid,
        "show_screen_info": state.show_screen_info,
        "selected_screen_index": state.selected_screen_index,
        "selected_screen_id": state.selected_screen_id,
        "selected_ticket_id": state.selected_ticket_id,
        "confirm_ticket_info": state.confirm_ticket_info,
        "show_add_buyer_window": state.show_add_buyer_window,
        "show_orderlist_window": state.show_orderlist_window,
        "show_qr_windows": state.show_qr_windows,
        "skip_words": state.skip_words,
        "login_input": {
            "phone": state.login_input.phone,
            "account": state.login_input.account,
            "password": state.login_input.password,
            "cookie": state.login_input.cookie,
            "sms_code": state.login_input.sms_code
        }
    }))
}

#[tauri::command]
pub fn add_project(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    // Construct the URL from the project ID
    let url = format!("https://show.bilibili.com/platform/detail.html?id={}", id);

    // 创建新项目
    let project = Project {
        id: id.clone(),
        name: name.clone(),
        url: url.clone(),
        created_at: current_timestamp(),
        updated_at: current_timestamp(),
    };

    // 检查是否已存在相同ID的项目
    if state.config.projects.iter().any(|p| p.id == id) {
        return Err("项目ID已存在".to_string());
    }

    // 添加到配置中
    state.config.projects.push(project);

    // 保存配置
    if let Err(e) = state.config.save_config() {
        log::error!("保存项目失败: {}", e);
        return Err(format!("保存项目失败: {}", e));
    }

    log::info!("项目添加成功: ID={}, 名称={}", id, name);
    Ok(())
}

#[tauri::command]
pub fn get_projects(state: State<'_, AppState>) -> Result<Vec<Project>, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    Ok(state.config.projects.clone())
}

#[tauri::command]
pub fn delete_project(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    let original_len = state.config.projects.len();
    state.config.projects.retain(|p| p.id != id);

    if state.config.projects.len() == original_len {
        return Err("未找到指定ID的项目".to_string());
    }

    // 保存配置
    if let Err(e) = state.config.save_config() {
        log::error!("删除项目后保存失败: {}", e);
        return Err(format!("删除项目后保存失败: {}", e));
    }

    log::info!("项目删除成功: ID={}", id);
    Ok(())
}

#[tauri::command]
pub fn get_monitor_stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock失败".to_string())?;

    Ok(json!({
        "attempts": 0,
        "success": 0,
        "failures": 0,
        "running": state.running_status.contains("运行") || state.running_status.contains("抢票"),
        "active_tasks": 0,
        "completed_tasks": 0
    }))
}

#[tauri::command]
pub fn get_recent_logs(state: State<'_, AppState>, count: usize) -> Result<Vec<String>, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    let logs: Vec<String> = state.logs.iter().rev().take(count).cloned().collect();

    Ok(logs)
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    grab_mode: u8,
    delay_time: u64,
    max_attempts: u64,
    enable_push: bool,
    enabled_methods: Vec<String>,
    bark_token: String,
    pushplus_token: String,
    fangtang_token: String,
    dingtalk_token: String,
    wechat_token: String,
    gotify_url: String,
    gotify_token: String,
    custom_ua: bool,
    user_agent: String,
    skip_words: Option<Vec<String>>,
) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    // Update AppState
    state.grab_mode = grab_mode;
    state.status_delay = delay_time as usize;
    state.custom_config.open_custom_ua = custom_ua;
    state.custom_config.custom_ua = user_agent.clone();
    state.push_config.enabled = enable_push;
    state.push_config.enabled_methods = enabled_methods;
    state.push_config.bark_token = bark_token;
    state.push_config.pushplus_token = pushplus_token;
    state.push_config.fangtang_token = fangtang_token;
    state.push_config.dingtalk_token = dingtalk_token;
    state.push_config.wechat_token = wechat_token;
    state.push_config.gotify_config.gotify_url = gotify_url;
    state.push_config.gotify_config.gotify_token = gotify_token;
    state.skip_words = skip_words.clone();

    // Update the config struct
    state.config.grab_mode = grab_mode;
    state.config.delay_time = delay_time;
    state.config.max_attempts = max_attempts;
    state.config.push_config = state.push_config.clone();
    state.config.custom_config = state.custom_config.clone();
    state.config.skip_words = skip_words;

    if custom_ua && !user_agent.is_empty() {
        state.default_ua = user_agent.clone();
        state.client = create_client(user_agent);
    }

    if let Err(e) = state.config.save_config() {
        log::error!("保存配置失败: {}", e);
        return Err(format!("保存配置失败: {}", e));
    }

    log::info!("设置已保存!");
    Ok(())
}

#[tauri::command]
pub fn clear_logs(state: State<'_, AppState>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    state.logs.clear();

    log::info!("日志已清空");
    Ok(())
}

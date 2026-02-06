use crate::state::AppState;
use crate::utils::{
    create_client, current_timestamp, decode_permissions, decode_policy, save_permissions,
};
use common::PushType;
use common::config::Project;
use common::taskmanager::{PushRequest, TaskRequest};
use common::{GRAB_LOG_COLLECTOR, LOG_COLLECTOR};
use serde_json::{Value, json};
use tauri::State;

#[tauri::command]
pub fn push_test(state: State<'_, AppState>, title: String, message: String) -> Result<(), String> {
    let push_config = {
        let config = state
            .config
            .lock()
            .map_err(|_| "config lock failed".to_string())?;
        if !config.push_config.enabled {
            return Err("push is disabled".to_string());
        }
        config.push_config.clone()
    };

    let request = TaskRequest::PushRequest(PushRequest {
        title,
        message,
        jump_url: None,
        push_config,
        push_type: PushType::All,
    });

    {
        let mut runtime = state
            .runtime
            .lock()
            .map_err(|_| "runtime lock failed".to_string())?;
        let result = runtime
            .task_manager
            .submit_task(request)
            .map(|_| ())
            .map_err(|e| format!("submit push failed: {}", e));
        result
    }
}

#[tauri::command]
pub async fn get_policy(state: State<'_, AppState>) -> Result<Value, String> {
    let (machine_id, app, version, public_key) = {
        let runtime = state
            .runtime
            .lock()
            .map_err(|_| "runtime lock failed".to_string())?;
        (
            runtime.machine_id.clone(),
            runtime.app.clone(),
            runtime.version.clone(),
            runtime.public_key.clone(),
        )
    };

    let client = {
        let auth = state
            .auth
            .lock()
            .map_err(|_| "auth lock failed".to_string())?;
        auth.client.clone()
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
    let policy = decode_policy(policy_token, &public_key)?;

    if let Some(permission_token) = value["data"]["permission"].as_str() {
        let permissions = decode_permissions(permission_token, &public_key)?;
        save_permissions(permission_token);
        {
            let mut config = state
                .config
                .lock()
                .map_err(|_| "config lock failed".to_string())?;
            config.config.permissions = permissions;
        }
    }
    Ok(policy)
}

#[tauri::command]
pub fn get_logs(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| "runtime lock failed".to_string())?;
    if let Some(logs) = LOG_COLLECTOR.lock().ok().and_then(|mut c| c.get_logs()) {
        for log in logs {
            runtime.logs.push(log);
        }
    }
    if runtime.logs.len() > 5000 {
        runtime.logs.drain(0..2500);
    }
    Ok(runtime.logs.clone())
}

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> Result<Value, String> {
    let runtime = state
        .runtime
        .lock()
        .map_err(|_| "runtime lock failed".to_string())?;
    let ui = state.ui.lock().map_err(|_| "ui lock failed".to_string())?;

    Ok(json!({
        "app": runtime.app,
        "version": runtime.version,
        "running_status": runtime.running_status,
        "machine_id": runtime.machine_id,
        "announce1": ui.announce1,
        "announce2": ui.announce2,
        "announce3": ui.announce3,
        "announce4": ui.announce4
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
pub fn set_show_qr_windows(
    state: State<'_, AppState>,
    qr_data: Option<String>,
) -> Result<(), String> {
    let mut ui = state.ui.lock().map_err(|_| "ui lock failed".to_string())?;
    ui.show_qr_windows = qr_data;
    Ok(())
}

#[tauri::command]
pub fn set_skip_words(
    state: State<'_, AppState>,
    words: Option<Vec<String>>,
) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|_| "config lock failed".to_string())?;
    config.skip_words = words;
    Ok(())
}

#[tauri::command]
pub fn set_skip_words_input(state: State<'_, AppState>, input: String) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|_| "config lock failed".to_string())?;
    config.skip_words_input = input;
    Ok(())
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> Result<Value, String> {
    let ui = state.ui.lock().map_err(|_| "ui lock failed")?;
    let runtime = state.runtime.lock().map_err(|_| "runtime lock failed")?;
    let auth = state.auth.lock().map_err(|_| "auth lock failed")?;
    let ticket = state.ticket.lock().map_err(|_| "ticket lock failed")?;
    let config = state.config.lock().map_err(|_| "config lock failed")?;

    Ok(json!({
        "selected_tab": ui.selected_tab,
        "is_loading": runtime.is_loading,
        "running_status": runtime.running_status,
        "show_log_window": ui.show_log_window,
        "show_login_window": ui.show_login_window,
        "login_method": auth.login_method,
        "ticket_id": ticket.ticket_id,
        "status_delay": ticket.status_delay,
        "grab_mode": ticket.grab_mode,
        "selected_account_uid": ui.selected_account_uid,
        "show_screen_info": ticket.show_screen_info,
        "selected_screen_id": ticket.selected_screen_id,
        "selected_ticket_id": ticket.selected_ticket_id,
        "confirm_ticket_info": ticket.confirm_ticket_info,
        "show_add_buyer_window": ui.show_add_buyer_window,
        "show_orderlist_window": ui.show_orderlist_window,
        "show_qr_windows": ui.show_qr_windows,
        "skip_words": config.skip_words,
        "login_input": {
            "phone": auth.login_input.phone,
            "account": auth.login_input.account,
            "password": auth.login_input.password,
            "cookie": auth.login_input.cookie,
            "sms_code": auth.login_input.sms_code
        },
        "custom_config": config.custom_config,
        "push_config": config.push_config,
        "config": config.config
    }))
}

#[tauri::command]
pub fn add_project(state: State<'_, AppState>, id: String, name: String) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|_| "config lock failed".to_string())?;

    let url = format!("https://show.bilibili.com/platform/detail.html?id={}", id);

    let project = Project {
        id: id.clone(),
        name: name.clone(),
        url: url.clone(),
        created_at: current_timestamp(),
        updated_at: current_timestamp(),
    };

    if config.config.projects.iter().any(|p| p.id == id) {
        return Err("项目ID已存在".to_string());
    }

    config.config.projects.push(project);

    if let Err(e) = config.config.save_config() {
        log::error!("保存项目失败: {}", e);
        return Err(format!("保存项目失败: {}", e));
    }

    log::info!("项目添加成功: ID={}, 名称={}", id, name);
    Ok(())
}

#[tauri::command]
pub fn get_projects(state: State<'_, AppState>) -> Result<Vec<Project>, String> {
    let config = state
        .config
        .lock()
        .map_err(|_| "config lock failed".to_string())?;
    Ok(config.config.projects.clone())
}

#[tauri::command]
pub fn delete_project(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|_| "config lock failed".to_string())?;

    let original_len = config.config.projects.len();
    config.config.projects.retain(|p| p.id != id);

    if config.config.projects.len() == original_len {
        return Err("未找到指定ID的项目".to_string());
    }

    if let Err(e) = config.config.save_config() {
        log::error!("删除项目后保存失败: {}", e);
        return Err(format!("删除项目后保存失败: {}", e));
    }

    log::info!("项目删除成功: ID={}", id);
    Ok(())
}

#[tauri::command]
pub fn get_monitor_stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let runtime = state
        .runtime
        .lock()
        .map_err(|_| "runtime lock失败".to_string())?;

    Ok(json!({
        "attempts": 0,
        "success": 0,
        "failures": 0,
        "running": runtime.running_status.contains("运行") || runtime.running_status.contains("抢票"),
        "active_tasks": 0,
        "completed_tasks": 0
    }))
}

#[tauri::command]
pub fn get_recent_logs(state: State<'_, AppState>, count: usize) -> Result<Vec<String>, String> {
    let runtime = state
        .runtime
        .lock()
        .map_err(|_| "runtime lock failed".to_string())?;

    let logs: Vec<String> = runtime.logs.iter().rev().take(count).cloned().collect();

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
    max_token_retry: u8,
    max_confirm_retry: u8,
    max_fake_check_retry: u32,
    max_order_retry: u32,
    retry_interval_ms: u64,
    dungeon_channel: u8,
    dungeon_intensity: u8,
    dungeon_frequency: u8,
    dungeon_pulse_ms: u64,
    dungeon_pause_ms: u64,
    dungeon_count: u8,
) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|_| "config lock failed".to_string())?;
    let mut ticket = state
        .ticket
        .lock()
        .map_err(|_| "ticket lock failed".to_string())?;
    let mut auth = state
        .auth
        .lock()
        .map_err(|_| "auth lock failed".to_string())?;

    ticket.grab_mode = grab_mode;
    ticket.status_delay = delay_time as usize;

    config.custom_config.open_custom_ua = custom_ua;
    config.custom_config.custom_ua = user_agent.clone();
    config.custom_config.max_token_retry = max_token_retry;
    config.custom_config.max_confirm_retry = max_confirm_retry;
    config.custom_config.max_fake_check_retry = max_fake_check_retry;
    config.custom_config.max_order_retry = max_order_retry;
    config.custom_config.retry_interval_ms = retry_interval_ms;

    config.push_config.enabled = enable_push;
    config.push_config.enabled_methods = enabled_methods;
    config.push_config.bark_token = bark_token;
    config.push_config.pushplus_token = pushplus_token;
    config.push_config.fangtang_token = fangtang_token;
    config.push_config.dingtalk_token = dingtalk_token;
    config.push_config.wechat_token = wechat_token;
    config.push_config.gotify_config.gotify_url = gotify_url;
    config.push_config.gotify_config.gotify_token = gotify_token;

    config.push_config.dungeon_config.channel = dungeon_channel;
    config.push_config.dungeon_config.intensity = dungeon_intensity;
    config.push_config.dungeon_config.frequency = dungeon_frequency;
    config.push_config.dungeon_config.pulse_ms = dungeon_pulse_ms;
    config.push_config.dungeon_config.pause_ms = dungeon_pause_ms;
    config.push_config.dungeon_config.count = dungeon_count;
    config.push_config.dungeon_config.enabled = config
        .push_config
        .enabled_methods
        .contains(&"dungeon".to_string());

    config.skip_words = skip_words.clone();

    config.config.grab_mode = grab_mode;
    config.config.delay_time = delay_time;
    config.config.max_attempts = max_attempts;
    config.config.push_config = config.push_config.clone();
    config.config.custom_config = config.custom_config.clone();
    config.config.skip_words = skip_words;

    if custom_ua && !user_agent.is_empty() {
        auth.default_ua = user_agent.clone();
        auth.client = create_client(user_agent);
    }

    if let Err(e) = config.config.save_config() {
        log::error!("保存配置失败: {}", e);
        return Err(format!("保存配置失败: {}", e));
    }

    log::info!("设置已保存!");
    Ok(())
}

#[tauri::command]
pub fn clear_logs(state: State<'_, AppState>) -> Result<(), String> {
    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| "runtime lock failed".to_string())?;

    runtime.logs.clear();

    log::info!("日志已清空");
    Ok(())
}

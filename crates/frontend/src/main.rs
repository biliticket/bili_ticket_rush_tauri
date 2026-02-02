#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use common::GRAB_LOG_COLLECTOR;

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use rand::{Rng, distributions::Alphanumeric, thread_rng};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use tauri::State;

use backend::taskmanager::TaskManagerImpl;
use common::PushType;
use common::account::{Account, add_account};
use common::captcha::LocalCaptcha;
use common::login::LoginInput;
use common::push::PushConfig;

use common::taskmanager::{
    GetAllorderRequest, GetBuyerInfoRequest, GetTicketInfoRequest, TaskManager, TaskRequest,
    TaskStatus,
};
use common::ticket::{BilibiliTicket, TicketInfo};
use common::utility::CustomConfig;
use common::utils::{Config, save_config};

const APP_NAME: &str = "BTR";
const APP_VERSION: &str = "7.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Project {
    id: String,
    name: String,
    url: String,
    created_at: u64,
    updated_at: u64,
}

#[derive(Clone)]
struct AppState {
    inner: Arc<Mutex<AppStateInner>>,
}

#[derive(Clone)]
struct AppStateInner {
    // App info
    app: String,
    version: String,
    policy: Option<Value>,
    public_key: String,
    machine_id: String,

    // UI state
    selected_tab: usize,
    is_loading: bool,
    running_status: String,

    // Logs
    logs: Vec<String>,
    show_log_window: bool,

    // Login
    show_login_window: bool,
    login_method: String,
    client: Client,
    default_ua: String,
    login_qrcode_url: Option<String>,
    qrcode_polling_task_id: Option<String>,
    login_input: LoginInput,
    pending_sms_task_id: Option<String>,
    sms_captcha_key: String,
    cookie_login: Option<String>,

    // Account management
    accounts: Vec<Account>,
    delete_account: Option<String>,
    account_switch: Option<AccountSwitch>,

    // Task management
    task_manager: Arc<Mutex<Box<dyn TaskManager>>>,

    // Config
    config: Config,
    push_config: PushConfig,
    custom_config: CustomConfig,

    // Ticket grabbing
    ticket_id: String,
    status_delay: usize,
    grab_mode: u8,
    selected_account_uid: Option<i64>,
    bilibiliticket_list: Vec<BilibiliTicket>,
    ticket_info: Option<TicketInfo>,
    show_screen_info: Option<i64>,
    selected_screen_index: Option<usize>,
    selected_screen_id: Option<i64>,
    selected_ticket_id: Option<i64>,
    ticket_info_last_request_time: Option<std::time::Instant>,
    confirm_ticket_info: Option<String>,
    selected_buyer_list: Option<Vec<common::ticket::BuyerInfo>>,
    selected_no_bind_buyer_info: Option<common::ticket::NoBindBuyerInfo>,
    buyer_type: u8, // 0: 非实名购票人, 1: 实名购票人

    // Buyer management
    show_add_buyer_window: Option<String>,
    show_orderlist_window: Option<String>,
    total_order_data: Option<OrderData>,
    orderlist_need_reload: bool,
    orderlist_last_request_time: Option<std::time::Instant>,
    orderlist_requesting: bool,

    // QR code payment
    show_qr_windows: Option<String>,

    // Announcements
    announce1: Option<String>,
    announce2: Option<String>,
    announce3: Option<String>,
    announce4: Option<String>,

    // Other
    skip_words: Option<Vec<String>>,
    skip_words_input: String,
}

#[derive(Clone)]
struct OrderData {
    account_id: String,
    data: Option<common::show_orderlist::OrderResponse>,
}

#[derive(Clone)]
struct AccountSwitch {
    uid: String,
    switch: bool,
}

impl AppState {
    pub fn new() -> Self {
        let config = Config::load_config().unwrap_or_else(|_| Config::new());

        let mut state = AppStateInner {
            app: APP_NAME.to_string(),
            version: APP_VERSION.to_string(),
            policy: None,
            public_key: "-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEApTAS0RElXIs4Kr0bO4n8\nJB+eBFF/TwXUlvtOM9FNgHjK8m13EdwXaLy9zjGTSQr8tshSRr0dQ6iaCG19Zo2Y\nXfvJrwQLqdezMN+ayMKFy58/S9EGG3Np2eGgKHUPnCOAlRicqWvBdQ/cxzTDNCxa\nORMZdJRoBvya7JijLLIC3CoqmMc6Fxe5i8eIP0zwlyZ0L0C1PQ82BcWn58y7tlPY\nTCz12cWnuKwiQ9LSOfJ4odJJQK0k7rXxwBBsYxULRno0CJ3rKfApssW4cfITYVax\nFtdbu0IUsgEeXs3EzNw8yIYnsaoZlFwLS8SMVsiAFOy2y14lR9043PYAQHm1Cjaf\noQIDAQAB\n-----END PUBLIC KEY-----".to_string(),
            machine_id: common::machine_id::get_machine_id_ob(),
            selected_tab: 0,
            is_loading: false,
            running_status: "空闲".to_string(),
            logs: Vec::new(),
            show_log_window: false,
            show_login_window: false,
            login_method: "扫码登录".to_string(),
            client: Client::new(),
            default_ua: default_user_agent(),
            login_qrcode_url: None,
            qrcode_polling_task_id: None,
            login_input: LoginInput {
                phone: String::new(),
                account: String::new(),
                password: String::new(),
                cookie: String::new(),
                sms_code: String::new(),
            },
            pending_sms_task_id: None,
            sms_captcha_key: String::new(),
            cookie_login: None,
            accounts: Config::load_all_accounts(),
            delete_account: None,
            account_switch: None,
            task_manager: Arc::new(Mutex::new(Box::new(TaskManagerImpl::new()))),
            config: config.clone(),
            push_config: serde_json::from_value::<PushConfig>(config["push_config"].clone())
                .unwrap_or_else(|_| PushConfig::new()),
            custom_config: serde_json::from_value::<CustomConfig>(config["custom_config"].clone())
                .unwrap_or_else(|_| CustomConfig::new()),
            ticket_id: String::new(),
            status_delay: 2,
            grab_mode: 0,
            selected_account_uid: None,
            bilibiliticket_list: Vec::new(),
            ticket_info: None,
            show_screen_info: None,
            selected_screen_index: None,
            selected_screen_id: None,
            selected_ticket_id: None,
            ticket_info_last_request_time: None,
            confirm_ticket_info: None,
            selected_buyer_list: None,
            selected_no_bind_buyer_info: None,
            buyer_type: 1, // 默认使用实名购票人
            show_add_buyer_window: None,
            show_orderlist_window: None,
            total_order_data: None,
            orderlist_need_reload: false,
            orderlist_last_request_time: None,
            orderlist_requesting: false,
            show_qr_windows: None,
            announce1: None,
            announce2: None,
            announce3: None,
            announce4: None,
            skip_words: None,
            skip_words_input: String::new(),
        };

        // Initialize client with custom UA
        let random_value = generate_random_string(8);
        state.default_ua = format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36 Edg/134.0.0.0 {}",
            random_value
        );

        if state.custom_config.open_custom_ua && !state.custom_config.custom_ua.is_empty() {
            state.default_ua = state.custom_config.custom_ua.clone();
        }

        let new_client = create_client(state.default_ua.clone());
        state.client = new_client;

        // Initialize accounts
        for account in &mut state.accounts {
            account.ensure_client();
        }

        Self {
            inner: Arc::new(Mutex::new(state)),
        }
    }
}

#[tauri::command]
fn get_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    Ok(state.accounts.clone())
}

#[tauri::command]
fn reload_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.accounts = Config::load_all_accounts();
    Ok(state.accounts.clone())
}

#[tauri::command]
fn add_account_by_cookie(state: State<'_, AppState>, cookie: String) -> Result<Account, String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    let account = add_account(&cookie, &state.client, &state.default_ua)?;
    save_config(&mut state.config, None, None, Some(account.clone()))
        .map_err(|e| format!("save config failed: {}", e))?;
    state.accounts.push(account.clone());
    Ok(account)
}

#[tauri::command]
fn delete_account_by_uid(state: State<'_, AppState>, uid: i64) -> Result<bool, String> {
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
fn set_account_active(state: State<'_, AppState>, uid: i64, active: bool) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    if let Some(account) = state.accounts.iter_mut().find(|a| a.uid == uid) {
        account.is_active = active;
        let account_clone = account.clone();
        drop(state);

        let mut config = Config::load_config().map_err(|e| format!("load config failed: {}", e))?;
        save_config(&mut config, None, None, Some(account_clone))
            .map_err(|e| format!("save config failed: {}", e))?;
        return Ok(());
    }
    Err("account not found".to_string())
}

#[tauri::command]
fn qrcode_login(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    let qrcode_key =
        common::login::qrcode_login(&state.client).map_err(|e| format!("生成二维码失败: {}", e))?;

    let qrcode_url = format!(
        "https://passport.bilibili.com/h5-app/passport/login/scan?qrcode_key={}",
        qrcode_key
    );

    use image::Luma;
    use qrcode::QrCode;

    let code = QrCode::new(qrcode_url.as_bytes()).map_err(|e| format!("生成二维码失败: {}", e))?;

    let image = code
        .render::<Luma<u8>>()
        .min_dimensions(200, 200)
        .max_dimensions(400, 400)
        .build();

    let mut png_data: Vec<u8> = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut png_data),
            image::ImageFormat::Png,
        )
        .map_err(|e| format!("转换图片失败: {}", e))?;

    let base64_image =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png_data);
    let data_url = format!("data:image/png;base64,{}", base64_image);

    let request = TaskRequest::QrCodeLoginRequest(common::taskmanager::QrCodeLoginRequest {
        qrcode_key: qrcode_key.clone(),
        qrcode_url: qrcode_url.clone(),
        user_agent: Some(state.default_ua.clone()),
    });

    let task_id = state
        .task_manager
        .lock()
        .map_err(|_| "Failed to lock task manager".to_string())?
        .submit_task(request)
        .map_err(|e| format!("提交二维码登录任务失败: {}", e))?;

    Ok(json!({
        "key": qrcode_key,
        "url": data_url,
        "task_id": task_id,
        "message": "二维码生成成功，请使用B站APP扫描"
    }))
}

#[tauri::command]
fn sms_login(state: State<'_, AppState>, phone: String) -> Result<String, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    let request = TaskRequest::LoginSmsRequest(common::taskmanager::LoginSmsRequest {
        phone: phone.clone(),
        client: state.client.clone(),
        custom_config: state.custom_config.clone(),
        local_captcha: common::captcha::LocalCaptcha::new(),
    });

    let result = state
        .task_manager
        .lock()
        .map_err(|_| "Failed to lock task manager".to_string())?
        .submit_task(request)
        .map_err(|e| format!("submit sms login failed: {}", e));

    result
}

#[tauri::command]
fn submit_sms_code(
    state: State<'_, AppState>,
    captcha_key: String,
    sms_code: String,
) -> Result<String, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    let request = TaskRequest::SubmitLoginSmsRequest(common::taskmanager::SubmitLoginSmsRequest {
        phone: "".to_string(),
        code: sms_code,
        captcha_key,
        client: state.client.clone(),
    });

    let result = state
        .task_manager
        .lock()
        .map_err(|_| "Failed to lock task manager".to_string())?
        .submit_task(request)
        .map_err(|e| format!("submit sms code failed: {}", e));

    result
}

#[tauri::command]
fn get_ticket_info(
    state: State<'_, AppState>,
    uid: i64,
    project_id: String,
) -> Result<String, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    let account = state
        .accounts
        .iter()
        .find(|a| a.uid == uid)
        .ok_or_else(|| "account not found".to_string())?;

    let cookie_manager = account
        .cookie_manager
        .clone()
        .ok_or_else(|| "cookie manager not initialized".to_string())?;

    let task_id = uuid::Uuid::new_v4().to_string();
    let request = TaskRequest::GetTicketInfoRequest(GetTicketInfoRequest {
        uid,
        task_id: task_id.clone(),
        project_id,
        cookie_manager,
    });

    let result = state
        .task_manager
        .lock()
        .map_err(|_| "Failed to lock task manager".to_string())?
        .submit_task(request)
        .map_err(|e| format!("submit ticket info request failed: {}", e));

    result
}

#[tauri::command]
fn get_buyer_info(state: State<'_, AppState>, uid: i64) -> Result<String, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    let account = state
        .accounts
        .iter()
        .find(|a| a.uid == uid)
        .ok_or_else(|| "account not found".to_string())?;

    let cookie_manager = account
        .cookie_manager
        .clone()
        .ok_or_else(|| "cookie manager not initialized".to_string())?;

    let task_id = uuid::Uuid::new_v4().to_string();
    let request = TaskRequest::GetBuyerInfoRequest(GetBuyerInfoRequest {
        uid,
        task_id: task_id.clone(),
        cookie_manager,
    });

    let result = state
        .task_manager
        .lock()
        .map_err(|_| "Failed to lock task manager".to_string())?
        .submit_task(request)
        .map_err(|e| format!("submit buyer info request failed: {}", e));

    result
}

#[tauri::command]
fn get_order_list(state: State<'_, AppState>, uid: i64) -> Result<String, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    let account = state
        .accounts
        .iter()
        .find(|a| a.uid == uid)
        .ok_or_else(|| "account not found".to_string())?;

    let cookie_manager = account
        .cookie_manager
        .clone()
        .ok_or_else(|| "cookie manager not initialized".to_string())?;

    let request = TaskRequest::GetAllorderRequest(GetAllorderRequest {
        task_id: "".to_string(),
        cookie_manager,
        status: TaskStatus::Pending,
        cookies: account.cookie.clone(),
        account_id: uid.to_string(),
        start_time: None,
    });

    let result = state
        .task_manager
        .lock()
        .map_err(|_| "Failed to lock task manager".to_string())?
        .submit_task(request)
        .map_err(|e| format!("submit order list request failed: {}", e));

    result
}

#[tauri::command]
fn poll_task_results(state: State<'_, AppState>) -> Result<Value, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    let results = state
        .task_manager
        .lock()
        .map_err(|_| "Failed to lock task manager".to_string())?
        .get_results();
    let json_results: Vec<Value> = results
        .into_iter()
        .map(|result| match result {
            common::taskmanager::TaskResult::QrCodeLoginResult(r) => json!({
                "type": "QrCodeLoginResult",
                "task_id": r.task_id,
                "status": format!("{:?}", r.status),
                "cookie": r.cookie,
                "error": r.error
            }),
            common::taskmanager::TaskResult::LoginSmsResult(r) => json!({
                "type": "LoginSmsResult",
                "success": r.success,
                "message": r.message
            }),
            common::taskmanager::TaskResult::SubmitSmsLoginResult(r) => json!({
                "type": "SubmitSmsLoginResult",
                "success": r.success,
                "message": r.message,
                "cookie": r.cookie
            }),
            common::taskmanager::TaskResult::PushResult(r) => json!({
                "type": "PushResult",
                "success": r.success,
                "message": r.message
            }),
            common::taskmanager::TaskResult::GetAllorderRequestResult(r) => json!({
                "type": "GetAllorderRequestResult",
                "task_id": r.task_id,
                "success": r.success,
                "account_id": r.account_id,
                "message": r.message,
                "order_info": r.order_info
            }),
            common::taskmanager::TaskResult::GetTicketInfoResult(r) => json!({
                "type": "GetTicketInfoResult",
                "task_id": r.task_id,
                "success": r.success,
                "uid": r.uid,
                "message": r.message,
                "ticket_info": r.ticket_info
            }),
            common::taskmanager::TaskResult::GetBuyerInfoResult(r) => json!({
                "type": "GetBuyerInfoResult",
                "task_id": r.task_id,
                "success": r.success,
                "uid": r.uid,
                "message": r.message,
                "buyer_info": r.buyer_info
            }),
            common::taskmanager::TaskResult::GrabTicketResult(r) => json!({
                "type": "GrabTicketResult",
                "success": r.success,
                "order_id": r.order_id,
                "message": r.message,
                "pay_result": r.pay_result,
                "confirm_result": r.confirm_result
            }),
        })
        .collect();

    Ok(json!(json_results))
}

#[tauri::command]
fn push_test(state: State<'_, AppState>, title: String, message: String) -> Result<(), String> {
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

    let request = TaskRequest::PushRequest(common::taskmanager::PushRequest {
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
async fn get_policy(state: State<'_, AppState>) -> Result<Value, String> {
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
            if let Value::Object(obj) = &mut state.config["permissions"] {
                *obj = permissions.as_object().cloned().unwrap_or_default();
            }
        }
    }
    Ok(policy)
}

#[tauri::command]
fn get_logs(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    if let Some(logs) = common::LOG_COLLECTOR
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
fn get_grab_logs(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    // 从抢票日志收集器获取日志
    if let Some(logs) = GRAB_LOG_COLLECTOR.lock().ok().and_then(
        |mut c: std::sync::MutexGuard<'_, common::record_log::GrabLogCollector>| c.get_logs(),
    ) {
        for log in logs {
            state.logs.push(log);
        }
    }

    // 过滤出抢票相关的日志
    let grab_logs: Vec<String> = state
        .logs
        .iter()
        .filter(|log| {
            let log_str = log.to_lowercase();
            log_str.contains("抢票")
                || log_str.contains("token")
                || log_str.contains("订单")
                || log_str.contains("验证码")
                || log_str.contains("倒计时")
                || log_str.contains("项目")
                || log_str.contains("场次")
                || log_str.contains("购票人")
                || log_str.contains("开始抢票")
                || log_str.contains("获取token")
                || log_str.contains("确认订单")
                || log_str.contains("下单")
                || log_str.contains("重试")
                || log_str.contains("失败")
                || log_str.contains("成功")
                || log_str.contains("距离抢票时间")
                || log_str.contains("获取购票人信息")
                || log_str.contains("获取项目详情")
                || log_str.contains("二维码")
                || log_str.contains("短信")
                || log_str.contains("登录")
        })
        .cloned()
        .collect();

    if grab_logs.len() > 5000 {
        let skip_count = grab_logs.len() - 5000;
        Ok(grab_logs.into_iter().skip(skip_count).collect())
    } else {
        Ok(grab_logs)
    }
}

#[tauri::command]
fn add_log(state: State<'_, AppState>, message: String) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.logs.push(message);
    if state.logs.len() > 5000 {
        state.logs.drain(0..2500);
    }
    Ok(())
}

#[tauri::command]
fn get_app_info(state: State<'_, AppState>) -> Result<Value, String> {
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
fn clear_grab_logs() -> Result<(), String> {
    if let Ok(mut collector) = GRAB_LOG_COLLECTOR.lock() {
        collector.clear_logs();
    }
    Ok(())
}

#[tauri::command]
fn set_ticket_id(state: State<'_, AppState>, ticket_id: String) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.ticket_id = ticket_id;
    Ok(())
}

#[tauri::command]
fn set_grab_mode(state: State<'_, AppState>, mode: u8) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.grab_mode = mode;
    Ok(())
}

#[tauri::command]
fn cancel_task(state: State<'_, AppState>, task_id: String) -> Result<(), String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    let mut task_manager = state
        .task_manager
        .lock()
        .map_err(|_| "Failed to lock task manager".to_string())?;

    task_manager.cancel_task(&task_id)?;

    log::info!("已取消任务: {}", task_id);
    Ok(())
}

#[tauri::command]
fn start_grab_ticket(state: State<'_, AppState>) -> Result<String, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    // 验证必要信息
    if state.ticket_id.is_empty() {
        return Err("请先选择项目".to_string());
    }

    if state.accounts.is_empty() {
        return Err("请先添加账号".to_string());
    }

    // 获取选中的账号或使用第一个活跃账号
    let selected_account = if let Some(uid) = state.selected_account_uid {
        state.accounts.iter().find(|acc| acc.uid == uid)
    } else {
        state.accounts.iter().find(|acc| acc.is_active)
    };

    let account = selected_account
        .ok_or_else(|| "没有可用的账号，请确保至少有一个账号是激活状态".to_string())?;

    // 验证账号有 cookie_manager
    let cookie_manager = account
        .cookie_manager
        .clone()
        .ok_or_else(|| "账号未初始化，请重新添加账号".to_string())?;

    let (id_bind, buyer_info, no_bind_buyer_info) = match state.buyer_type {
        0 => {
            // 非实名购票人信息
            if state.selected_no_bind_buyer_info.is_none() {
                return Err("请先设置非实名购票人信息".to_string());
            }
            (0, None, state.selected_no_bind_buyer_info.clone())
        }
        1 => {
            // 实名购票人信息
            if state.selected_buyer_list.is_none() {
                return Err("请先选择实名购票人信息".to_string());
            }
            (1, state.selected_buyer_list.clone(), None)
        }
        2 => {
            // 实名购票人信息（备用模式）
            if state.selected_buyer_list.is_none() {
                return Err("请先选择实名购票人信息".to_string());
            }
            (2, state.selected_buyer_list.clone(), None)
        }
        _ => {
            return Err("无效的购票人类型".to_string());
        }
    };

    let biliticket = BilibiliTicket {
        uid: account.uid,
        method: 0,
        ua: state.default_ua.clone(),
        config: state.custom_config.clone(),
        account: account.clone(),
        push_self: state.push_config.clone(),
        status_delay: state.status_delay,
        captcha_use_type: 0,
        cookie_manager: account.cookie_manager.clone(),
        project_id: state.ticket_id.clone(),
        screen_id: state
            .selected_screen_id
            .map(|id| id.to_string())
            .unwrap_or_default(),
        id_bind,
        project_info: state.ticket_info.clone(),
        all_buyer_info: None,
        buyer_info,
        no_bind_buyer_info,
        select_ticket_id: state.selected_ticket_id.map(|id| id.to_string()),
        pay_money: None,
        count: Some(1),
        device_id: String::new(),
    };

    // 生成任务ID
    let task_id = format!(
        "{}-{}",
        account.uid,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    // 创建抢票请求
    let grab_request = TaskRequest::GrabTicketRequest(common::taskmanager::GrabTicketRequest {
        task_id: task_id.clone(),
        uid: account.uid,
        project_id: state.ticket_id.clone(),
        screen_id: state
            .selected_screen_id
            .map(|id| id.to_string())
            .unwrap_or_default(),
        ticket_id: state
            .selected_ticket_id
            .map(|id| id.to_string())
            .unwrap_or_default(),
        count: 1,
        buyer_info: vec![],
        cookie_manager,
        biliticket,
        grab_mode: state.grab_mode,
        status: TaskStatus::Pending,
        start_time: None,
        is_hot: false,
        local_captcha: LocalCaptcha::new(),
        skip_words: None,
    });

    // 提交任务
    state
        .task_manager
        .lock()
        .map_err(|_| "Failed to lock task manager".to_string())?
        .submit_task(grab_request)
        .map_err(|e| format!("提交抢票任务失败: {}", e))?;

    Ok(task_id)
}

#[tauri::command]
fn set_selected_account(state: State<'_, AppState>, uid: Option<i64>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.selected_account_uid = uid;
    Ok(())
}

#[tauri::command]
fn set_show_screen_info(state: State<'_, AppState>, uid: Option<i64>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.show_screen_info = uid;
    Ok(())
}

#[tauri::command]
fn set_confirm_ticket_info(state: State<'_, AppState>, uid: Option<String>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.confirm_ticket_info = uid;
    Ok(())
}

#[tauri::command]
fn set_show_add_buyer_window(
    state: State<'_, AppState>,
    uid: Option<String>,
) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.show_add_buyer_window = uid;
    Ok(())
}

#[tauri::command]
fn set_show_orderlist_window(
    state: State<'_, AppState>,
    uid: Option<String>,
) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.show_orderlist_window = uid;
    Ok(())
}

#[tauri::command]
fn set_show_qr_windows(state: State<'_, AppState>, qr_data: Option<String>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.show_qr_windows = qr_data;
    Ok(())
}

#[tauri::command]
fn set_login_method(state: State<'_, AppState>, method: String) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.login_method = method;
    Ok(())
}

#[tauri::command]
fn set_show_login_window(state: State<'_, AppState>, show: bool) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.show_login_window = show;
    Ok(())
}

#[tauri::command]
fn set_login_input(state: State<'_, AppState>, input: LoginInput) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.login_input = input;
    Ok(())
}

#[tauri::command]
fn set_cookie_login(state: State<'_, AppState>, cookie: Option<String>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.cookie_login = cookie;
    Ok(())
}

#[tauri::command]
fn set_delete_account(state: State<'_, AppState>, uid: Option<String>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.delete_account = uid;
    Ok(())
}

#[tauri::command]
fn set_account_switch(state: State<'_, AppState>, uid: String, switch: bool) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.account_switch = Some(AccountSwitch { uid, switch });
    Ok(())
}

#[tauri::command]
fn set_selected_screen(
    state: State<'_, AppState>,
    index: Option<usize>,
    id: Option<i64>,
) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.selected_screen_index = index;
    state.selected_screen_id = id;
    Ok(())
}

#[tauri::command]
fn set_selected_ticket(state: State<'_, AppState>, id: Option<i64>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.selected_ticket_id = id;
    Ok(())
}

#[tauri::command]
fn set_selected_buyer_list(
    state: State<'_, AppState>,
    buyer_list: Option<Vec<common::ticket::BuyerInfo>>,
) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.selected_buyer_list = buyer_list;
    Ok(())
}

#[tauri::command]
fn set_buyer_type(state: State<'_, AppState>, buyer_type: u8) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.buyer_type = buyer_type;
    Ok(())
}

#[tauri::command]
fn set_no_bind_buyer_info(
    state: State<'_, AppState>,
    name: String,
    tel: String,
) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    let no_bind_buyer_info = common::ticket::NoBindBuyerInfo {
        name,
        tel,
        uid: 0, // 非实名购票人没有uid
    };

    state.selected_no_bind_buyer_info = Some(no_bind_buyer_info);
    Ok(())
}

#[tauri::command]
fn clear_no_bind_buyer_info(state: State<'_, AppState>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.selected_no_bind_buyer_info = None;
    Ok(())
}

#[tauri::command]
fn set_skip_words(state: State<'_, AppState>, words: Option<Vec<String>>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.skip_words = words;
    Ok(())
}

#[tauri::command]
fn set_skip_words_input(state: State<'_, AppState>, input: String) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;
    state.skip_words_input = input;
    Ok(())
}

#[tauri::command]
fn get_state(state: State<'_, AppState>) -> Result<Value, String> {
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
        "skip_words_input": state.skip_words_input,
        "login_input": {
            "phone": state.login_input.phone,
            "account": state.login_input.account,
            "password": state.login_input.password,
            "cookie": state.login_input.cookie,
            "sms_code": state.login_input.sms_code
        }
    }))
}

// ========== Helper Functions ==========

fn create_client(user_agent: String) -> Client {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_str(&user_agent).unwrap_or_else(|_| {
            header::HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
        }),
    );

    Client::builder()
        .default_headers(headers)
        .cookie_store(true)
        .build()
        .unwrap_or_default()
}

fn default_user_agent() -> String {
    let random_value = generate_random_string(8);
    format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36 Edg/134.0.0.0 {}",
        random_value
    )
}

fn generate_random_string(length: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(|c| c as char)
        .collect()
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Serialize, Deserialize)]
struct PolicyPayload {
    policy: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct PermissionsPayload {
    permissions: Value,
}

fn decode_policy(token: &str, public_key: &str) -> Result<Value, String> {
    let decoding_key = DecodingKey::from_rsa_pem(public_key.as_bytes())
        .map_err(|e| format!("invalid public key: {}", e))?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    validation.required_spec_claims.clear();

    decode::<PolicyPayload>(token, &decoding_key, &validation)
        .map(|data| data.claims.policy)
        .map_err(|e| format!("decode policy failed: {}", e))
}

fn decode_permissions(token: &str, public_key: &str) -> Result<Value, String> {
    let decoding_key = DecodingKey::from_rsa_pem(public_key.as_bytes())
        .map_err(|e| format!("invalid public key: {}", e))?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    validation.required_spec_claims.clear();

    decode::<PermissionsPayload>(token, &decoding_key, &validation)
        .map(|data| data.claims.permissions)
        .map_err(|e| format!("decode permissions failed: {}", e))
}

fn save_permissions(token: &str) {
    if let Ok(mut file) = std::fs::File::create("permissions") {
        let _ = std::io::Write::write_all(&mut file, token.as_bytes());
    }
}

// ========== 项目管理函数 ==========

#[tauri::command]
fn add_project(
    state: State<'_, AppState>,
    id: String,
    name: String,
    url: String,
) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    // 创建新项目
    let project = Project {
        id: id.clone(),
        name: name.clone(),
        url: url.clone(),
        created_at: current_timestamp(),
        updated_at: current_timestamp(),
    };

    // 添加到配置中
    if !state.config["projects"].is_array() {
        state.config["projects"] = json!([]);
    }

    if let Value::Array(ref mut projects) = state.config["projects"] {
        // 检查是否已存在相同ID的项目
        for existing_project in projects.iter() {
            if existing_project["id"].as_str() == Some(&id) {
                return Err("项目ID已存在".to_string());
            }
        }

        let project_json =
            serde_json::to_value(&project).map_err(|e| format!("序列化项目失败: {}", e))?;
        projects.push(project_json);
    }

    // 保存配置
    if let Err(e) = state.config.save_config() {
        log::error!("保存项目失败: {}", e);
        return Err(format!("保存项目失败: {}", e));
    }

    log::info!("项目添加成功: ID={}, 名称={}", id, name);
    Ok(())
}

#[tauri::command]
fn get_projects(state: State<'_, AppState>) -> Result<Vec<Project>, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    // 从配置中获取项目列表
    let projects = if state.config["projects"].is_array() {
        let projects_json = &state.config["projects"];
        serde_json::from_value(projects_json.clone())
            .map_err(|e| format!("解析项目列表失败: {}", e))?
    } else {
        Vec::new()
    };

    Ok(projects)
}

#[tauri::command]
fn delete_project(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    if !state.config["projects"].is_array() {
        return Err("项目列表不存在".to_string());
    }

    if let Value::Array(ref mut projects) = state.config["projects"] {
        let original_len = projects.len();
        projects.retain(|project| project["id"].as_str() != Some(&id));

        if projects.len() == original_len {
            return Err("未找到指定ID的项目".to_string());
        }

        // 保存配置
        if let Err(e) = state.config.save_config() {
            log::error!("删除项目后保存失败: {}", e);
            return Err(format!("删除项目后保存失败: {}", e));
        }

        log::info!("项目删除成功: ID={}", id);
        Ok(())
    } else {
        Err("项目列表格式错误".to_string())
    }
}

// ========== 监控统计函数 ==========

#[tauri::command]
fn get_monitor_stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock失败".to_string())?;

    // 简化实现：返回基本统计信息
    // TODO: 未来从任务管理器获取实时统计
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
fn get_recent_logs(state: State<'_, AppState>, count: usize) -> Result<Vec<String>, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    let logs: Vec<String> = state.logs.iter().rev().take(count).cloned().collect();

    Ok(logs)
}

#[tauri::command]
fn save_settings(
    state: State<'_, AppState>,
    grab_mode: u8,
    delay_time: usize,
    max_attempts: i32,
    enable_push: bool,
    enabled_methods: Vec<String>,
    bark_token: String,
    pushplus_token: String,
    fangtang_token: String,
    dingtalk_token: String,
    wechat_token: String,
    gotify_url: String,
    gotify_token: String,
    smtp_server: String,
    smtp_port: String,
    smtp_username: String,
    smtp_password: String,
    smtp_from: String,
    smtp_to: String,
    custom_ua: bool,
    user_agent: String,
) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    state.grab_mode = grab_mode;

    state.status_delay = delay_time;

    state.config["max_attempts"] = json!(max_attempts);
    log::info!("最大尝试次数设置: {}", max_attempts);

    state.push_config.enabled = enable_push;
    state.push_config.enabled_methods = enabled_methods;
    state.push_config.bark_token = bark_token;
    state.push_config.pushplus_token = pushplus_token;
    state.push_config.fangtang_token = fangtang_token;
    state.push_config.dingtalk_token = dingtalk_token;
    state.push_config.wechat_token = wechat_token;

    state.push_config.gotify_config.gotify_url = gotify_url;
    state.push_config.gotify_config.gotify_token = gotify_token;

    state.push_config.smtp_config.smtp_server = smtp_server;
    state.push_config.smtp_config.smtp_port = smtp_port;
    state.push_config.smtp_config.smtp_username = smtp_username;
    state.push_config.smtp_config.smtp_password = smtp_password;
    state.push_config.smtp_config.smtp_from = smtp_from;
    state.push_config.smtp_config.smtp_to = smtp_to;

    state.custom_config.open_custom_ua = custom_ua;
    state.custom_config.custom_ua = user_agent.clone();

    if custom_ua && !user_agent.is_empty() {
        state.default_ua = user_agent.clone();
        let new_client = create_client(user_agent.clone());
        state.client = new_client;
    }

    if let Err(e) = state.config.save_config() {
        log::error!("保存配置失败: {}", e);
        return Err(format!("保存配置失败: {}", e));
    }

    log::info!("设置已保存!");
    Ok(())
}

#[tauri::command]
fn clear_logs(state: State<'_, AppState>) -> Result<(), String> {
    let mut state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    state.logs.clear();

    log::info!("日志已清空");
    Ok(())
}

#[tauri::command]
fn poll_qrcode_status(
    state: State<'_, AppState>,
    key: String,
) -> Result<serde_json::Value, String> {
    let state = state
        .inner
        .lock()
        .map_err(|_| "state lock failed".to_string())?;

    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("创建运行时失败: {}", e))?;

    let status =
        rt.block_on(async { backend::api::poll_qrcode_login(&key, Some(&state.default_ua)).await });

    match status {
        common::login::QrCodeLoginStatus::Pending => Ok(json!({
            "status": "pending",
            "message": "二维码已生成，等待扫描",
            "key": key
        })),
        common::login::QrCodeLoginStatus::Scanning => Ok(json!({
            "status": "scanning",
            "message": "二维码已扫描，等待确认",
            "key": key
        })),
        common::login::QrCodeLoginStatus::Confirming => Ok(json!({
            "status": "confirming",
            "message": "二维码已确认，正在登录",
            "key": key
        })),
        common::login::QrCodeLoginStatus::Success(cookie) => Ok(json!({
            "status": "success",
            "message": "登录成功",
            "key": key,
            "cookie": cookie
        })),
        common::login::QrCodeLoginStatus::Failed(error) => Ok(json!({
            "status": "error",
            "message": format!("登录失败: {}", error),
            "key": key
        })),
        common::login::QrCodeLoginStatus::Expired => Ok(json!({
            "status": "expired",
            "message": "二维码已过期",
            "key": key
        })),
    }
}

fn main() {
    if let Err(e) = common::init_logger() {
        eprintln!("初始化日志失败，原因: {}", e);
    }
    log::info!("日志初始化成功");

    std::panic::set_hook(Box::new(|panic_info| {
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            if s.contains("swap") || s.contains("vsync") {
                log::warn!("图形渲染非致命错误: {}", s);
            } else {
                log::error!("程序panic: {}", panic_info);
            }
        } else {
            log::error!("程序panic: {}", panic_info);
        }
    }));

    if !common::utils::ensure_single_instance() {
        eprintln!("程序已经在运行中，请勿重复启动！");
        std::thread::sleep(std::time::Duration::from_secs(5));
        std::process::exit(1);
    }

    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            get_accounts,
            reload_accounts,
            add_account_by_cookie,
            delete_account_by_uid,
            set_account_active,
            qrcode_login,
            sms_login,
            submit_sms_code,
            get_ticket_info,
            get_buyer_info,
            get_order_list,
            poll_task_results,
            push_test,
            get_policy,
            get_logs,
            get_grab_logs,
            add_log,
            get_app_info,
            clear_grab_logs,
            cancel_task,
            set_ticket_id,
            set_grab_mode,
            set_selected_account,
            set_show_screen_info,
            set_confirm_ticket_info,
            set_show_add_buyer_window,
            set_show_orderlist_window,
            set_show_qr_windows,
            set_login_method,
            set_show_login_window,
            set_login_input,
            set_cookie_login,
            start_grab_ticket,
            set_delete_account,
            set_account_switch,
            set_selected_screen,
            set_selected_ticket,
            set_selected_buyer_list,
            set_skip_words,
            set_skip_words_input,
            get_state,
            add_project,
            get_projects,
            delete_project,
            get_monitor_stats,
            get_recent_logs,
            save_settings,
            clear_logs,
            poll_qrcode_status,
            set_buyer_type,
            set_no_bind_buyer_info,
            clear_no_bind_buyer_info
        ])
        .run(tauri::generate_context!())
        .expect("tauri run failed");
}

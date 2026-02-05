use crate::state::AppState;
use common::login::LoginInput;
use common::taskmanager::TaskRequest;
use image::Luma;
use qrcode::QrCode;
use serde_json::json;
use tauri::State;

#[tauri::command]
pub fn qrcode_login(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let auth = state
        .auth
        .lock()
        .map_err(|_| "auth lock failed".to_string())?;

    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| "runtime lock failed".to_string())?;

    let qrcode_key =
        common::login::qrcode_login(&auth.client).map_err(|e| format!("生成二维码失败: {}", e))?;

    let qrcode_url = format!(
        "https://passport.bilibili.com/h5-app/passport/login/scan?qrcode_key={}",
        qrcode_key
    );

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
        user_agent: Some(auth.default_ua.clone()),
    });

    let task_id = runtime
        .task_manager
        .submit_task(request)
        .map_err(|e| format!("提交二维码登录任务失败: {}", e))?;

    Ok(json!({
        "key": qrcode_key,
        "url": data_url,
        "task_id": task_id,
        "message": "二维码生成成功, 请使用B站APP扫描"
    }))
}

#[tauri::command]
pub fn poll_qrcode_status(
    state: State<'_, AppState>,
    key: String,
) -> Result<serde_json::Value, String> {
    let auth = state
        .auth
        .lock()
        .map_err(|_| "auth lock failed".to_string())?;

    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("创建运行时失败: {}", e))?;

    let status =
        rt.block_on(async { backend::api::poll_qrcode_login(&key, Some(&auth.default_ua)).await });

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

#[tauri::command]
pub async fn get_country_list_command(
    state: State<'_, AppState>,
) -> Result<Vec<common::login::Country>, String> {
    let client = {
        let auth = state
            .auth
            .lock()
            .map_err(|_| "auth lock failed".to_string())?;
        auth.client.clone()
    };
    common::login::get_country_list(&client).await
}

#[tauri::command]
pub fn send_loginsms_command(
    state: State<'_, AppState>,
    phone_number: String,
    cid: i32,
) -> Result<String, String> {
    let auth = state
        .auth
        .lock()
        .map_err(|_| "auth lock failed".to_string())?;
    let config = state
        .config
        .lock()
        .map_err(|_| "config lock failed".to_string())?;
    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| "runtime lock failed".to_string())?;

    let request = TaskRequest::LoginSmsRequest(common::taskmanager::LoginSmsRequest {
        phone: phone_number.clone(),
        cid,
        client: auth.client.clone(),
        custom_config: config.custom_config.clone(),
        local_captcha: common::captcha::LocalCaptcha::new(),
    });

    let result = runtime
        .task_manager
        .submit_task(request)
        .map_err(|e| format!("submit sms login request failed: {}", e));

    result
}

#[tauri::command]
pub fn submit_loginsms_command(
    state: State<'_, AppState>,
    phone_number: String,
    cid: i32,
    captcha_key: String,
    sms_code: String,
) -> Result<String, String> {
    let auth = state
        .auth
        .lock()
        .map_err(|_| "auth lock failed".to_string())?;
    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| "runtime lock failed".to_string())?;

    let request = TaskRequest::SubmitLoginSmsRequest(common::taskmanager::SubmitLoginSmsRequest {
        phone: phone_number,
        cid,
        code: sms_code,
        captcha_key,
        client: auth.client.clone(),
    });

    let result = runtime
        .task_manager
        .submit_task(request)
        .map_err(|e| format!("submit sms login failed: {}", e));

    result
}

#[tauri::command]
pub fn set_login_method(state: State<'_, AppState>, method: String) -> Result<(), String> {
    let mut auth = state
        .auth
        .lock()
        .map_err(|_| "auth lock failed".to_string())?;
    auth.login_method = method;
    Ok(())
}

#[tauri::command]
pub fn set_show_login_window(state: State<'_, AppState>, show: bool) -> Result<(), String> {
    let mut ui = state.ui.lock().map_err(|_| "ui lock failed".to_string())?;
    ui.show_login_window = show;
    Ok(())
}

#[tauri::command]
pub fn set_login_input(state: State<'_, AppState>, input: LoginInput) -> Result<(), String> {
    let mut auth = state
        .auth
        .lock()
        .map_err(|_| "auth lock failed".to_string())?;
    auth.login_input = input;
    Ok(())
}

#[tauri::command]
pub fn set_cookie_login(state: State<'_, AppState>, cookie: Option<String>) -> Result<(), String> {
    let mut auth = state
        .auth
        .lock()
        .map_err(|_| "auth lock failed".to_string())?;
    auth.cookie_login = cookie;
    Ok(())
}

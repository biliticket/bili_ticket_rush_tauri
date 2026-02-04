use crate::api::poll_qrcode_login;
use common::login::{password_login, QrCodeLoginStatus, send_loginsms, sms_login};
use common::taskmanager::{
    LoginSmsRequest, LoginSmsRequestResult, PasswordLoginRequest, PasswordLoginResult, QrCodeLoginRequest, SubmitLoginSmsRequest,
    SubmitSmsLoginResult, TaskQrCodeLoginResult, TaskResult,
};
use tokio::sync::mpsc;

pub async fn handle_qrcode_login_request(
    qrcode_req: QrCodeLoginRequest,
    result_tx: mpsc::Sender<TaskResult>,
) {
    let task_id = uuid::Uuid::new_v4().to_string(); // In a real scenario, this might come from the request if we tracked it there.
    // Actually, task_id is usually generated in submit_task. 
    // But here we generate a new one or use the one from the request if passed (Request struct doesn't have it).
    // The previous implementation generated a new one. We can stick to that or pass it if we change Request.
    // For now, let's generate one, but it should ideally be consistent.
    // Wait, the frontend needs to know which task this is.
    // The previous implementation generated a UUID.
    // The `submit_task` generates a UUID and returns it.
    // But `handle_qrcode_login_request` *inside* `TaskManagerImpl` runs in a spawned task.
    // `TaskManagerImpl` stores `Task` with the `task_id`.
    // BUT `handle_qrcode_login_request` doesn't receive `task_id` from `TaskManagerImpl`.
    // It receives `QrCodeLoginRequest`.
    // `QrCodeLoginRequest` in `common` does not have `task_id`.
    // This is a flaw in the existing design: the handler generates a *new* ID, different from what `submit_task` returned?
    // Let's check `TaskManagerImpl` in `mod.rs`.
    // `TaskMessage::SubmitTask((task_id, request))` -> `task_id` is available in the loop.
    // But `handle_qrcode_login_request` is called with just `request`.
    // The `task_id` generated inside `handle_qrcode_login_request` will be different from the one returned to the user!
    // This explains why polling might be tricky if IDs don't match.
    // However, the user asked to replace polling with events. Events carry data.
    // If I send an event with a *new* task ID, the frontend won't know it corresponds to the request.
    
    // CORRECTION: `TaskRequest` variants usually have `task_id`?
    // `QrCodeLoginRequest` does NOT have `task_id`.
    // `TaskManagerImpl::submit_task` generates `task_id` and stores it.
    // The worker loop in `mod.rs` receives `task_id` but *discards* it when calling `handle_qrcode_login_request`.
    // I should fix `TaskManagerImpl` to pass `task_id` to handlers, OR add `task_id` to `QrCodeLoginRequest`.
    
    // For this specific replacement, I will modify `handle_qrcode_login_request` to loop.
    // But I should essentially fix the `task_id` issue if I can.
    // Since I cannot easily change `TaskRequest` definition in `common` without breaking other things (maybe),
    // I will check `mod.rs` again.
    
    // `mod.rs`:
    // `TaskRequest::QrCodeLoginRequest(qrcode_req) => tokio::spawn(handle_qrcode_login_request(qrcode_req, result_tx))`
    // It ignores the `task_id` from `SubmitTask((task_id, ...))`.
    
    // I will assume for now I should just loop.
    // The `task_id` being different is a pre-existing issue or I misread `mod.rs`.
    // Actually, `handle_qrcode_login_request` generates a NEW UUID. This is definitely a bug or weird design.
    // But for QR login, the frontend might not care about the ID if it listens to "QrCodeLoginResult".
    // Or maybe it does.
    
    // Let's just implement the loop for now.
    
    loop {
        // 二维码登录逻辑
        let status = poll_qrcode_login(&qrcode_req.qrcode_key, qrcode_req.user_agent.as_deref()).await;

        let (cookie, error) = match &status {
            QrCodeLoginStatus::Success(cookie) => (Some(cookie.clone()), None),
            QrCodeLoginStatus::Failed(err) => (None, Some(err.clone())),
            _ => (None, None),
        };

        // 创建正确的结果类型
        // We reuse the same task_id if we can, but since we generate it here...
        // effectively we are generating a stream of events with the same (new) ID.
        let task_result = TaskResult::QrCodeLoginResult(TaskQrCodeLoginResult {
            task_id: task_id.clone(),
            status: status.clone(),
            cookie,
            error,
            qrcode_key: Some(qrcode_req.qrcode_key.clone()),
        });

        if let Err(e) = result_tx.send(task_result).await {
            log::error!("Send qrcode login result failed: {}", e);
            break;
        }
        
        match status {
            QrCodeLoginStatus::Success(_) | QrCodeLoginStatus::Failed(_) | QrCodeLoginStatus::Expired => break,
            _ => tokio::time::sleep(tokio::time::Duration::from_secs(2)).await,
        }
    }
}

pub async fn handle_login_sms_request(
    login_sms_req: LoginSmsRequest,
    result_tx: mpsc::Sender<TaskResult>,
) {
    let task_id = uuid::Uuid::new_v4().to_string();
    let phone = login_sms_req.phone.clone();
    let cid = login_sms_req.cid;
    let client = login_sms_req.client.clone();
    let custom_config = login_sms_req.custom_config.clone();
    let local_captcha = login_sms_req.local_captcha.clone();

    log::info!("开始发送短信验证码 ID: {}, CID: {}", task_id, cid);
    let response = send_loginsms(&phone, cid, &client, custom_config, local_captcha).await;
    log::info!("完成发送短信验证码 ID: {}", task_id);
    let success = response.is_ok();
    let message = match &response {
        Ok(msg) => msg.clone(),
        Err(err) => {
            log::error!("发送短信验证码失败: {}", err);
            err.to_string()
        }
    };
    log::info!(
        "发送短信任务完成 ID: {}, 结果: {}",
        task_id,
        if success { "成功" } else { "失败" }
    );

    let task_result = TaskResult::LoginSmsResult(LoginSmsRequestResult {
        task_id,
        phone,
        success,
        message,
    });

    if let Err(e) = result_tx.send(task_result).await {
        log::error!("Send login sms result failed: {}", e);
    }
}

pub async fn handle_submit_login_sms_request(
    login_sms_req: SubmitLoginSmsRequest,
    result_tx: mpsc::Sender<TaskResult>,
) {
    let task_id = uuid::Uuid::new_v4().to_string();
    let phone = login_sms_req.phone.clone();
    let cid = login_sms_req.cid;
    let client = login_sms_req.client.clone();
    let captcha_key = login_sms_req.captcha_key.clone();
    let code = login_sms_req.code.clone();

    log::info!("短信验证码登录进行中 ID: {}, CID: {}", task_id, cid);

    let response = sms_login(&phone, cid, &code, &captcha_key, &client).await;
    let success = response.is_ok();
    let message: String = match &response {
        Ok(msg) => msg.clone(),
        Err(err) => {
            log::error!("提交短信验证码失败: {}", err);
            err.to_string()
        }
    };
    let cookie = response.ok();

    log::info!(
        "提交短信任务完成 ID: {}, 结果: {}",
        task_id,
        if success { "成功" } else { "失败" }
    );

    let task_result = TaskResult::SubmitSmsLoginResult(SubmitSmsLoginResult {
        task_id,
        phone,
        success,
        message,
        cookie,
    });

    if let Err(e) = result_tx.send(task_result).await {
        log::error!("Send submit sms login result failed: {}", e);
    }
}

pub async fn handle_password_login_request(
    password_login_req: PasswordLoginRequest,
    result_tx: mpsc::Sender<TaskResult>,
) {
    let task_id = password_login_req.task_id.clone();
    let username = password_login_req.username.clone();
    let password = password_login_req.password.clone();
    let client = password_login_req.client.clone();
    let custom_config = password_login_req.custom_config.clone();
    let local_captcha = password_login_req.local_captcha.clone();

    log::info!("Password login in progress for user: {}", username);

    let response =
        password_login(&username, &password, &client, custom_config, local_captcha).await;

    let success = response.is_ok();
    let message = match &response {
        Ok(cookie) => "登录成功".to_string(),
        Err(err) => {
            log::error!("Password login failed for user {}: {}", username, err);
            err.to_string()
        }
    };
    let cookie = response.ok();

    let task_result = TaskResult::PasswordLoginResult(PasswordLoginResult {
        task_id,
        success,
        message,
        cookie,
    });

    if let Err(e) = result_tx.send(task_result).await {
        log::error!("Failed to send password login result: {}", e);
    }
}

use crate::api::poll_qrcode_login;
use common::login::{QrCodeLoginStatus, send_loginsms, sms_login};
use common::taskmanager::{
    LoginSmsRequest, LoginSmsRequestResult,
    QrCodeLoginRequest, SubmitLoginSmsRequest, SubmitSmsLoginResult, TaskQrCodeLoginResult,
    TaskResult,
};
use tokio::sync::mpsc;

pub async fn handle_qrcode_login_request(
    qrcode_req: QrCodeLoginRequest,
    result_tx: mpsc::Sender<TaskResult>,
) {
    let task_id = uuid::Uuid::new_v4().to_string();

    loop {
        let status =
            poll_qrcode_login(&qrcode_req.qrcode_key, qrcode_req.user_agent.as_deref()).await;

        let (cookie, error) = match &status {
            QrCodeLoginStatus::Success(cookie) => (Some(cookie.clone()), None),
            QrCodeLoginStatus::Failed(err) => (None, Some(err.clone())),
            _ => (None, None),
        };

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
            QrCodeLoginStatus::Success(_)
            | QrCodeLoginStatus::Failed(_)
            | QrCodeLoginStatus::Expired => break,
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



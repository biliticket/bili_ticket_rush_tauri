use crate::state::AppState;
use common::captcha::LocalCaptcha;
use common::taskmanager::{
    GetAllorderRequest, GetBuyerInfoRequest, GetTicketInfoRequest, GrabTicketRequest, TaskRequest,
    TaskResult, TaskStatus,
};
use common::ticket::BilibiliTicket;
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

#[tauri::command]
pub fn get_ticket_info(
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
pub fn get_buyer_info(state: State<'_, AppState>, uid: i64) -> Result<String, String> {
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
pub fn get_order_list(state: State<'_, AppState>, uid: i64) -> Result<String, String> {
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
pub fn poll_task_results(state: State<'_, AppState>) -> Result<Value, String> {
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
            TaskResult::QrCodeLoginResult(r) => json!({
                "type": "QrCodeLoginResult",
                "task_id": r.task_id,
                "status": format!("{:?}", r.status),
                "cookie": r.cookie,
                "error": r.error
            }),
            TaskResult::LoginSmsResult(r) => json!({
                "type": "LoginSmsResult",
                "success": r.success,
                "message": r.message
            }),
            TaskResult::SubmitSmsLoginResult(r) => json!({
                "type": "SubmitSmsLoginResult",
                "success": r.success,
                "message": r.message,
                "cookie": r.cookie
            }),
            TaskResult::PushResult(r) => json!({
                "type": "PushResult",
                "success": r.success,
                "message": r.message
            }),
            TaskResult::GetAllorderRequestResult(r) => json!({
                "type": "GetAllorderRequestResult",
                "task_id": r.task_id,
                "success": r.success,
                "account_id": r.account_id,
                "message": r.message,
                "order_info": r.order_info
            }),
            TaskResult::GetTicketInfoResult(r) => {
                let mut success = r.success;
                let mut message = r.message.clone();

                if success {
                    if let Some(ticket_info) = &r.ticket_info {
                        if ticket_info.data.vip_exclusive {
                            // 检查账号是否有大会员
                            let is_vip = state
                                .accounts
                                .iter()
                                .find(|a| a.uid == r.uid)
                                .map(|a| a.vip_status == 1)
                                .unwrap_or(false);

                            if !is_vip {
                                success = false;
                                message = "该项目为大会员专属，您的账号未开通大会员".to_string();
                            }
                        }
                    }
                }

                json!({
                    "type": "GetTicketInfoResult",
                    "task_id": r.task_id,
                    "success": success,
                    "uid": r.uid,
                    "message": message,
                    "ticket_info": r.ticket_info
                })
            }
            TaskResult::GetBuyerInfoResult(r) => json!({
                "type": "GetBuyerInfoResult",
                "task_id": r.task_id,
                "success": r.success,
                "uid": r.uid,
                "message": r.message,
                "buyer_info": r.buyer_info
            }),
            TaskResult::GrabTicketResult(r) => json!({
                "type": "GrabTicketResult",
                "task_id": r.task_id,
                "success": r.success,
                "order_id": r.order_id,
                "message": r.message,
                "pay_result": r.pay_result,
                "confirm_result": r.confirm_result
            }),
            TaskResult::PasswordLoginResult(r) => json!({ // Add this
                "type": "PasswordLoginResult",
                "task_id": r.task_id,
                "success": r.success,
                "message": r.message,
                "cookie": r.cookie
            }),
        })
        .collect();

    Ok(json!(json_results))
}

#[tauri::command]
pub fn cancel_task(state: State<'_, AppState>, task_id: String) -> Result<(), String> {
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
pub fn start_grab_ticket(state: State<'_, AppState>) -> Result<String, String> {
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
    let grab_request = TaskRequest::GrabTicketRequest(GrabTicketRequest {
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
        buyer_info: state.selected_buyer_list.clone().unwrap_or_default(),
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

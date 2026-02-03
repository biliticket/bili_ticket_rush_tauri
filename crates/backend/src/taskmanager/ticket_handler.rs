use crate::api::{get_buyer_info, get_project};
use common::taskmanager::{
    GetBuyerInfoRequest, GetBuyerInfoResult, GetTicketInfoRequest, GetTicketInfoResult, TaskResult,
};
use tokio::sync::mpsc;

pub async fn handle_get_ticket_info_request(
    get_ticketinfo_req: GetTicketInfoRequest,
    result_tx: mpsc::Sender<TaskResult>,
) {
    let cookie_manager = get_ticketinfo_req.cookie_manager.clone();
    let task_id = get_ticketinfo_req.task_id.clone();
    let project_id = get_ticketinfo_req.project_id.clone();
    let uid = get_ticketinfo_req.uid.clone();
    log::debug!("正在获取project{}", task_id);
    let response = get_project(cookie_manager, &project_id).await;
    let success = response.is_ok();
    let ticket_info = match &response {
        Ok(info) => {
            if info.data.screen_list.is_empty() {
                log::warn!("项目信息获取成功但场次列表为空，可能是API格式变化");
            }
            Some(info.clone())
        }
        Err(e) => {
            log::error!("获取项目时失败，原因：{}", e);
            None
        }
    };
    let message = match &response {
        Ok(info) => {
            format!("项目{}请求成功", info.errno)
        }
        Err(e) => e.to_string(),
    };
    let task_result = TaskResult::GetTicketInfoResult(GetTicketInfoResult {
        task_id: task_id.clone(),
        uid: uid.clone(),
        ticket_info: ticket_info.clone(),
        success,
        message: message.clone(),
    });
    if let Err(e) = result_tx.send(task_result).await {
        log::error!("Send get ticket info result failed: {}", e);
    }
}

pub async fn handle_get_buyer_info_request(
    get_buyerinfo_req: GetBuyerInfoRequest,
    result_tx: mpsc::Sender<TaskResult>,
) {
    let cookie_manager = get_buyerinfo_req.cookie_manager.clone();
    let task_id = get_buyerinfo_req.task_id.clone();
    let uid = get_buyerinfo_req.uid.clone();
    log::debug!("正在获取购票人信息{}", task_id);
    let response = get_buyer_info(cookie_manager).await;
    let success = response.is_ok();
    let buyer_info = match &response {
        Ok(info) => Some(info.clone()),
        Err(e) => {
            log::error!("获取购票人信息失败，原因：{}", e);
            None
        }
    };
    let message = match &response {
        Ok(_) => "购票人信息请求成功".to_string(),
        Err(e) => e.to_string(),
    };
    let task_result = TaskResult::GetBuyerInfoResult(GetBuyerInfoResult {
        task_id: task_id.clone(),
        uid: uid.clone(),
        buyer_info: buyer_info.clone(),
        success,
        message: message.clone(),
    });
    if let Err(e) = result_tx.send(task_result).await {
        log::error!("Send get buyer info result failed: {}", e);
    }
}

use crate::show_orderlist::get_orderlist;
use common::taskmanager::{GetAllorderRequest, GetAllorderRequestResult, TaskResult};
use tokio::sync::mpsc;

pub async fn handle_get_all_order_request(
    get_order_req: GetAllorderRequest,
    result_tx: mpsc::Sender<TaskResult>,
) {
    let cookie_manager = get_order_req.cookie_manager.clone();
    let task_id = get_order_req.task_id;
    let account_id = get_order_req.account_id.clone();

    log::info!("正在获取全部订单 ID: {}", task_id);
    let response = get_orderlist(cookie_manager).await;
    let success = response.is_ok();

    let data = match &response {
        Ok(order_resp) => order_resp.clone(),
        Err(err) => {
            log::error!("获取全部订单失败: {}", err);
            let task_result = TaskResult::GetAllorderRequestResult(GetAllorderRequestResult {
                task_id,
                success: false,
                message: err.to_string(),
                order_info: None,
                account_id,
                timestamp: std::time::Instant::now(),
            });
            let _ = result_tx.send(task_result).await;
            return;
        }
    };

    let message = format!("获取全部订单成功: {}", data.data.total);

    let task_result = TaskResult::GetAllorderRequestResult(GetAllorderRequestResult {
        task_id,
        success,
        message,
        order_info: Some(data),
        account_id,
        timestamp: std::time::Instant::now(),
    });

    if let Err(e) = result_tx.send(task_result).await {
        log::error!("Send get all order result failed: {}", e);
    }
}

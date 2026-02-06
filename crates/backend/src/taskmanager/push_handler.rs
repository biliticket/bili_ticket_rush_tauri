use common::taskmanager::{PushRequest, PushRequestResult, PushType, TaskResult};
use tokio::sync::mpsc;

pub async fn handle_push_request(push_req: PushRequest, result_tx: mpsc::Sender<TaskResult>) {
    let task_id = uuid::Uuid::new_v4().to_string();
    let push_config = push_req.push_config.clone();
    let title = push_req.title.clone();
    let message = push_req.message.clone();
    let jump_url = push_req.jump_url.clone();
    let push_type = push_req.push_type.clone();

    log::info!("开始处理推送任务 ID: {}, 类型: {:?}", task_id, push_type);

    let (success, result_message) = match push_type {
        PushType::All => {
            push_config
                .push_all_async(&title, &message, &jump_url)
                .await
        }
        PushType::Dungeon => push_config.push_dungeon().await,
        _ => (false, "未实现的推送类型".to_string()),
    };

    let task_result = TaskResult::PushResult(PushRequestResult {
        task_id: task_id.clone(),
        success,
        message: result_message,
        push_type: push_type.clone(),
    });

    if let Err(e) = result_tx.send(task_result).await {
        log::error!("发送推送任务结果失败: {}", e);
    }

    log::info!(
        "推送任务 ID: {} 完成, 结果: {}",
        task_id,
        if success { "成功" } else { "失败" }
    );
}

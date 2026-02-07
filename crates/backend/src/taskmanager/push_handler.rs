use crate::dungeon::DungeonService;
use common::taskmanager::{PushRequest, PushRequestResult, PushType, TaskResult};
use std::sync::Arc;
use tokio::sync::mpsc;

pub async fn handle_push_request(
    push_req: PushRequest,
    result_tx: mpsc::Sender<TaskResult>,
    dungeon_service: Option<Arc<DungeonService>>,
) {
    let task_id = uuid::Uuid::new_v4().to_string();
    let push_config = push_req.push_config.clone();
    let title = push_req.title.clone();
    let message = push_req.message.clone();
    let jump_url = push_req.jump_url.clone();
    let push_type = push_req.push_type.clone();

    log::info!("开始处理推送任务 ID: {}, 类型: {:?}", task_id, push_type);

    let (success, result_message, dungeon_target_id) = match push_type {
        PushType::All => {
            let mut dungeon_handled = false;
            let mut dungeon_res = (false, String::new(), None);

            if push_config.enabled_methods.contains(&"dungeon".to_string())
                && push_config.dungeon_config.enabled
            {
                if let Some(service) = dungeon_service {
                    let dc = &push_config.dungeon_config;
                    log::info!("使用持久连接发送 Dungeon 脉冲...");
                    match service
                        .send_pulse(
                            dc.channel,
                            dc.intensity,
                            dc.frequency,
                            dc.pulse_ms,
                            dc.pause_ms,
                            dc.count,
                        )
                        .await
                    {
                        Ok(_) => {
                            dungeon_handled = true;
                            let tid = service.target_id.lock().await.clone();
                            dungeon_res = (true, "Dungeon 持久连接推送成功".to_string(), tid);
                        }
                        Err(e) => {
                            log::warn!("Dungeon 持久连接推送失败，将尝试重新连接: {}", e);
                        }
                    }
                }
            }

            let mut effective_config = push_config.clone();
            if dungeon_handled {
                effective_config.enabled_methods.retain(|m| m != "dungeon");
            }

            let (mut succ, msg, mut tid) = effective_config
                .push_all_async(&title, &message, &jump_url, Some(result_tx.clone()))
                .await;

            if dungeon_handled {
                succ = true;
                if tid.is_none() {
                    tid = dungeon_res.2;
                }
            }

            (succ, msg, tid)
        }
        _ => (false, "未实现的推送类型".to_string(), None),
    };

    let task_result = TaskResult::PushResult(PushRequestResult {
        task_id: task_id.clone(),
        success,
        message: result_message,
        push_type: push_type.clone(),
        dungeon_target_id,
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

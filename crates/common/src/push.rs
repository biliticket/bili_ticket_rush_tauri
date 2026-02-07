use crate::config::PushConfig;
use crate::taskmanager::{
    DungeonQrResult, PushRequest, PushType, TaskManager, TaskRequest, TaskResult,
};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

impl PushConfig {
    pub fn push_all(
        &self,
        title: &str,
        message: &str,
        jump_url: &Option<String>,
        task_manager: &mut dyn TaskManager,
    ) {
        if !self.enabled {
            return;
        }
        let push_request = TaskRequest::PushRequest(PushRequest {
            title: title.to_string(),
            message: message.to_string(),
            jump_url: jump_url.clone(),
            push_config: self.clone(),
            push_type: PushType::All,
        });
        match task_manager.submit_task(push_request) {
            Ok(task_id) => {
                log::debug!("提交全渠道推送任务成功，任务ID: {}", task_id);
            }
            Err(e) => {
                log::error!("提交推送任务失败: {}", e);
            }
        }
    }

    pub async fn push_all_async(
        &self,
        title: &str,
        message: &str,
        jump_url: &Option<String>,
        result_tx: Option<mpsc::Sender<TaskResult>>,
    ) -> (bool, String, Option<String>) {
        let mut success_count = 0;
        let mut failure_count = 0;
        let mut failures = Vec::new();
        let mut dungeon_target_id = None;

        if self.enabled_methods.contains(&"bark".to_string()) && !self.bark_token.is_empty() {
            let (success, msg) = self.push_bark(title, message).await;
            if success {
                success_count += 1;
            } else {
                failure_count += 1;
                failures.push(format!("Bark推送出错: {}", msg));
            }
        }

        if self.enabled_methods.contains(&"pushplus".to_string()) && !self.pushplus_token.is_empty()
        {
            let (success, msg) = self.push_pushplus(title, message).await;
            if success {
                success_count += 1;
            } else {
                failure_count += 1;
                failures.push(format!("PushPlus推送出错: {}", msg));
            }
        }

        if self.enabled_methods.contains(&"fangtang".to_string()) && !self.fangtang_token.is_empty()
        {
            let (success, msg) = self.push_fangtang(title, message).await;
            if success {
                success_count += 1;
            } else {
                failure_count += 1;
                failures.push(format!("Fangtang推送出错: {}", msg));
            }
        }

        if self.enabled_methods.contains(&"dingtalk".to_string()) && !self.dingtalk_token.is_empty()
        {
            let (success, msg) = self.push_dingtalk(title, message).await;
            if success {
                success_count += 1;
            } else {
                failure_count += 1;
                failures.push(format!("Dingtalk推送出错: {}", msg));
            }
        }

        if self.enabled_methods.contains(&"wechat".to_string()) && !self.wechat_token.is_empty() {
            let (success, msg) = self.push_wechat(title, message).await;
            if success {
                success_count += 1;
            } else {
                failure_count += 1;
                failures.push(format!("WeChat推送出错: {}", msg));
            }
        }

        if self.enabled_methods.contains(&"gotify".to_string())
            && !self.gotify_config.gotify_token.is_empty()
        {
            let (success, msg) = self.push_gotify(title, message, jump_url).await;
            if success {
                success_count += 1;
            } else {
                failure_count += 1;
                failures.push(format!("Gotify推送出错: {}", msg));
            }
        }

        if self.enabled_methods.contains(&"dungeon".to_string()) && self.dungeon_config.enabled {
            let (success, msg, target_id) = self.push_dungeon(result_tx).await;
            if success {
                success_count += 1;
                dungeon_target_id = target_id;
            } else {
                failure_count += 1;
                failures.push(format!("Dungeon推送出错: {}", msg));
            }
        }

        if success_count == 0 {
            return (
                false,
                format!(
                    "{} 成功 / {} 失败。失败详情: {}",
                    success_count,
                    failure_count,
                    failures.join("; ")
                ),
                dungeon_target_id,
            );
        } else {
            return (true, format!("{} 个渠道推送成功", success_count), dungeon_target_id);
        }
    }
    pub async fn push_gotify(
        &self,
        title: &str,
        message: &str,
        jump_url: &Option<String>,
    ) -> (bool, String) {
        let mut default_headers = reqwest::header::HeaderMap::new();
        let jump_url_real = jump_url
            .as_deref()
            .unwrap_or("bilibili://mall/web?url=https://www.bilibili.com");

        let push_target_url = if self.gotify_config.gotify_url.contains("http") {
            self.gotify_config.gotify_url.clone()
        } else {
            format!("http://{}", self.gotify_config.gotify_url)
        };
        default_headers.insert(
            "Content-Type",
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        default_headers.insert(
            "Authorization",
            reqwest::header::HeaderValue::from_str(&format!(
                "Bearer {}",
                self.gotify_config.gotify_token
            ))
            .unwrap(),
        );
        let client_builder = Client::builder()
            .default_headers(default_headers)
            .timeout(std::time::Duration::from_secs(20));
        let data = serde_json::json!({
            "message": message,
            "title": title,
            "priority": 9,
            "extras": {
            "client::notification": {
                "click": {"url": jump_url_real},
            },
            "android::action": {
                "onReceive": {"intentUrl": jump_url_real}
            }
        }
        });
        let client = match client_builder.build() {
            Ok(client) => client,
            Err(e) => return (false, format!("创建HTTP客户端失败: {}", e)),
        };
        let url = format!("{}/message", push_target_url);

        match client.post(&url).json(&data).send().await {
            Ok(resp) => {
                let status = resp.status();
                match resp.text().await {
                    Ok(text) => {
                        log::debug!("Gotify 推送响应: 状态码 {}, 内容: {}", status, text);
                        if status.is_success() {
                            (true, "推送成功".to_string())
                        } else {
                            (false, format!("推送失败，状态码: {}", status))
                        }
                    }
                    Err(e) => (false, format!("读取响应失败: {}", e)),
                }
            }
            Err(e) => (false, format!("推送失败: {}", e)),
        }
    }
    pub async fn push_bark(&self, title: &str, message: &str) -> (bool, String) {
        let client = Client::new();
        let data = serde_json::json!({
            "title":title,
            "body":message,
            "level":"timeSensitive",
            "badge":1,
            "icon":"https://sr.mihoyo.com/favicon-mi.ico",
            "group":"biliticket",
            "isArchive":1,

        });
        let url = format!("https://api.day.app/{}/", self.bark_token);
        match client.post(&url).json(&data).send().await {
            Ok(resp) => {
                let status = resp.status();
                match resp.text().await {
                    Ok(text) => {
                        log::debug!("Bark 推送响应: 状态码 {}, 内容: {}", status, text);
                        if status.is_success() {
                            (true, "推送成功".to_string())
                        } else {
                            (false, format!("推送失败，状态码: {}", status))
                        }
                    }
                    Err(e) => (false, format!("读取响应失败: {}", e)),
                }
            }
            Err(e) => (false, format!("推送失败: {}", e)),
        }
    }

    pub async fn push_pushplus(&self, title: &str, message: &str) -> (bool, String) {
        let client = Client::new();
        let url = "http://www.pushplus.plus/send";
        let data = serde_json::json!({
            "token":self.pushplus_token,
            "title":title,
            "content":message,
        });
        match client.post(url).json(&data).send().await {
            Ok(resp) => {
                let status = resp.status();
                match resp.text().await {
                    Ok(text) => {
                        log::debug!("PushPlus 推送响应: 状态码 {}, 内容: {}", status, text);
                        if status.is_success() {
                            (true, "推送成功".to_string())
                        } else {
                            (false, format!("推送失败，状态码: {}", status))
                        }
                    }
                    Err(e) => (false, format!("读取响应失败: {}", e)),
                }
            }
            Err(e) => (false, format!("推送失败: {}", e)),
        }
    }

    pub async fn push_fangtang(&self, title: &str, message: &str) -> (bool, String) {
        let client = Client::new();
        let url = format!("https://sctapi.ftqq.com/{}.send", self.fangtang_token);
        let data = serde_json::json!({
            "title":title,
            "desp":message,
            "noip":1
        });
        match client.post(url).json(&data).send().await {
            Ok(resp) => {
                let status = resp.status();
                match resp.text().await {
                    Ok(text) => {
                        log::debug!("Fangtang 推送响应: 状态码 {}, 内容: {}", status, text);
                        if status.is_success() {
                            (true, "推送成功".to_string())
                        } else {
                            (false, format!("推送失败，状态码: {}", status))
                        }
                    }
                    Err(e) => (false, format!("读取响应失败: {}", e)),
                }
            }
            Err(e) => (false, format!("推送失败: {}", e)),
        }
    }

    pub async fn push_dingtalk(&self, title: &str, message: &str) -> (bool, String) {
        let client = Client::new();
        let url = format!(
            "https://oapi.dingtalk.com/robot/send?access_token={}",
            self.dingtalk_token
        );
        let data = serde_json::json!({
            "msgtype":"text",
            "text":{
                "content":format!("{} \n {}", title, message)
            }
        });
        match client
            .post(url)
            .json(&data)
            .header("Content-Type", "application/json")
            .header("Charset", "UTF-8")
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                match resp.text().await {
                    Ok(text) => {
                        log::debug!("钉钉推送响应: 状态码 {}, 内容: {}", status, text);
                        if status.is_success() {
                            (true, "推送成功".to_string())
                        } else {
                            (false, format!("推送失败，状态码: {}", status))
                        }
                    }
                    Err(e) => (false, format!("读取响应失败: {}", e)),
                }
            }
            Err(e) => (false, format!("推送失败: {}", e)),
        }
    }

    pub async fn push_wechat(&self, title: &str, message: &str) -> (bool, String) {
        let client = Client::new();
        let url = format!(
            "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key={}",
            self.wechat_token
        );
        let data = serde_json::json!({
            "msgtype":"text",
            "text":{
                "content":format!("{} \n {}", title, message)
            }
        });
        match client
            .post(url)
            .json(&data)
            .header("Content-Type", "application/json")
            .header("Charset", "UTF-8")
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                match resp.text().await {
                    Ok(text) => {
                        log::debug!("微信推送响应: 状态码 {}, 内容: {}", status, text);
                        if status.is_success() {
                            (true, "推送成功".to_string())
                        } else {
                            (false, format!("推送失败，状态码: {}", status))
                        }
                    }
                    Err(e) => (false, format!("读取响应失败: {}", e)),
                }
            }
            Err(e) => (false, format!("推送失败: {}", e)),
        }
    }

    pub async fn push_dungeon(
        &self,
        result_tx: Option<mpsc::Sender<TaskResult>>,
    ) -> (bool, String, Option<String>) {
        let ws_url = "wss://ws.dungeon-lab.cn";
        let url = match Url::parse(ws_url) {
            Ok(u) => u,
            Err(e) => return (false, format!("解析WebSocket URL失败: {}", e), None),
        };

        log::info!("正在连接 Dungeon Socket 服务: {}", ws_url);

        let (ws_stream, _) = match connect_async(url).await {
            Ok(s) => s,
            Err(e) => {
                log::error!("连接 Socket 服务失败: {}", e);
                return (false, format!("连接 Socket 服务失败: {}", e), None);
            }
        };

        let (mut write, mut read) = ws_stream.split();
        let mut client_id = String::new();
        let mut target_id = String::new();
        let mut bound = false;

        let timeout = std::time::Duration::from_secs(60);
        let start_time = std::time::Instant::now();

        log::debug!("WebSocket 连接成功，等待服务器返回 Client ID...");

        while start_time.elapsed() < timeout {
            match tokio::time::timeout(std::time::Duration::from_secs(1), read.next()).await {
                Ok(Some(Ok(msg))) => {
                    if let Message::Text(text) = msg {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                            let msg_type = v["type"].as_str().unwrap_or("");
                            let message = v["message"].as_str().unwrap_or("");

                            if msg_type == "bind" {
                                if message == "targetId" {
                                    client_id = v["clientId"].as_str().unwrap_or("").to_string();
                                    log::debug!("获取到 Client ID: {}", client_id);

                                    let qr_content = format!(
                                        "https://www.dungeon-lab.com/app-download.php#DGLAB-SOCKET#wss://ws.dungeon-lab.cn/{}",
                                        client_id
                                    );
                                    if let Some(ref tx) = result_tx {
                                        let _ = tx
                                            .send(TaskResult::DungeonQrResult(DungeonQrResult {
                                                task_id: "".to_string(),
                                                qr_url: qr_content,
                                            }))
                                            .await;
                                    }
                                } else if message == "200"
                                    || !v["targetId"].as_str().unwrap_or("").is_empty()
                                {
                                    target_id = v["targetId"].as_str().unwrap_or("").to_string();
                                    log::debug!("App 绑定成功! Target ID: {}", target_id);
                                    bound = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                Ok(Some(Err(e))) => {
                    return (false, format!("WS 读取错误: {}", e), None);
                }
                Ok(None) => {
                    return (false, "WS 连接被服务器断开".to_string(), None);
                }
                Err(_) => {
                    // Timeout
                }
            }
        }

        if !bound {
            return (
                false,
                "等待 App 绑定超时，请确保 App 已扫描二维码并连接".to_string(),
                None,
            );
        }

        let channel_idx = self.dungeon_config.channel;
        let channel_char = if channel_idx == 0 { "A" } else { "B" };
        let clear_channel_idx = if channel_idx == 0 { "1" } else { "2" };

        let freq_val = (self.dungeon_config.frequency as u16).max(10).min(100);
        let intensity = self.dungeon_config.intensity.min(100);

        let hex_str = format!(
            "{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            freq_val, freq_val, freq_val, freq_val, intensity, intensity, intensity, intensity
        );

        let pulse_duration = self.dungeon_config.pulse_ms.max(100);
        let num_chunks = (pulse_duration + 99) / 100;
        let num_chunks = num_chunks.min(100);

        let mut wave_data = Vec::new();
        for _ in 0..num_chunks {
            wave_data.push(hex_str.clone());
        }

        let clear_msg = json!({
            "type": "msg",
            "clientId": client_id,
            "targetId": target_id,
            "message": format!("clear-{}", clear_channel_idx)
        });

        if let Err(e) = write.send(Message::Text(clear_msg.to_string())).await {
            return (false, format!("发送清除指令失败: {}", e), None);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        log::debug!("开始执行脉冲循环: {} 次", self.dungeon_config.count);

        for i in 0..self.dungeon_config.count {
            log::debug!("发送第 {}/{} 次脉冲", i + 1, self.dungeon_config.count);
            let pulse_msg = json!({
                "type": "msg",
                "clientId": client_id,
                "targetId": target_id,
                "message": format!("pulse-{}:{}", channel_char, serde_json::to_string(&wave_data).unwrap())
            });

            if let Err(e) = write.send(Message::Text(pulse_msg.to_string())).await {
                log::error!("发送脉冲失败: {}", e);
                break;
            }

            let sleep_time = self.dungeon_config.pulse_ms + self.dungeon_config.pause_ms;
            tokio::time::sleep(std::time::Duration::from_millis(sleep_time)).await;
        }

        let _ = write.close().await;

        (true, "WebSocket 脉冲发送完成".to_string(), Some(target_id))
    }
}

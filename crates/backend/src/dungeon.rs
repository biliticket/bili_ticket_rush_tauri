use common::taskmanager::{DungeonQrResult, TaskResult};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::protocol::Message,
};
use url::Url;

type WsWriter = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

pub struct DungeonService {
    writer: Arc<Mutex<Option<WsWriter>>>,
    pub target_id: Arc<Mutex<Option<String>>>,
    pub client_id: Arc<Mutex<Option<String>>>,
}

impl DungeonService {
    pub fn new() -> Self {
        Self {
            writer: Arc::new(Mutex::new(None)),
            target_id: Arc::new(Mutex::new(None)),
            client_id: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn connect(&self, event_tx: mpsc::Sender<TaskResult>) -> Result<(), String> {
        {
            if self.target_id.lock().await.is_some() {
                return Ok(());
            }
        }

        let url = Url::parse("wss://ws.dungeon-lab.cn").map_err(|e| e.to_string())?;
        log::info!("正在连接 Dungeon Socket 服务: {}", url);

        let (ws_stream, _) = connect_async(url.to_string())
            .await
            .map_err(|e| e.to_string())?;
        let (mut write, mut read) = ws_stream.split();

        let writer_clone = self.writer.clone();
        let target_id_clone = self.target_id.clone();
        let client_id_clone = self.client_id.clone();

        tokio::spawn(async move {
            let mut bound = false;

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        let text_str = text.to_string();
                        if let Ok(v) = serde_json::from_str::<Value>(&text_str) {
                            let msg_type = v["type"].as_str().unwrap_or("");
                            let message = v["message"].as_str().unwrap_or("");

                            if msg_type == "bind" {
                                if message == "targetId" {
                                    let my_client_id_val = v["clientId"].as_str().unwrap_or("").to_string();
                                    *client_id_clone.lock().await = Some(my_client_id_val.clone());
                                    log::debug!("获取到 Client ID: {}", my_client_id_val);

                                    let qr_content = format!(
                                        "https://www.dungeon-lab.com/app-download.php#DGLAB-SOCKET#wss://ws.dungeon-lab.cn/{}",
                                        my_client_id_val
                                    );
                                    
                                    let _ = event_tx.send(TaskResult::DungeonQrResult(DungeonQrResult {
                                        task_id: "system".to_string(),
                                        qr_url: qr_content,
                                    })).await;

                                } else if message == "200" || !v["targetId"].as_str().unwrap_or("").is_empty() {
                                    let tid = v["targetId"].as_str().unwrap_or("").to_string();
                                    *target_id_clone.lock().await = Some(tid.clone());
                                    log::info!("Dungeon App 绑定成功! Target ID: {}", tid);
                                    bound = true;
                                    break;
                                }
                            }
                        }
                    }
                    Ok(Message::Ping(_)) => {
                        let _ = write.send(Message::Pong(vec![].into())).await;
                    }
                    Err(e) => {
                        log::error!("Dungeon WS 错误: {}", e);
                        return;
                    }
                    _ => {}
                }
            }

            if bound {
                *writer_clone.lock().await = Some(write);

                loop {
                    match read.next().await {
                        Some(Ok(Message::Close(_))) | None => {
                            log::warn!("Dungeon WS 连接断开");
                            break;
                        }
                        Some(Ok(Message::Ping(_))) => {
                            let mut w = writer_clone.lock().await;
                            if let Some(w) = w.as_mut() {
                                let _ = w.send(Message::Pong(vec![].into())).await;
                            }
                        }
                        Some(Err(e)) => {
                            log::error!("Dungeon WS 读取错误: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }

                *writer_clone.lock().await = None;
                *target_id_clone.lock().await = None;
            }
        });

        Ok(())
    }

    pub async fn send_pulse(
        &self,
        channel: u8,
        intensity: u8,
        frequency: u8,
        pulse_ms: u64,
        pause_ms: u64,
        count: u8,
    ) -> Result<String, String> {
        let writer_lock = self.writer.lock().await;
        if writer_lock.is_none() {
            return Err("未连接到 Dungeon 服务".to_string());
        }

        let client_id = self.client_id.lock().await.clone().unwrap_or_default();
        let target_id = self.target_id.lock().await.clone().unwrap_or_default();

        if target_id.is_empty() {
            return Err("未绑定 APP".to_string());
        }

        let channel_char = if channel == 0 { "A" } else { "B" };
        let clear_channel_idx = if channel == 0 { "1" } else { "2" };
        
        let freq_hz = (frequency as u16).max(10).min(100);
        let period_ms = 1000 / freq_hz;
        
        let freq_val = period_ms.max(10).min(100);
        let intensity_val = intensity.min(100);

        let hex_str = format!(
            "{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            freq_val,
            freq_val,
            freq_val,
            freq_val,
            intensity_val,
            intensity_val,
            intensity_val,
            intensity_val
        );

        let pulse_duration = pulse_ms.max(100);
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

        drop(writer_lock);
        let mut w_guard = self.writer.lock().await;

        if let Some(w) = w_guard.as_mut() {
            if let Err(e) = w.send(Message::Text(clear_msg.to_string().into())).await {
                return Err(format!("发送清除指令失败: {}", e));
            }
        } else {
            return Err("连接已断开".to_string());
        }

        drop(w_guard);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        log::debug!("开始执行脉冲循环: {} 次", count);

        for _ in 0..count {
            let pulse_msg = json!({
                "type": "msg",
                "clientId": client_id,
                "targetId": target_id,
                "message": format!("pulse-{}:{}", channel_char, serde_json::to_string(&wave_data).unwrap())
            });

            let mut w_guard = self.writer.lock().await;
            if let Some(w) = w_guard.as_mut() {
                if let Err(e) = w.send(Message::Text(pulse_msg.to_string().into())).await {
                    log::error!("发送脉冲失败: {}", e);
                    break;
                }
            } else {
                break;
            }
            drop(w_guard);

            let sleep_time = pulse_ms + pause_ms;
            tokio::time::sleep(std::time::Duration::from_millis(sleep_time)).await;
        }

        Ok("发送完成".to_string())
    }
}

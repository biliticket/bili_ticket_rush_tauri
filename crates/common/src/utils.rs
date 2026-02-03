use reqwest::Client;

// 单例锁实现，防止程序多开
use single_instance::SingleInstance;

// 简化后的单例检查实现
pub fn ensure_single_instance() -> bool {
    let app_id = "bili_ticket_rush_6BA7B79C-0E4F-4FCC-B7A2-4DA5E8D7E0F6";
    let instance = SingleInstance::new(app_id).unwrap();

    if !instance.is_single() {
        log::error!("程序已经在运行中，请勿重复启动！");
        eprintln!("程序已经在运行中，请勿重复启动！");
        std::thread::sleep(std::time::Duration::from_secs(2));
        false
    } else {
        Box::leak(Box::new(instance));
        true
    }
}

pub async fn get_now_time(client: &Client) -> i64 {
    let url = "https://api.bilibili.com/x/click-interface/click/now";

    if let Ok(response) = client.get(url).send().await {
        if let Ok(text) = response.text().await {
            if let Ok(json_data) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(now_sec) = json_data["data"]["now"].as_i64() {
                    log::debug!("解析出的网络时间(秒级)：{}", now_sec);
                    return now_sec;
                }
            }
        }
    }
    log::debug!("获取网络时间失败，使用本地时间");
    chrono::Utc::now().timestamp()
}

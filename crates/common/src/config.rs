use crate::account::Account;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::Path;

// --- Project Struct ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub url: String,
    pub created_at: u64,
    pub updated_at: u64,
}

// --- Main Config Struct ---
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BtrConfig {
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub projects: Vec<Project>, // Added projects
    #[serde(default)]
    pub push_config: PushConfig,
    #[serde(default)]
    pub custom_config: CustomConfig,
    #[serde(default)]
    pub grab_mode: u8,
    #[serde(default = "default_delay_time")]
    pub delay_time: u64,
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u64,
    #[serde(default)]
    pub permissions: Value,
    #[serde(default)]
    pub skip_words: Option<Vec<String>>,
}

fn default_delay_time() -> u64 {
    2
}
fn default_max_attempts() -> u64 {
    100
}

impl Default for BtrConfig {
    fn default() -> Self {
        BtrConfig {
            accounts: Vec::new(),
            projects: Vec::new(), // Added projects
            push_config: PushConfig::default(),
            custom_config: CustomConfig::default(),
            grab_mode: 0,
            delay_time: default_delay_time(),
            max_attempts: default_max_attempts(),
            permissions: Value::Null,
            skip_words: None,
        }
    }
}

impl BtrConfig {
    pub fn load_config() -> io::Result<Self> {
        if !Path::new("./config").exists() {
            return Ok(BtrConfig::default());
        }

        let raw_context = fs::read_to_string("./config")?;
        serde_json::from_str(&raw_context).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save_config(&self) -> io::Result<()> {
        let json_str = serde_json::to_string_pretty(self)?;

        let temp_path = "./config.tmp";
        fs::write(temp_path, json_str)?;
        fs::rename(temp_path, "./config")
    }

    pub fn add_account(&mut self, account: Account) {
        if !self.accounts.iter().any(|a| a.uid == account.uid) {
            self.accounts.push(account);
        }
    }

    pub fn load_accounts(&self) -> &Vec<Account> {
        &self.accounts
    }

    pub fn update_account(&mut self, account: &Account) -> bool {
        if let Some(existing_account) = self.accounts.iter_mut().find(|a| a.uid == account.uid) {
            *existing_account = account.clone();
            true
        } else {
            false
        }
    }

    pub fn delete_account(&mut self, uid: i64) -> bool {
        let old_len = self.accounts.len();
        self.accounts.retain(|acc| acc.uid != uid);
        let removed = self.accounts.len() != old_len;
        if removed {
            if let Err(e) = self.save_config() {
                log::error!("删除账号后保存配置失败: {}", e);
            }
        }
        removed
    }
}

// --- Push Config Structs ---
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushConfig {
    pub enabled: bool,
    pub enabled_methods: Vec<String>,
    pub bark_token: String,
    pub pushplus_token: String,
    pub fangtang_token: String,
    pub dingtalk_token: String,
    pub wechat_token: String,
    #[serde(default)]
    pub gotify_config: GotifyConfig,
}

impl Default for PushConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            enabled_methods: Vec::new(),
            bark_token: String::new(),
            pushplus_token: String::new(),
            fangtang_token: String::new(),
            dingtalk_token: String::new(),
            wechat_token: String::new(),
            gotify_config: GotifyConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GotifyConfig {
    pub gotify_url: String,
    pub gotify_token: String,
}

// --- Custom Config Struct ---
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomConfig {
    pub open_custom_ua: bool,
    pub custom_ua: String,
    pub captcha_mode: usize,
    pub ttocr_key: String,
    pub preinput_phone1: String,
    pub preinput_phone2: String,
}

impl Default for CustomConfig {
    fn default() -> Self {
        Self {
            open_custom_ua: true,
            custom_ua: String::from(
                "Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Mobile Safari/537.36",
            ),
            captcha_mode: 0,
            ttocr_key: String::new(),
            preinput_phone1: String::new(),
            preinput_phone2: String::new(),
        }
    }
}

use crate::account::Account;
use aes::Aes128;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use block_modes::block_padding::Pkcs7;
use block_modes::{BlockMode, Cbc};
use rand::Rng;
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
        }
    }
}

impl BtrConfig {
    pub fn load_config() -> io::Result<Self> {
        if !Path::new("./config").exists() {
            return Ok(BtrConfig::default());
        }

        let raw_context = fs::read_to_string("./config")?;
        let content: Vec<&str> = raw_context.split('%').collect();
        if content.len() != 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid config file format",
            ));
        }

        let iv = BASE64
            .decode(content[0].trim())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let decoded = BASE64
            .decode(content[1].trim())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let decrypted = decrypt_data(iv, &decoded)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let plain_text = String::from_utf8(decrypted)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        serde_json::from_str(&plain_text)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save_config(&self) -> io::Result<()> {
        let json_str = serde_json::to_string_pretty(self)?;
        let (iv, encrypted) =
            encrypt_data(json_str.as_bytes()).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let encoded_iv = BASE64.encode(&iv);
        let encoded_encrypted = BASE64.encode(&encrypted);
        let final_content = format!("{}%{}", encoded_iv, encoded_encrypted);

        let temp_path = "./config.tmp";
        fs::write(temp_path, final_content)?;
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
    #[serde(default)]
    pub smtp_config: SmtpConfig,
}

impl Default for PushConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            enabled_methods: vec![
                "bark".to_string(),
                "pushplus".to_string(),
                "fangtang".to_string(),
                "dingtalk".to_string(),
                "wechat".to_string(),
                "smtp".to_string(),
                "gotify".to_string(),
            ],
            bark_token: String::new(),
            pushplus_token: String::new(),
            fangtang_token: String::new(),
            dingtalk_token: String::new(),
            wechat_token: String::new(),
            gotify_config: GotifyConfig::default(),
            smtp_config: SmtpConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GotifyConfig {
    pub gotify_url: String,
    pub gotify_token: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SmtpConfig {
    pub smtp_server: String,
    pub smtp_port: String,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_from: String,
    pub smtp_to: String,
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

// --- Encryption Functions ---
fn gen_machine_id_bytes_128b() -> Vec<u8> {
    let id: String = machine_uid::get().unwrap_or_else(|_| "0123456789abcdef".to_string());
    let mut padded_id = id.into_bytes();
    padded_id.resize(16, 0);
    padded_id[..16].to_vec()
}

fn encrypt_data(data: &[u8]) -> Result<(Vec<u8>, Vec<u8>), block_modes::BlockModeError> {
    type Aes128Cbc = Cbc<Aes128, Pkcs7>;
    let mut iv = [0u8; 16];
    rand::thread_rng().fill(&mut iv[..]);
    let cipher = Aes128Cbc::new_from_slices(&gen_machine_id_bytes_128b(), &iv)
        .map_err(|_| block_modes::BlockModeError)?;

    Ok((iv.to_vec(), cipher.encrypt_vec(data)))
}

fn decrypt_data(iv: Vec<u8>, encrypted: &[u8]) -> Result<Vec<u8>, block_modes::BlockModeError> {
    type Aes128Cbc = Cbc<Aes128, Pkcs7>;
    let cipher = Aes128Cbc::new_from_slices(&gen_machine_id_bytes_128b(), &iv)
        .map_err(|_| block_modes::BlockModeError)?;

    cipher.decrypt_vec(encrypted)
}
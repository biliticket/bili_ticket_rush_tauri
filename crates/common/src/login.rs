use crate::account::Account;
use crate::account::add_account;
use crate::captcha::LocalCaptcha;
use crate::captcha::captcha;
use crate::config::CustomConfig;
use crate::http_utils::{request_get, request_post};
use reqwest::Client;
use serde_json::json;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rsa::{pkcs8::DecodePublicKey, Pkcs1v15Encrypt, RsaPublicKey};
use rand::rngs::OsRng;

#[derive(Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct LoginInput {
    pub phone: String,
    pub account: String,
    pub password: String,
    pub sms_code: String,
    pub cookie: String,
}

pub struct QrCodeLoginTask {
    pub qrcode_key: String,
    pub qrcode_url: String,
    pub start_time: std::time::Instant,
    pub status: QrCodeLoginStatus,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum QrCodeLoginStatus {
    Pending,
    Scanning,
    Confirming,
    Success(String), //成功时返回cookie信息
    Failed(String),  //失败时返回错误信息
    Expired,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SendLoginSmsStatus {
    Success(String),
    Failed(String),
}

pub fn qrcode_login(client: &Client) -> Result<String, String> {
    // 创建一个临时的运行时来执行异步代码
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let response = request_get(
            client,
            "https://passport.bilibili.com/x/passport-login/web/qrcode/generate",
            None,
        )
        .await
        .map_err(|e| e.to_string())?;

        let json = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| e.to_string())?;

        if let Some(qrcode_key) = json["data"]["qrcode_key"].as_str() {
            Ok(qrcode_key.to_string())
        } else {
            Err("无法获取二维码URL".to_string())
        }
    })
}
pub async fn password_login(
    username: &str,
    password: &str,
    client: &Client,
    custom_config: CustomConfig,
    local_captcha: LocalCaptcha,
) -> Result<String, String> {
    let (salt, public_key_str) = get_pubkey_and_salt(client).await?;
    let captcha_response = request_get(
        client,
        "https://passport.bilibili.com/x/passport-login/captcha",
        None,
    )
    .await
    .map_err(|e| e.to_string())?;

    let json_captcha = captcha_response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;

    let gt = json_captcha["data"]["geetest"]["gt"].as_str().unwrap_or("");
    let challenge = json_captcha["data"]["geetest"]["challenge"].as_str().unwrap_or("");
    let token = json_captcha["data"]["token"].as_str().unwrap_or("");
    let referer = "https://passport.bilibili.com/x/passport-login/captcha";

    let captcha_result_str = match captcha(
        custom_config.clone(),
        gt,
        challenge,
        referer,
        33,
        local_captcha,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => return Err(format!("Captcha recognition failed: {}", e)),
    };
    let captcha_result: serde_json::Value =
        serde_json::from_str(&captcha_result_str).map_err(|e| e.to_string())?;

    let validate = captcha_result["validate"].as_str().unwrap_or("");
    let seccode = captcha_result["seccode"].as_str().unwrap_or("");

    let encrypted_password = {
        let public_key = RsaPublicKey::from_public_key_pem(&public_key_str)
            .map_err(|e| format!("Failed to parse RSA public key: {}", e))?;

        let data_to_encrypt = format!("{}{}", salt, password);

        let mut rng = OsRng;
        let encrypted_bytes = public_key
            .encrypt(&mut rng, Pkcs1v15Encrypt, data_to_encrypt.as_bytes())
            .map_err(|e| format!("Failed to encrypt password: {}", e))?;

        STANDARD.encode(&encrypted_bytes)
    };

    let json_data = json!({
        "username": username,
        "password": encrypted_password,
        "validate": validate,
        "token": token,
        "seccode": seccode,
        "challenge": challenge,
        "go_url": "https://www.bilibili.com/",
        "keep": 0,
        "source": "main_web",
    });
    log::debug!("Login request data: {:?}", json_data);

    let login_response = request_post(
        client,
        "https://passport.bilibili.com/x/passport-login/web/login",
        None,
        Some(&json_data),
    )
    .await
    .map_err(|e| e.to_string())?;

    let mut all_cookies = Vec::new();
    let cookie_headers = login_response
        .headers()
        .get_all(reqwest::header::SET_COOKIE);

    for value in cookie_headers {
        if let Ok(cookie_str) = value.to_str() {
            if let Some(end_pos) = cookie_str.find(';') {
                all_cookies.push(cookie_str[0..end_pos].to_string());
            } else {
                all_cookies.push(cookie_str.to_string());
            }
        }
    }
    log::info!("Cookies received: {:?}", all_cookies);

    let json_response = login_response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Failed to parse login response JSON: {}", e))?;
    log::debug!(": {:?}", json_response);

    if json_response["code"].as_i64() == Some(0) {
        log::info!("Password login successful!");
        Ok(all_cookies.to_vec().join(";"))
    } else {
        Err(format!(
            "Password login failed: {}",
            json_response["message"].as_str().unwrap_or("unknown error")
        ))
    }
}


async fn get_pubkey_and_salt(client: &Client) -> Result<(String, String), String> {
    let response = request_get(
        client,
        "https://passport.bilibili.com/x/passport-login/web/key",
        None,
    )
    .await
    .map_err(|e| format!("Failed to get public key and salt: {}", e.to_string()))?;

    let json = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Failed to parse public key and salt response: {}", e.to_string()))?;

    log::debug!("Public key and salt response: {:?}", json);

    if json["code"].as_i64() == Some(0) {
        let hash = json["data"]["hash"].as_str().ok_or("hash not found")?.to_string();
        let key = json["data"]["key"].as_str().ok_or("key not found")?.to_string();
        Ok((hash, key))
    } else {
        Err(format!(
            "Failed to get public key and salt: {}",
            json["message"].as_str().unwrap_or("unknown error")
        ))
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Country {
    pub name: String,
    pub cid: i32,
}

pub async fn get_country_list(client: &Client) -> Result<Vec<Country>, String> {
    let response = request_get(
        client,
        "https://passport.bilibili.com/web/generic/country/list",
        None,
    )
    .await
    .map_err(|e| e.to_string())?;

    let json = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;

    if json["code"].as_i64() == Some(0) {
        let mut countries = Vec::new();
        
        let process_list = |list: &Vec<serde_json::Value>, countries: &mut Vec<Country>| {
            for item in list {
                let name = item["cname"].as_str().unwrap_or("").to_string();
                let cid_str = item["country_id"].as_str().unwrap_or("86");
                let cid = cid_str.parse::<i32>().unwrap_or(86);
                countries.push(Country { name, cid });
            }
        };

        if let Some(common_list) = json["data"]["common"].as_array() {
            process_list(common_list, &mut countries);
        }
        if let Some(others_list) = json["data"]["others"].as_array() {
            process_list(others_list, &mut countries);
        }
        Ok(countries)
    } else {
        Err(json["message"].as_str().unwrap_or("获取地区列表失败").to_string())
    }
}

pub async fn send_loginsms(
    phone: &str,
    cid: i32,
    client: &Client,
    custom_config: CustomConfig,
    local_captcha: LocalCaptcha,
) -> Result<String, String> {
    let response = request_get(client, "https://www.bilibili.com/", None)
        .await
        .map_err(|e| e.to_string())?;

    log::debug!("{:?}", response.cookies().collect::<Vec<_>>());

    // 发送请求
    let response = request_get(
        client,
        "https://passport.bilibili.com/x/passport-login/captcha",
        None,
    )
    .await
    .map_err(|e| e.to_string())?;
    log::info!("获取验证码: {:?}", response);

    let json = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    let gt = json["data"]["geetest"]["gt"].as_str().unwrap_or("");
    let challenge = json["data"]["geetest"]["challenge"].as_str().unwrap_or("");
    let token = json["data"]["token"].as_str().unwrap_or("");
    let referer = "https://passport.bilibili.com/x/passport-login/captcha";
    match captcha(
        custom_config.clone(),
        gt,
        challenge,
        referer,
        33,
        local_captcha,
    )
    .await
    {
        Ok(result_str) => {
            log::info!("验证码识别成功: {}", result_str);
            let result: serde_json::Value =
                serde_json::from_str(&result_str).map_err(|e| e.to_string())?;

            let json_data = json!({
            "cid": cid,
            "tel": phone,
            "token": token,
            "source":"main_mini",
            "challenge": result["challenge"],
            "validate": result["validate"],
            "seccode": result["seccode"],
            });
            log::debug!("验证码数据: {:?}", json_data);
            let send_sms = request_post(
                client,
                "https://passport.bilibili.com/x/passport-login/web/sms/send",
                None,
                Some(&json_data),
            )
            .await
            .map_err(|e| e.to_string())?;

            let json_response = send_sms
                .json::<serde_json::Value>()
                .await
                .map_err(|e| e.to_string())?;
            log::debug!("验证码发送响应: {:?}", json_response);
            if json_response["code"].as_i64() == Some(0) {
                let captcha_key = json_response["data"]["captcha_key"].as_str().unwrap_or("");
                log::info!("验证码发送成功");
                log::debug!("captcha_key: {:?}", captcha_key);
                Ok(captcha_key.to_string())
            } else {
                log::error!(
                    "验证码发送失败: {}",
                    json_response["message"].as_str().unwrap_or("未知错误")
                );
                Err("验证码发送失败".to_string())
            }
        }
        Err(e) => {
            log::error!("验证码识别失败: {}", e);
            Err("验证码识别失败".to_string())
        }
    }
}

pub async fn sms_login(
    phone: &str,
    cid: i32,
    sms_code: &str,
    captcha_key: &str,
    client: &Client,
) -> Result<String, String> {
    let data = serde_json::json!({
        "cid": cid,
        "tel": phone,
        "code": sms_code,
        "source":"main_mini",
        "captcha_key":captcha_key,
    });
    log::debug!("短信登录数据: {:?}", data);
    let login_response = request_post(
        client,
        "https://passport.bilibili.com/x/passport-login/web/login/sms",
        None,
        Some(&data),
    )
    .await
    .map_err(|e| e.to_string())?;
    let mut all_cookies = Vec::new();
    let cookie_headers = login_response
        .headers()
        .get_all(reqwest::header::SET_COOKIE);
    log::debug!("headers返回：{:?}", cookie_headers);
    for value in cookie_headers {
        if let Ok(cookie_str) = value.to_str() {
            if let Some(end_pos) = cookie_str.find(';') {
                all_cookies.push(cookie_str[0..end_pos].to_string());
            } else {
                all_cookies.push(cookie_str.to_string());
            }
        }
    }
    log::info!("获取cookie: {:?}", all_cookies);
    let json_response = login_response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("解析JSON失败: {}", e))?;
    log::debug!("登录接口响应：{:?}", json_response);
    if json_response["code"].as_i64() == Some(0) {
        log::info!("短信登录成功！");
        log::info!("登录cookie：{:?}", all_cookies);
        return Ok(all_cookies.to_vec().join(";"));
    }
    Err("短信登录失败".to_string())
}

pub fn cookie_login(cookie: &str, client: &Client, ua: &str) -> Result<Account, String> {
    match add_account(cookie, client, ua) {
        Ok(account) => {
            log::info!("ck登录成功");
            Ok(account)
        }
        Err(e) => {
            log::error!("ck登录失败: {}", e);
            Err(e)
        }
    }
}

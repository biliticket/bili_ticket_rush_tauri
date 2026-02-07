use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use rand::{Rng, distributions::Alphanumeric, thread_rng};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn create_client(user_agent: String) -> Client {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_str(&user_agent).unwrap_or_else(|_| {
            header::HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
        }),
    );

    Client::builder()
        .default_headers(headers)
        .cookie_store(true)
        .build()
        .unwrap_or_default()
}

pub fn default_user_agent() -> String {
    let random_value = generate_random_string(8);
    format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36 Edg/134.0.0.0 {}",
        random_value
    )
}

pub fn generate_random_string(length: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(|c| c as char)
        .collect()
}

pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Serialize, Deserialize)]
struct PolicyPayload {
    policy: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct PermissionsPayload {
    permissions: Value,
}

pub fn decode_policy(token: &str, public_key: &str) -> Result<Value, String> {
    let decoding_key = DecodingKey::from_rsa_pem(public_key.as_bytes())
        .map_err(|e| format!("invalid public key: {}", e))?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    validation.required_spec_claims.clear();

    decode::<PolicyPayload>(token, &decoding_key, &validation)
        .map(|data| data.claims.policy)
        .map_err(|e| format!("decode policy failed: {}", e))
}

pub fn decode_permissions(token: &str, public_key: &str) -> Result<Value, String> {
    let decoding_key = DecodingKey::from_rsa_pem(public_key.as_bytes())
        .map_err(|e| format!("invalid public key: {}", e))?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    validation.required_spec_claims.clear();

    decode::<PermissionsPayload>(token, &decoding_key, &validation)
        .map(|data| data.claims.permissions)
        .map_err(|e| format!("decode permissions failed: {}", e))
}

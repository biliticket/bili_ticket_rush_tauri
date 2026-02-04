use reqwest::Client;
use serde_json::Value;
use std::sync::{Arc, Mutex};

use backend::taskmanager::TaskManagerImpl;
use common::account::Account;
use common::config::{BtrConfig as Config, CustomConfig, PushConfig};
use common::login::LoginInput;
use common::machine_id;
use common::taskmanager::TaskManager;
use common::ticket::{BilibiliTicket, TicketInfo};
use common::show_orderlist::OrderResponse;
use common::ticket::{BuyerInfo, NoBindBuyerInfo};
use common::captcha::LocalCaptcha;

use crate::utils::{create_client, default_user_agent};

pub const APP_NAME: &str = "BTR";
pub const APP_VERSION: &str = "7.0.0";

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<Mutex<AppStateInner>>,
}

#[derive(Clone)]
pub struct AppStateInner {
    pub app: String,
    pub version: String,
    pub policy: Option<Value>,
    pub public_key: String,
    pub machine_id: String,

    pub selected_tab: usize,
    pub is_loading: bool,
    pub running_status: String,

    pub logs: Vec<String>,
    pub show_log_window: bool,

    pub show_login_window: bool,
    pub login_method: String,
    pub client: Client,
    pub default_ua: String,
    pub login_qrcode_url: Option<String>,
    pub qrcode_polling_task_id: Option<String>,
    pub login_input: LoginInput,
    pub pending_sms_task_id: Option<String>,
    pub sms_captcha_key: String,
    pub cookie_login: Option<String>,

    pub accounts: Vec<Account>,
    pub delete_account: Option<String>,
    pub account_switch: Option<AccountSwitch>,

    pub task_manager: Arc<Mutex<Box<dyn TaskManager>>>,

    pub config: Config,
    pub push_config: PushConfig,
    pub custom_config: CustomConfig,

    pub ticket_id: String,
    pub status_delay: usize,
    pub grab_mode: u8,
    pub selected_account_uid: Option<i64>,
    pub bilibiliticket_list: Vec<BilibiliTicket>,
    pub ticket_info: Option<TicketInfo>,
    pub show_screen_info: Option<i64>,
    pub selected_screen_index: Option<usize>,
    pub selected_screen_id: Option<i64>,
    pub selected_ticket_id: Option<i64>,
    pub ticket_info_last_request_time: Option<std::time::Instant>,
    pub confirm_ticket_info: Option<String>,
    pub selected_buyer_list: Option<Vec<BuyerInfo>>,
    pub selected_no_bind_buyer_info: Option<NoBindBuyerInfo>,
    pub buyer_type: u8,

    pub show_add_buyer_window: Option<String>,
    pub show_orderlist_window: Option<String>,
    pub total_order_data: Option<OrderData>,
    pub orderlist_need_reload: bool,
    pub orderlist_last_request_time: Option<std::time::Instant>,
    pub orderlist_requesting: bool,

    pub show_qr_windows: Option<String>,

    pub announce1: Option<String>,
    pub announce2: Option<String>,
    pub announce3: Option<String>,
    pub announce4: Option<String>,

    pub skip_words: Option<Vec<String>>,
    pub skip_words_input: String,
    
    // Captcha
    pub local_captcha: LocalCaptcha,
}

#[derive(Clone)]
pub struct OrderData {
    pub account_id: String,
    pub data: Option<OrderResponse>,
}

#[derive(Clone)]
pub struct AccountSwitch {
    pub uid: String,
    pub switch: bool,
}

impl AppState {
    pub fn new() -> Self {
        let config = Config::load_config().unwrap_or_else(|e| {
            log::error!("加载配置失败，将使用默认配置: {}", e);
            Config::default()
        });

        let mut state = AppStateInner {
            app: APP_NAME.to_string(),
            version: APP_VERSION.to_string(),
            policy: None,
            public_key: "-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKcaQEApTAS0RElXIs4Kr0bO4n8\nJB+eBFF/TwXUlvtOM9FNgHjK8m13EdwXaLy9zjGTSQr8tshSRr0dQ6iaCG19Zo2Y\nXfvJrwQLqdezMN+ayMKFy58/S9EGG3Np2eGgKHUPnCOAlRicqWvBdQ/cxzTDNCxa\nORMZdJRoBvya7JijLLIC3CoqmMc6Fxe5i8eIP0zwlyZ0L0C1PQ82BcWn58y7tlPY\nTCz12cWnuKwiQ9LSOfJ4odJJQK0k7rXxwBBsYxULRno0CJ3rKfApssW4cfITYVax\nFtdbu0IUsgEeXs3EzNw8yIYnsaoZlFwLS8SMVsiAFOy2y14lR9043PYAQHm1Cjaf\noQIDAQAB\n-----END PUBLIC KEY-----".to_string(),
            machine_id: machine_id::get_machine_id_ob(),
            selected_tab: 0,
            is_loading: false,
            running_status: "空闲".to_string(),
            logs: Vec::new(),
            show_log_window: false,
            show_login_window: false,
            login_method: "扫码登录".to_string(),
            client: Client::new(), // Will be replaced
            default_ua: default_user_agent(),
            login_qrcode_url: None,
            qrcode_polling_task_id: None,
            login_input: LoginInput::default(),
            pending_sms_task_id: None,
            sms_captcha_key: String::new(),
            cookie_login: None,
            accounts: config.accounts.clone(),
            delete_account: None,
            account_switch: None,
            task_manager: Arc::new(Mutex::new(Box::new(TaskManagerImpl::new()))),
            push_config: config.push_config.clone(),
            custom_config: config.custom_config.clone(),
            ticket_id: String::new(),
            status_delay: config.delay_time as usize,
            grab_mode: config.grab_mode,
            selected_account_uid: None,
            bilibiliticket_list: Vec::new(),
            ticket_info: None,
            show_screen_info: None,
            selected_screen_index: None,
            selected_screen_id: None,
            selected_ticket_id: None,
            ticket_info_last_request_time: None,
            confirm_ticket_info: None,
            selected_buyer_list: None,
            selected_no_bind_buyer_info: None,
            buyer_type: 1,

            show_add_buyer_window: None,
            show_orderlist_window: None,
            total_order_data: None,
            orderlist_need_reload: false,
            orderlist_last_request_time: None,
            orderlist_requesting: false,
            show_qr_windows: None,
            announce1: None,
            announce2: None,
            announce3: None,
            announce4: None,
            skip_words: config.skip_words.clone(),
            skip_words_input: String::new(),
            config,
            local_captcha: LocalCaptcha::new(),
        };

        if state.custom_config.open_custom_ua && !state.custom_config.custom_ua.is_empty() {
            state.default_ua = state.custom_config.custom_ua.clone();
        }

        state.client = create_client(state.default_ua.clone());

        for account in &mut state.accounts {
            account.ensure_client();
        }

        Self {
            inner: Arc::new(Mutex::new(state)),
        }
    }
}

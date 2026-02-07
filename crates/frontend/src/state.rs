use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};

use backend::dungeon::DungeonService;
use backend::taskmanager::TaskManagerImpl;
use common::account::Account;
use common::captcha::LocalCaptcha;
use common::config::{BtrConfig as Config, CustomConfig, PushConfig};
use common::login::LoginInput;
use common::machine_id;
use common::show_orderlist::OrderResponse;
use common::taskmanager::TaskManager;
use common::ticket::{BilibiliTicket, TicketInfo};
use common::ticket::{BuyerInfo, NoBindBuyerInfo};

use crate::utils::{create_client, default_user_agent};
use tokio::sync::mpsc;

pub const APP_NAME: &str = "BTR";
pub const APP_VERSION: &str = "7.0.0";

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Mutex<ConfigState>>,
    pub ticket: Arc<Mutex<TicketState>>,
    pub auth: Arc<Mutex<AuthState>>,
    pub runtime: Arc<Mutex<RuntimeState>>,
    pub ui: Arc<Mutex<UiState>>,
}

pub struct ConfigState {
    pub config: Config,
    pub push_config: PushConfig,
    pub custom_config: CustomConfig,
    pub accounts: Vec<Account>,
    pub skip_words: Option<Vec<String>>,
    pub skip_words_input: String,
}

pub struct TicketState {
    pub ticket_id: String,
    pub grab_mode: u8,
    pub status_delay: usize,

    pub bilibiliticket_list: Vec<BilibiliTicket>,
    pub ticket_info: Option<TicketInfo>,
    pub show_screen_info: Option<i64>,
    pub selected_screen_id: Option<i64>,
    pub selected_ticket_id: Option<i64>,
    pub ticket_info_last_request_time: Option<std::time::Instant>,
    pub confirm_ticket_info: Option<String>,
    pub selected_buyer_list: Option<Vec<BuyerInfo>>,
    pub selected_no_bind_buyer_info: Option<NoBindBuyerInfo>,
    pub buyer_type: u8,
}

pub struct AuthState {
    pub login_method: String,
    pub login_input: LoginInput,
    pub login_qrcode_url: Option<String>,
    pub qrcode_polling_task_id: Option<String>,
    pub pending_sms_task_id: Option<String>,
    pub sms_captcha_key: String,
    pub cookie_login: Option<String>,
    pub client: Client,
    pub default_ua: String,
}

pub struct RuntimeState {
    pub app: String,
    pub version: String,
    pub policy: Option<Value>,
    pub public_key: String,
    pub machine_id: String,
    pub permissions: Option<Value>,

    pub running_status: String,
    pub is_loading: bool,
    pub logs: Vec<String>,

    pub task_manager: Box<dyn TaskManager + Send>,
    pub local_captcha: LocalCaptcha,
    pub result_receiver: Option<mpsc::Receiver<common::taskmanager::TaskResult>>,
    pub result_sender: Option<mpsc::Sender<common::taskmanager::TaskResult>>,
    pub dungeon_service: Option<std::sync::Arc<backend::dungeon::DungeonService>>,
}

pub struct UiState {
    pub selected_tab: usize,
    pub show_log_window: bool,
    pub show_login_window: bool,
    pub show_add_buyer_window: Option<String>,
    pub show_orderlist_window: Option<String>,
    pub show_qr_windows: Option<String>,

    pub delete_account: Option<String>,
    pub account_switch: Option<AccountSwitch>,
    pub selected_account_uid: Option<i64>,

    pub total_order_data: Option<OrderData>,
    pub orderlist_need_reload: bool,
    pub orderlist_last_request_time: Option<std::time::Instant>,
    pub orderlist_requesting: bool,

    pub announce1: Option<String>,
    pub announce2: Option<String>,
    pub announce3: Option<String>,
    pub announce4: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OrderData {
    pub account_id: String,
    pub data: Option<OrderResponse>,
}

#[derive(Clone, Serialize, Deserialize)]
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

        let mut auth_state = AuthState {
            login_method: "扫码登录".to_string(),
            client: Client::new(),
            default_ua: default_user_agent(),
            login_qrcode_url: None,
            qrcode_polling_task_id: None,
            login_input: LoginInput::default(),
            pending_sms_task_id: None,
            sms_captcha_key: String::new(),
            cookie_login: None,
        };

        if config.custom_config.open_custom_ua && !config.custom_config.custom_ua.is_empty() {
            auth_state.default_ua = config.custom_config.custom_ua.clone();
        }
        auth_state.client = create_client(auth_state.default_ua.clone());

        let mut config_state = ConfigState {
            accounts: config.accounts.clone(),
            push_config: config.push_config.clone(),
            custom_config: config.custom_config.clone(),
            skip_words: config.skip_words.clone(),
            skip_words_input: String::new(),
            config,
        };

        for account in &mut config_state.accounts {
            account.ensure_client();
        }

        let ticket_state = TicketState {
            ticket_id: String::new(),
            status_delay: config_state.config.delay_time as usize,
            grab_mode: config_state.config.grab_mode,
            bilibiliticket_list: Vec::new(),
            ticket_info: None,
            show_screen_info: None,
            selected_screen_id: None,
            selected_ticket_id: None,
            ticket_info_last_request_time: None,
            confirm_ticket_info: None,
            selected_buyer_list: None,
            selected_no_bind_buyer_info: None,
            buyer_type: 1,
        };

        let (tx, rx) = mpsc::channel(100);
        let mut task_manager = Box::new(TaskManagerImpl::new());
        task_manager.set_result_sender(tx.clone());

        let dungeon_service = Arc::new(DungeonService::new());
        {
            let mut ds_lock = task_manager.dungeon_service.blocking_lock();
            *ds_lock = Some(dungeon_service.clone());
        }

        let runtime_state = RuntimeState {
            app: APP_NAME.to_string(),
            version: APP_VERSION.to_string(),
            policy: None,
            public_key: "-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKcaQEApTAS0RElXIs4Kr0bO4n8\nJB+eBFF/TwXUlvtOM9FNgHjK8m13EdwXaLy9zjGTSQr8tshSRr0dQ6iaCG19Zo2Y\nXfvJrwQLqdezMN+ayMKFy58/S9EGG3Np2eGgKHUPnCOAlRicqWvBdQ/cxzTDNCxa\nORMZdJRoBvya7JijLLIC3CoqmMc6Fxe5i8eIP0zwlyZ0L0C1PQ82BcWn58y7tlPY\nTCz12cWnuKwiQ9LSOfJ4odJJQK0k7rXxwBBsYxULRno0CJ3rKfApssW4cfITYVax\nFtdbu0IUsgEeXs3EzNw8yIYnsaoZlFwLS8SMVsiAFOy2y14lR9043PYAQHm1Cjaf\noQIDAQAB\n-----END PUBLIC KEY-----".to_string(),
            machine_id: machine_id::get_machine_id_ob(),
            permissions: None,
            running_status: "空闲".to_string(),
            is_loading: false,
            logs: Vec::new(),
            task_manager,
            local_captcha: LocalCaptcha::new(),
            result_receiver: Some(rx),
            result_sender: Some(tx),
            dungeon_service: Some(dungeon_service),
        };

        let ui_state = UiState {
            selected_tab: 0,
            show_log_window: false,
            show_login_window: false,
            show_add_buyer_window: None,
            show_orderlist_window: None,
            show_qr_windows: None,
            delete_account: None,
            account_switch: None,
            selected_account_uid: None,
            total_order_data: None,
            orderlist_need_reload: false,
            orderlist_last_request_time: None,
            orderlist_requesting: false,
            announce1: None,
            announce2: None,
            announce3: None,
            announce4: None,
        };

        Self {
            config: Arc::new(Mutex::new(config_state)),
            ticket: Arc::new(Mutex::new(ticket_state)),
            auth: Arc::new(Mutex::new(auth_state)),
            runtime: Arc::new(Mutex::new(runtime_state)),
            ui: Arc::new(Mutex::new(ui_state)),
        }
    }
}

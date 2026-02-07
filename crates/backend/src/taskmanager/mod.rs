pub mod grab_ticket_handler;
pub mod login_handler;
pub mod order_handler;
pub mod push_handler;
pub mod ticket_handler;

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use self::{
    grab_ticket_handler::handle_grab_ticket_request,
    login_handler::{
        handle_login_sms_request, handle_qrcode_login_request, handle_submit_login_sms_request,
    },
    order_handler::handle_get_all_order_request,
    push_handler::handle_push_request,
    ticket_handler::{handle_get_buyer_info_request, handle_get_ticket_info_request},
};
use common::taskmanager::*;
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

pub struct TaskManagerImpl {
    task_sender: mpsc::Sender<TaskMessage>,
    result_receiver: Option<mpsc::Receiver<TaskResult>>,
    running_tasks: HashMap<String, Task>,
    runtime: Arc<Runtime>,
    _worker_thread: Option<thread::JoinHandle<()>>,
    pub dungeon_service: Arc<Mutex<Option<Arc<crate::dungeon::DungeonService>>>>,
}

enum TaskMessage {
    SubmitTask((String, TaskRequest)),
    CancelTask(String),
    Shutdown,
}

impl TaskManager for TaskManagerImpl {
    fn new() -> Self {
        let (task_tx, mut task_rx) = mpsc::channel(100);
        let (result_tx, result_rx) = mpsc::channel(100);

        let runtime = Arc::new(Runtime::new().unwrap());
        let rt = runtime.clone();

        let dungeon_service = Arc::new(Mutex::new(None));
        let ds_clone = dungeon_service.clone();

        let worker = thread::spawn(move || {
            let mut task_handles: HashMap<String, JoinHandle<()>> = HashMap::new();

            rt.block_on(async {
                while let Some(msg) = task_rx.recv().await {
                    match msg {
                        TaskMessage::SubmitTask((task_id, request)) => {
                            let result_tx = result_tx.clone();
                            let ds_for_task = ds_clone.clone();

                            let handle = match request {
                                TaskRequest::QrCodeLoginRequest(qrcode_req) => {
                                    tokio::spawn(handle_qrcode_login_request(qrcode_req, result_tx))
                                }
                                TaskRequest::LoginSmsRequest(login_sms_req) => {
                                    tokio::spawn(handle_login_sms_request(login_sms_req, result_tx))
                                }
                                TaskRequest::PushRequest(push_req) => {
                                    let ds_guard = ds_for_task.lock().await;
                                    let ds_opt: Option<Arc<crate::dungeon::DungeonService>> =
                                        ds_guard.clone();
                                    tokio::spawn(handle_push_request(push_req, result_tx, ds_opt))
                                }
                                TaskRequest::SubmitLoginSmsRequest(login_sms_req) => tokio::spawn(
                                    handle_submit_login_sms_request(login_sms_req, result_tx),
                                ),
                                TaskRequest::GetAllorderRequest(get_order_req) => tokio::spawn(
                                    handle_get_all_order_request(get_order_req, result_tx),
                                ),
                                TaskRequest::GetTicketInfoRequest(get_ticketinfo_req) => {
                                    tokio::spawn(handle_get_ticket_info_request(
                                        get_ticketinfo_req,
                                        result_tx,
                                    ))
                                }
                                TaskRequest::GetBuyerInfoRequest(get_buyerinfo_req) => {
                                    tokio::spawn(handle_get_buyer_info_request(
                                        get_buyerinfo_req,
                                        result_tx,
                                    ))
                                }
                                TaskRequest::GrabTicketRequest(grab_ticket_req) => tokio::spawn(
                                    handle_grab_ticket_request(grab_ticket_req, result_tx),
                                ),
                            };
                            task_handles.insert(task_id, handle);
                        }
                        TaskMessage::CancelTask(task_id) => {
                            if let Some(handle) = task_handles.remove(&task_id) {
                                log::info!("正在取消任务: {}", &task_id);
                                handle.abort();
                            }
                        }
                        TaskMessage::Shutdown => break,
                    }
                }
            });
        });

        Self {
            task_sender: task_tx,
            result_receiver: Some(result_rx),
            running_tasks: HashMap::new(),
            runtime,
            _worker_thread: Some(worker),
            dungeon_service,
        }
    }

    fn submit_task(&mut self, request: TaskRequest) -> Result<String, String> {
        let task_id = match &request {
            TaskRequest::GetBuyerInfoRequest(req) => {
                if !req.task_id.is_empty() {
                    req.task_id.clone()
                } else {
                    uuid::Uuid::new_v4().to_string()
                }
            }
            TaskRequest::GetTicketInfoRequest(req) => {
                if !req.task_id.is_empty() {
                    req.task_id.clone()
                } else {
                    uuid::Uuid::new_v4().to_string()
                }
            }
            TaskRequest::GrabTicketRequest(req) => {
                if !req.task_id.is_empty() {
                    req.task_id.clone()
                } else {
                    uuid::Uuid::new_v4().to_string()
                }
            }
            _ => uuid::Uuid::new_v4().to_string(),
        };

        match &request {
            TaskRequest::QrCodeLoginRequest(qrcode_req) => {
                log::info!("提交二维码登录任务 ID: {}", task_id);
                let task = QrCodeLoginTask {
                    task_id: task_id.clone(),
                    qrcode_key: qrcode_req.qrcode_key.clone(),
                    qrcode_url: qrcode_req.qrcode_url.clone(),
                    status: TaskStatus::Pending,
                    start_time: Some(std::time::Instant::now()),
                };
                self.running_tasks
                    .insert(task_id.clone(), Task::QrCodeLoginTask(task));
            }
            TaskRequest::LoginSmsRequest(login_sms_req) => {
                log::info!(
                    "提交短信验证码任务 ID: {}, 手机号: {}",
                    task_id,
                    login_sms_req.phone
                );
                let task = LoginSmsRequestTask {
                    task_id: task_id.clone(),
                    phone: login_sms_req.phone.clone(),
                    status: TaskStatus::Pending,
                    start_time: Some(std::time::Instant::now()),
                };
                self.running_tasks
                    .insert(task_id.clone(), Task::LoginSmsRequestTask(task));
            }
            TaskRequest::PushRequest(push_req) => {
                log::info!("提交推送任务 ID: {}", task_id);
                let task = PushTask {
                    task_id: task_id.clone(),
                    push_type: push_req.push_type.clone(),
                    title: push_req.title.clone(),
                    message: push_req.message.clone(),
                    status: TaskStatus::Pending,
                    start_time: Some(std::time::Instant::now()),
                };
                self.running_tasks
                    .insert(task_id.clone(), Task::PushTask(task));
            }
            TaskRequest::SubmitLoginSmsRequest(login_sms_req) => {
                log::info!(
                    "提交短信验证码登录任务 ID: {}, 手机号: {}",
                    task_id,
                    login_sms_req.phone
                );
                let task = SubmitLoginSmsRequestTask {
                    task_id: task_id.clone(),
                    phone: login_sms_req.phone.clone(),
                    code: login_sms_req.code.clone(),
                    captcha_key: login_sms_req.captcha_key.clone(),
                    status: TaskStatus::Pending,
                    start_time: Some(std::time::Instant::now()),
                };
                self.running_tasks
                    .insert(task_id.clone(), Task::SubmitLoginSmsRequestTask(task));
            }
            TaskRequest::GetAllorderRequest(get_order_req) => {
                log::info!("提交获取全部订单任务 ID: {}", task_id);
                let task = GetAllorderRequest {
                    task_id: task_id.clone(),
                    cookie_manager: get_order_req.cookie_manager.clone(),
                    status: TaskStatus::Pending,
                    cookies: get_order_req.cookies.clone(),
                    account_id: get_order_req.account_id.clone(),
                    start_time: Some(std::time::Instant::now()),
                };
                self.running_tasks
                    .insert(task_id.clone(), Task::GetAllorderRequestTask(task));
            }
            TaskRequest::GetTicketInfoRequest(get_ticketinfo_req) => {
                log::info!("提交获取票务信息任务 ID: {}", task_id);
                let task = GetTicketInfoTask {
                    task_id: task_id.clone(),
                    project_id: get_ticketinfo_req.project_id.clone(),
                    status: TaskStatus::Running,
                    start_time: Some(std::time::Instant::now()),
                    cookie_manager: get_ticketinfo_req.cookie_manager.clone(),
                };
                self.running_tasks
                    .insert(task_id.clone(), Task::GetTicketInfoTask(task));
            }
            TaskRequest::GetBuyerInfoRequest(get_buyerinfo_req) => {
                log::info!("提交获取购票人信息任务 ID: {}", task_id);
                let task = GetBuyerInfoTask {
                    uid: get_buyerinfo_req.uid.clone(),
                    task_id: task_id.clone(),
                    cookie_manager: get_buyerinfo_req.cookie_manager.clone(),
                    status: TaskStatus::Pending,
                    start_time: Some(std::time::Instant::now()),
                };
                self.running_tasks
                    .insert(task_id.clone(), Task::GetBuyerInfoTask(task));
            }
            TaskRequest::GrabTicketRequest(grab_ticket_req) => {
                log::info!("提交抢票任务 ID: {}", task_id);
                let task = GrabTicketTask {
                    task_id: task_id.clone(),
                    biliticket: grab_ticket_req.biliticket.clone(),
                    status: TaskStatus::Pending,
                    client: grab_ticket_req.cookie_manager.client.clone(),
                    start_time: Some(std::time::Instant::now()),
                };
                self.running_tasks
                    .insert(task_id.clone(), Task::GrabTicketTask(task));
            }
        }

        if let Err(e) = self
            .task_sender
            .blocking_send(TaskMessage::SubmitTask((task_id.clone(), request)))
        {
            return Err(format!("无法提交任务: {}", e));
        }

        Ok(task_id)
    }

    fn set_result_sender(&mut self, sender: mpsc::Sender<TaskResult>) {
        if let Some(mut rx) = self.result_receiver.take() {
            let rt = self.runtime.clone();
            rt.spawn(async move {
                while let Some(result) = rx.recv().await {
                    if let Err(e) = sender.send(result).await {
                        log::error!("Failed to forward task result: {}", e);
                        break;
                    }
                }
            });
        }
    }

    fn get_results(&mut self) -> Vec<TaskResult> {
        let mut results = Vec::new();
        if let Some(rx) = &mut self.result_receiver {
            while let Ok(result) = rx.try_recv() {
                results.push(result);
            }
        }
        results
    }

    fn cancel_task(&mut self, task_id: &str) -> Result<(), String> {
        if let Some(task) = self.running_tasks.get_mut(task_id) {
            if let Err(e) = self
                .task_sender
                .blocking_send(TaskMessage::CancelTask(task_id.to_owned()))
            {
                return Err(format!("无法取消任务: {}", e));
            }
            let new_status = TaskStatus::Cancelled;
            match task {
                Task::QrCodeLoginTask(t) => t.status = new_status,
                Task::LoginSmsRequestTask(t) => t.status = new_status,
                Task::PushTask(t) => t.status = new_status,
                Task::SubmitLoginSmsRequestTask(t) => t.status = new_status,
                Task::GetAllorderRequestTask(t) => t.status = new_status,
                Task::GetTicketInfoTask(t) => t.status = new_status,
                Task::GetBuyerInfoTask(t) => t.status = new_status,
                Task::GrabTicketTask(t) => t.status = new_status,
            }
            Ok(())
        } else {
            Err("任务不存在".to_string())
        }
    }

    fn get_task_status(&self, task_id: &str) -> Option<TaskStatus> {
        self.running_tasks.get(task_id).map(|task| match task {
            Task::QrCodeLoginTask(t) => t.status.clone(),
            Task::LoginSmsRequestTask(t) => t.status.clone(),
            Task::PushTask(t) => t.status.clone(),
            Task::SubmitLoginSmsRequestTask(t) => t.status.clone(),
            Task::GetAllorderRequestTask(t) => t.status.clone(),
            Task::GetTicketInfoTask(t) => t.status.clone(),
            Task::GetBuyerInfoTask(t) => t.status.clone(),
            Task::GrabTicketTask(t) => t.status.clone(),
        })
    }

    fn shutdown(&mut self) {
        let _ = self.task_sender.blocking_send(TaskMessage::Shutdown);
        if let Some(handle) = self._worker_thread.take() {
            let _ = handle.join();
        }
    }
}

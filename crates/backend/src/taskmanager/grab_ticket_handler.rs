use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::api::{
    check_fake_ticket, confirm_ticket_order, create_order, get_countdown, get_ticket_token,
};

use common::{
    captcha::handle_risk_verification,
    config::CustomConfig,
    cookie_manager::CookieManager,
    gen_cp::CTokenGenerator,
    taskmanager::{GrabTicketRequest, GrabTicketResult, TaskResult},
    ticket::{BuyerInfo, CheckFakeResult, ConfirmTicketResult},
};
use rand::{Rng, SeedableRng, rngs::StdRng};
use serde_json::json;
use tokio::sync::mpsc;

use crate::api::get_project;

pub async fn handle_grab_ticket_request(
    grab_ticket_req: GrabTicketRequest,
    result_tx: mpsc::Sender<TaskResult>,
) {
    let project_id = grab_ticket_req.project_id.clone();
    let screen_id = grab_ticket_req.screen_id.clone();
    let ticket_id = grab_ticket_req.ticket_id.clone();
    let buyer_info = grab_ticket_req.buyer_info.clone();
    let cookie_manager = grab_ticket_req.cookie_manager.clone();
    let task_id = grab_ticket_req.task_id.clone();
    let uid = grab_ticket_req.uid.clone();
    let mode = grab_ticket_req.grab_mode.clone();
    let custon_config = grab_ticket_req.biliticket.config.clone();
    let csrf = grab_ticket_req.biliticket.account.csrf.clone();
    let local_captcha = grab_ticket_req.local_captcha.clone();
    let count = grab_ticket_req.count.clone();
    let project_info = grab_ticket_req.biliticket.project_info.clone();
    let skip_words = grab_ticket_req.skip_words.clone();
    let mut rng = StdRng::from_entropy();
    let is_hot = grab_ticket_req.is_hot.clone();
    let cpdd = if project_info.is_some() {
        Arc::new(Mutex::new(CTokenGenerator::new(
            project_info.clone().unwrap().sale_begin as i64,
            0,
            rng.gen_range(2000..10000),
        )))
    } else {
        Arc::new(Mutex::new(CTokenGenerator::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            0,
            rng.gen_range(2000..10000),
        )))
    };
    log::debug!("开始分析抢票任务：{}", task_id);

    match mode {
        0 => {
            timed_grab_ticket_mode(
                cookie_manager,
                cpdd,
                project_id,
                screen_id,
                ticket_id,
                count.try_into().unwrap(),
                is_hot,
                project_info,
                task_id,
                uid,
                &result_tx,
                grab_ticket_req,
                buyer_info,
                custon_config,
                csrf,
                Some(local_captcha),
            )
            .await;
        }
        1 => {
            direct_grab_ticket_mode(
                cookie_manager,
                cpdd,
                project_id,
                screen_id,
                ticket_id,
                count.try_into().unwrap(),
                is_hot,
                task_id,
                uid,
                &result_tx,
                grab_ticket_req,
                buyer_info,
                custon_config,
                csrf,
                Some(local_captcha),
            )
            .await;
        }
        2 => {
            leak_grab_ticket_mode(
                cookie_manager,
                cpdd,
                project_id,
                screen_id,
                ticket_id,
                count.try_into().unwrap(),
                is_hot,
                skip_words,
                rng,
                task_id,
                uid,
                &result_tx,
                grab_ticket_req,
                buyer_info,
                custon_config,
                csrf,
                Some(local_captcha),
            )
            .await;
        }
        _ => {
            log::error!("未知模式");
        }
    }
}

async fn timed_grab_ticket_mode(
    cookie_manager: Arc<CookieManager>,
    cpdd: Arc<Mutex<CTokenGenerator>>,
    project_id: String,
    screen_id: String,
    ticket_id: String,
    count: i16,
    is_hot: bool,
    project_info: Option<common::ticket::TicketInfo>,
    task_id: String,
    uid: i64,
    result_tx: &mpsc::Sender<TaskResult>,
    grab_ticket_req: GrabTicketRequest,
    buyer_info: Vec<BuyerInfo>,
    custon_config: CustomConfig,
    csrf: String,
    local_captcha: Option<common::captcha::LocalCaptcha>,
) {
    log::debug!("定时抢票模式");

    // 如果没有项目详情，尝试自动获取
    let project_info = if project_info.is_none() {
        log::info!("后台项目信息缺失，正在自动获取以确定开始时间...");
        match get_project(cookie_manager.clone(), &project_id).await {
            Ok(resp) => Some(resp.data),
            Err(e) => {
                log::error!("自动获取项目详情失败: {}", e);
                None
            }
        }
    } else {
        project_info
    };

    let mut countdown = match get_countdown(cookie_manager.clone(), project_info).await {
        Ok(countdown) => countdown,
        Err(e) => {
            log::error!("获取倒计时失败: {}", e);
            return;
        }
    };

    if countdown > 0.0 {
        log::info!("距离抢票时间还有{}秒", countdown);
        while countdown > 20.0 {
            countdown -= 15.0;
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
            log::info!("距离抢票时间还有{}秒", countdown);
        }
        while countdown > 1.3 {
            log::info!("距离抢票时间还有{}秒", countdown);
            countdown -= 1.0;
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
        tokio::time::sleep(tokio::time::Duration::from_secs_f32(0.8)).await;
    }

    log::info!("开始抢票！");
    let mut token_retry_count = 0;
    let max_token_retry = custon_config.max_token_retry as i8;

    //抢票主循环
    loop {
        let token_result = get_ticket_token(
            cookie_manager.clone(),
            cpdd.clone(),
            &project_id,
            &screen_id,
            &ticket_id,
            count,
            is_hot,
        )
        .await;
        match token_result {
            Ok((token, ptoken)) => {
                log::info!("获取抢票token成功！:{} ptoken:{}", token, ptoken);
                let mut confirm_retry_count = 0;
                let max_confirm_retry = custon_config.max_confirm_retry as i8;

                loop {
                    let (success, _) = handle_grab_ticket(
                        cookie_manager.clone(),
                        cpdd.clone(),
                        &project_id,
                        &token,
                        &ptoken,
                        is_hot,
                        &task_id,
                        uid,
                        &result_tx,
                        &grab_ticket_req,
                        &buyer_info,
                    )
                    .await;
                    if success {
                        log::info!("抢票流程结束，退出定时抢票模式");
                        return;
                    }

                    confirm_retry_count += 1;
                    if confirm_retry_count >= max_confirm_retry {
                        log::error!("确认订单失败，已达最大重试次数");
                        let task_result = TaskResult::GrabTicketResult(GrabTicketResult {
                            task_id: task_id.clone(),
                            uid,
                            success: false,
                            message: "确认订单失败，已达最大重试次数".to_string(),
                            order_id: None,
                            pay_token: None,
                            pay_result: None,
                            confirm_result: None,
                        });
                        let _ = result_tx.send(task_result).await;
                        return;
                    }
                }
            }
            Err(risk_param) => {
                if risk_param.code == -401 || risk_param.code == 401 {
                    log::warn!("需要验证码，开始处理验证码...");
                    match handle_risk_verification(
                        cookie_manager.clone(),
                        risk_param,
                        &custon_config,
                        &csrf,
                        local_captcha.clone().expect("REASON"),
                    )
                    .await
                    {
                        Ok(()) => log::info!("验证码处理成功！"),
                        Err(e) => {
                            log::error!("验证码处理失败: {}", e);
                            token_retry_count += 1;
                            if token_retry_count >= max_token_retry {
                                let task_result = TaskResult::GrabTicketResult(GrabTicketResult {
                                    task_id: task_id.clone(),
                                    uid,
                                    success: false,
                                    message: format!("验证码处理失败，已达最大重试次数: {}", e),
                                    order_id: None,
                                    pay_token: None,
                                    pay_result: None,
                                    confirm_result: None,
                                });
                                let _ = result_tx.send(task_result).await;
                                return;
                            }
                        }
                    }
                } else {
                    match risk_param.code {
                        100080 | 100082 => {
                            log::error!("抢票失败，场次/项目/日期选择有误，请重新提交任务");
                        }
                        100039 => {
                            log::error!("抢票失败，该场次已停售，请重新提交任务");
                        }
                        _ => {
                            log::error!("抢票失败，未知错误，请重新提交任务");
                        }
                    }
                    token_retry_count += 1;
                    if token_retry_count >= max_token_retry {
                        let task_result = TaskResult::GrabTicketResult(GrabTicketResult {
                            task_id: task_id.clone(),
                            uid,
                            success: false,
                            message: format!(
                                "获取token失败，错误代码: {}，错误信息：{}",
                                risk_param.code, risk_param.message
                            ),
                            order_id: None,
                            pay_token: None,
                            pay_result: None,
                            confirm_result: None,
                        });
                        let _ = result_tx.send(task_result).await;
                        return;
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}
async fn direct_grab_ticket_mode(
    cookie_manager: Arc<CookieManager>,
    cpdd: Arc<Mutex<CTokenGenerator>>,
    project_id: String,
    screen_id: String,
    ticket_id: String,
    count: i16,
    is_hot: bool,
    task_id: String,
    uid: i64,
    result_tx: &mpsc::Sender<TaskResult>,
    grab_ticket_req: GrabTicketRequest,
    buyer_info: Vec<BuyerInfo>,
    custon_config: CustomConfig,
    csrf: String,
    local_captcha: Option<common::captcha::LocalCaptcha>,
) {
    log::debug!("直接抢票模式");
    let mut token_retry_count = 0;
    let max_token_retry = custon_config.max_token_retry as i8;

    //抢票主循环
    loop {
        let token_result = get_ticket_token(
            cookie_manager.clone(),
            cpdd.clone(),
            &project_id,
            &screen_id,
            &ticket_id,
            count,
            is_hot,
        )
        .await;
        match token_result {
            Ok((token, ptoken)) => {
                log::info!("获取抢票token成功！:{} ptoken:{}", token, ptoken);
                let mut confirm_retry_count = 0;
                let max_confirm_retry = custon_config.max_confirm_retry as i8;

                loop {
                    let (success, _) = handle_grab_ticket(
                        cookie_manager.clone(),
                        cpdd.clone(),
                        &project_id,
                        &token,
                        &ptoken,
                        is_hot,
                        &task_id,
                        uid,
                        &result_tx,
                        &grab_ticket_req,
                        &buyer_info,
                    )
                    .await;
                    if success {
                        log::info!("抢票流程结束，退出直接抢票模式");
                        return;
                    }

                    confirm_retry_count += 1;
                    if confirm_retry_count >= max_confirm_retry {
                        log::error!("确认订单失败，已达最大重试次数");
                        let task_result = TaskResult::GrabTicketResult(GrabTicketResult {
                            task_id: task_id.clone(),
                            uid,
                            success: false,
                            message: "确认订单失败，已达最大重试次数".to_string(),
                            order_id: None,
                            pay_token: None,
                            pay_result: None,
                            confirm_result: None,
                        });
                        let _ = result_tx.send(task_result).await;
                        return;
                    }
                }
            }
            Err(risk_param) => {
                if risk_param.code == -401 || risk_param.code == 401 {
                    log::warn!("需要验证码，开始处理验证码...");
                    match handle_risk_verification(
                        cookie_manager.clone(),
                        risk_param,
                        &custon_config,
                        &csrf,
                        local_captcha.clone().expect("REASON"),
                    )
                    .await
                    {
                        Ok(()) => log::info!("验证码处理成功！"),
                        Err(e) => {
                            log::error!("验证码处理失败: {}", e);
                            token_retry_count += 1;
                            if token_retry_count >= max_token_retry {
                                let task_result = TaskResult::GrabTicketResult(GrabTicketResult {
                                    task_id: task_id.clone(),
                                    uid,
                                    success: false,
                                    message: format!("验证码处理失败，已达最大重试次数: {}", e),
                                    order_id: None,
                                    pay_token: None,
                                    pay_result: None,
                                    confirm_result: None,
                                });
                                let _ = result_tx.send(task_result).await;
                                return;
                            }
                        }
                    }
                } else {
                    match risk_param.code {
                        100080 | 100082 => {
                            log::error!("抢票失败，场次/项目/日期选择有误，请重新提交任务")
                        }
                        100039 => log::error!("抢票失败，该场次已停售，请重新提交任务"),
                        _ => log::error!("抢票失败，未知错误，请重新提交任务"),
                    }
                    token_retry_count += 1;
                    if token_retry_count >= max_token_retry {
                        let task_result = TaskResult::GrabTicketResult(GrabTicketResult {
                            task_id: task_id.clone(),
                            uid,
                            success: false,
                            message: format!(
                                "获取token失败，错误代码: {}，错误信息：{}",
                                risk_param.code, risk_param.message
                            ),
                            order_id: None,
                            pay_token: None,
                            pay_result: None,
                            confirm_result: None,
                        });
                        let _ = result_tx.send(task_result).await;
                        return;
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}
async fn leak_grab_ticket_mode(
    cookie_manager: Arc<CookieManager>,
    cpdd: Arc<Mutex<CTokenGenerator>>,
    project_id: String,
    _screen_id: String,
    _ticket_id: String,
    count: i16,
    mut is_hot: bool,
    skip_words: Option<Vec<String>>,
    rng: StdRng,
    task_id: String,
    uid: i64,
    result_tx: &mpsc::Sender<TaskResult>,
    mut grab_ticket_req: GrabTicketRequest,
    buyer_info: Vec<BuyerInfo>,
    custon_config: CustomConfig,
    csrf: String,
    local_captcha: Option<common::captcha::LocalCaptcha>,
) {
    log::debug!("捡漏模式");
    let mut token_retry_count = 0;
    let max_token_retry = custon_config.max_token_retry as i8;

    'main_loop: loop {
        let project_data =
            match get_project(cookie_manager.clone(), project_id.clone().as_str()).await {
                Ok(data) => data,
                Err(e) => {
                    log::error!("获取项目数据失败，原因：{}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }
            };
        is_hot = project_data.data.hot_project;

        if ![1, 2].contains(&project_data.data.id_bind) {
            log::error!("暂不支持抢非实名票捡漏模式");
            break 'main_loop;
        }
        grab_ticket_req.biliticket.id_bind = project_data.data.id_bind as usize;

        'screen_loop: for screen_data in project_data.data.screen_list {
            if !screen_data.clickable {
                continue;
            }

            grab_ticket_req.screen_id = screen_data.id.to_string();
            grab_ticket_req.biliticket.screen_id = screen_data.id.to_string();
            log::info!("当前项目有可抢票场次，开始抢票！");

            'ticket_loop: for ticket_data in screen_data.ticket_list {
                if !ticket_data.clickable {
                    continue;
                }
                if let Some(ref skip_words) = skip_words {
                    let title = ticket_data.screen_name.to_lowercase();
                    if skip_words
                        .iter()
                        .any(|word| title.contains(&word.to_lowercase()))
                    {
                        log::info!("跳过包含过滤关键词的场次: {}", ticket_data.screen_name);
                        continue;
                    }
                    let ticket_title = ticket_data.desc.to_lowercase();
                    if skip_words
                        .iter()
                        .any(|word| ticket_title.contains(&word.to_lowercase()))
                    {
                        log::info!("跳过包含过滤关键词的票种: {}", ticket_data.desc);
                        continue;
                    }
                }

                log::info!(
                    "当前{} {}票种可售，开始抢票！",
                    ticket_data.screen_name,
                    ticket_data.desc
                );
                grab_ticket_req.ticket_id = ticket_data.id.to_string();
                grab_ticket_req.biliticket.select_ticket_id = Some(ticket_data.id.to_string());

                let token_result = get_ticket_token(
                    cookie_manager.clone(),
                    cpdd.clone(),
                    &project_id,
                    &grab_ticket_req.screen_id,
                    &grab_ticket_req.ticket_id,
                    count,
                    is_hot,
                )
                .await;

                match token_result {
                    Ok((token, ptoken)) => {
                        log::info!("获取抢票token成功！:{} ptoken:{}", token, ptoken);
                        let mut confirm_retry_count = 0;
                        let max_confirm_retry = custon_config.max_confirm_retry as i8;

                        loop {
                            let (success, retry_limit) = handle_grab_ticket(
                                cookie_manager.clone(),
                                cpdd.clone(),
                                &project_id,
                                &token,
                                &ptoken,
                                is_hot,
                                &task_id,
                                uid,
                                &result_tx,
                                &grab_ticket_req,
                                &buyer_info,
                            )
                            .await;
                            if success {
                                log::info!("抢票流程结束，退出捡漏模式");
                                break 'main_loop;
                            }
                            if retry_limit {
                                log::info!("该票种已达到最大重试次数，恢复捡漏模式，尝试其他票种");
                                break 'screen_loop;
                            }

                            confirm_retry_count += 1;
                            if confirm_retry_count >= max_confirm_retry {
                                log::error!("确认订单失败，已达最大重试次数，尝试其他票种");
                                break;
                            }

                            tokio::time::sleep(tokio::time::Duration::from_secs_f32(0.3)).await;
                        }
                    }
                    Err(risk_param) => {
                        if risk_param.code == -401 || risk_param.code == 401 {
                            log::warn!("需要验证码，开始处理验证码...");
                            match handle_risk_verification(
                                cookie_manager.clone(),
                                risk_param,
                                &custon_config,
                                &csrf,
                                local_captcha.clone().expect("REASON"),
                            )
                            .await
                            {
                                Ok(()) => log::info!("验证码处理成功！"),
                                Err(e) => {
                                    log::error!("验证码处理失败: {}", e);
                                    token_retry_count += 1;
                                    if token_retry_count >= max_token_retry {
                                        let task_result =
                                            TaskResult::GrabTicketResult(GrabTicketResult {
                                                task_id: task_id.clone(),
                                                uid,
                                                success: false,
                                                message: format!(
                                                    "验证码处理失败，已达最大重试次数: {}",
                                                    e
                                                ),
                                                order_id: None,
                                                pay_token: None,
                                                pay_result: None,
                                                confirm_result: None,
                                            });
                                        let _ = result_tx.send(task_result).await;
                                        break 'main_loop;
                                    }
                                }
                            }
                        } else {
                            match risk_param.code {
                                100080 | 100082 => log::error!("抢票失败，场次/项目/日期选择有误"),
                                100039 => log::error!("抢票失败，该场次已停售"),
                                _ => log::error!("抢票失败，未知错误"),
                            }
                            token_retry_count += 1;
                            if token_retry_count >= max_token_retry {
                                let task_result = TaskResult::GrabTicketResult(GrabTicketResult {
                                    task_id: task_id.clone(),
                                    uid,
                                    success: false,
                                    message: format!(
                                        "获取token失败，错误代码: {}，错误信息：{}",
                                        risk_param.code, risk_param.message
                                    ),
                                    order_id: None,
                                    pay_token: None,
                                    pay_result: None,
                                    confirm_result: None,
                                });
                                let _ = result_tx.send(task_result).await;
                                break 'main_loop;
                            }
                        }
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                }
            }
        }

        log::info!("所有场次和票种检查完毕，等待2秒后重新检查");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }
    log::info!("捡漏模式任务已退出");
}
async fn handle_grab_ticket(
    cookie_manager: Arc<CookieManager>,
    cpdd: Arc<Mutex<CTokenGenerator>>,
    project_id: &str,
    token: &str,
    ptoken: &str,
    is_hot: bool,
    task_id: &str,
    uid: i64,
    result_tx: &mpsc::Sender<TaskResult>,
    grab_ticket_req: &GrabTicketRequest,
    buyer_info: &Vec<BuyerInfo>,
) -> (bool, bool) {
    // 确认订单
    match confirm_ticket_order(cookie_manager.clone(), project_id, token).await {
        Ok(confirm_result) => {
            log::info!("确认订单成功！准备下单");

            if let Some((success, retry_limit)) = try_create_order(
                cookie_manager.clone(),
                cpdd,
                project_id,
                token,
                ptoken,
                &confirm_result,
                is_hot,
                grab_ticket_req,
                buyer_info,
                task_id,
                uid,
                result_tx,
            )
            .await
            {
                return (success, retry_limit);
            }

            (true, false) // 订单流程已完成
        }
        Err(e) => {
            log::error!("确认订单失败，原因：{}  正在重试...", e);
            (false, false) // 需要继续重试
        }
    }
}

// 处理创建订单逻辑
async fn try_create_order(
    cookie_manager: Arc<CookieManager>,
    cpdd: Arc<Mutex<CTokenGenerator>>,
    project_id: &str,
    token: &str,
    ptoken: &str,
    confirm_result: &ConfirmTicketResult,
    is_hot: bool,
    grab_ticket_req: &GrabTicketRequest,
    buyer_info: &Vec<BuyerInfo>,
    task_id: &str,
    uid: i64,
    result_tx: &mpsc::Sender<TaskResult>,
) -> Option<(
    bool,
    bool, // 第二个参数标记是因为达到重试上限
)> {
    let mut order_retry_count = 0;
    let mut need_retry = false;

    // 下单循环
    loop {
        if order_retry_count >= 3 {
            need_retry = true;
        }

        match create_order(
            cookie_manager.clone(),
            cpdd.clone(),
            project_id,
            token,
            ptoken,
            confirm_result,
            is_hot,
            &grab_ticket_req.biliticket,
            buyer_info,
            true,
            need_retry,
            false,
            None,
        )
        .await
        {
            Ok(order_result) => {
                log::info!("下单成功！订单信息{:?}", order_result);
                let empty_json = json!({});
                let order_data = order_result.get("data").unwrap_or(&empty_json);

                let zero_json = json!(0);
                let order_id = order_data
                    .get("orderId")
                    .unwrap_or(&zero_json)
                    .as_i64()
                    .unwrap_or(0);

                let empty_string_json = json!("");
                let pay_token = order_data
                    .get("token")
                    .unwrap_or(&empty_string_json)
                    .as_str()
                    .unwrap_or("");

                log::info!("下单成功！正在检测是否假票！");

                // 假票检测重试循环
                let mut fake_check_retry = 0;
                let max_fake_check_retry =
                    grab_ticket_req.biliticket.config.max_fake_check_retry as i32;

                loop {
                    let check_result = match check_fake_ticket(
                        cookie_manager.clone(),
                        project_id,
                        pay_token,
                        order_id,
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(e) => {
                            log::error!("检测假票失败，原因：{}", e);
                            fake_check_retry += 1;
                            if fake_check_retry >= max_fake_check_retry {
                                log::error!("检测假票多次失败，默认下单成功，请前往订单中心支付");
                                // 即使检测失败，也视为抢票成功，只是没有支付二维码
                                let task_result = TaskResult::GrabTicketResult(GrabTicketResult {
                                    task_id: task_id.to_string(),
                                    uid,
                                    success: true,
                                    message: "抢票成功，但获取支付信息失败，请前往B站订单中心支付"
                                        .to_string(),
                                    order_id: Some(order_id.to_string()),
                                    pay_token: Some(pay_token.to_string()),
                                    confirm_result: Some(confirm_result.clone()),
                                    pay_result: None,
                                });
                                let _ = result_tx.send(task_result).await;
                                return Some((true, false));
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            continue;
                        }
                    };

                    let errno = check_result
                        .get("errno")
                        .unwrap_or(&zero_json)
                        .as_i64()
                        .unwrap_or(0);

                    if errno != 0 {
                        log::error!("检测到假票(errno={})，放弃当前订单，继续抢票", errno);
                        // 假票，跳出内层循环，继续外层 create_order 循环
                        break;
                    }

                    let analyze_result = match serde_json::from_value::<CheckFakeResult>(
                        check_result.clone(),
                    ) {
                        Ok(result) => result,
                        Err(e) => {
                            log::error!("解析假票结果失败，原因：{}", e);
                            fake_check_retry += 1;
                            if fake_check_retry >= max_fake_check_retry {
                                log::error!("解析支付信息多次失败，默认下单成功");
                                let task_result = TaskResult::GrabTicketResult(GrabTicketResult {
                                    task_id: task_id.to_string(),
                                    uid,
                                    success: true,
                                    message: "抢票成功，但解析支付信息失败，请前往B站订单中心支付"
                                        .to_string(),
                                    order_id: Some(order_id.to_string()),
                                    pay_token: Some(pay_token.to_string()),
                                    confirm_result: Some(confirm_result.clone()),
                                    pay_result: None,
                                });
                                let _ = result_tx.send(task_result).await;

                                let project_name = &confirm_result.project_name;
                                let screen_name = &confirm_result.screen_name;
                                let ticket_name = &confirm_result.ticket_info.name;
                                let jump_url = Some(format!("bilibili://mall/web?url=https://mall.bilibili.com/neul-next/ticket/orderDetail.html?order_id={}", order_id.to_string()));
                                let title = format!("抢票成功: {}", project_name);
                                let message = format!(
                                    "项目: {}\n场次: {}\n票种: {}\n订单号: {}\n状态: 解析支付信息失败，请前往订单中心支付",
                                    project_name, screen_name, ticket_name, order_id
                                );
                                grab_ticket_req
                                    .biliticket
                                    .push_self
                                    .push_all_async(&title, &message, &jump_url)
                                    .await;

                                return Some((true, false));
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            continue;
                        }
                    };

                    let pay_result = analyze_result.data.pay_param;
                    // 通知成功
                    let task_result = TaskResult::GrabTicketResult(GrabTicketResult {
                        task_id: task_id.to_string(),
                        uid,
                        success: true,
                        message: "抢票成功".to_string(),
                        order_id: Some(order_id.clone().to_string()),
                        pay_token: Some(pay_token.to_string()),
                        confirm_result: Some(confirm_result.clone()),
                        pay_result: Some(pay_result.clone()),
                    });
                    let _ = result_tx.send(task_result.clone()).await;

                    let project_name = &confirm_result.project_name;
                    let screen_name = &confirm_result.screen_name;
                    let ticket_name = &confirm_result.ticket_info.name;
                    let jump_url = Some(format!("bilibili://mall/web?url=https://mall.bilibili.com/neul-next/ticket/orderDetail.html?order_id={}", order_id.to_string()));
                    let title = format!("抢票成功: {}", project_name);
                    let message = format!(
                        "项目: {}\n场次: {}\n票种: {}\n订单号: {}\n请尽快支付！",
                        project_name, screen_name, ticket_name, order_id
                    );
                    log::info!("准备发送推送通知... 启用渠道: {:?}", grab_ticket_req.biliticket.push_self.enabled_methods);
                    let (push_success, push_msg) = grab_ticket_req
                        .biliticket
                        .push_self
                        .push_all_async(&title, &message, &jump_url)
                        .await;
                    log::info!("推送结果: 成功={}, 信息={}", push_success, push_msg);

                    return Some((true, false)); // 成功，不需要继续重试
                }

                // 如果是从 break 跳出（假票），则继续外层循环（create_order）
                // 这里不需要显式 continue，因为 break 后会执行到下面的 order_retry_count += 1
            }

            Err(e) => {
                // 处理错误情况
                match e {
                    //需要继续重试的临时错误
                    100001 | 429 | 900001 => log::info!("b站限速，正常现象"),
                    100009 => {
                        log::info!("当前票种库存不足");
                        tokio::time::sleep(tokio::time::Duration::from_secs_f32(0.6)).await;
                    }
                    211 => log::info!("很遗憾，差一点点抢到票，继续加油吧！"),

                    //需要暂停的情况
                    3 => {
                        log::info!("抢票速度过快，即将被硬控5秒");
                        log::info!("暂停4.8秒");
                        tokio::time::sleep(tokio::time::Duration::from_secs_f32(4.8)).await;
                    }

                    //需要重新获取token的情况
                    100041 | 100050 | 900002 => {
                        log::info!("token失效，即将重新获取token");
                        return Some((true, true));
                    }

                    //需要终止抢票的致命错误
                    100017 | 100016 => {
                        log::info!("当前项目/类型/场次已停售");
                        return Some((true, false));
                    }
                    1 => {
                        log::error!(
                            "超人 请慢一点，这是仅限1人抢票的项目，或抢票格式有误，请重新提交任务"
                        );
                        return Some((true, false));
                    }
                    83000004 => {
                        log::error!("没有配置购票人信息！请重新配置");
                        return Some((true, false));
                    }
                    100079 | 100003 | 100048 => {
                        log::error!("购票人存在待付款订单，请前往支付或取消后重新下单");
                        let task_result = TaskResult::GrabTicketResult(GrabTicketResult {
                            task_id: task_id.to_string(),
                            uid,
                            success: false,
                            message: "购票人存在待付款订单，请前往支付或取消后重新下单".to_string(),
                            order_id: None,
                            pay_token: None,
                            pay_result: None,
                            confirm_result: None,
                        });
                        let _ = result_tx.send(task_result).await;
                        return Some((true, false));
                    }
                    100039 => {
                        log::error!("活动收摊啦,下次要快点哦");
                        return Some((true, false));
                    }
                    209001 => {
                        log::error!("当前项目只能选择一个购票人！不支持多选，请重新提交任务");
                        return Some((true, false));
                    }
                    737 => {
                        log::error!(
                            "B站传了一个null回来，请看一下上一行的message提示信息，自行决定是否继续，如果取消请关闭重新打开该应用"
                        );
                    }
                    999 => log::error!("程序内部错误！传参错误"),
                    919 => {
                        log::error!(
                            "程序内部错误！该项目区分绑定非绑定项目错误，传入意外值，请尝试重新下单以及提出issue"
                        );
                        return Some((true, false));
                    }
                    _ => log::error!("下单失败，未知错误码：{} 可以提出issue修复该问题", e),
                }
            }
        }

        order_retry_count += 1;
        if grab_ticket_req.grab_mode == 2
            && order_retry_count >= grab_ticket_req.biliticket.config.max_order_retry as i32
        {
            log::error!(
                "捡漏模式下单失败，已达最大重试次数，放弃该票种抢票，准备检测其他票种继续捡漏"
            );
            return Some((false, true));
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(
            grab_ticket_req.biliticket.config.retry_interval_ms,
        ))
        .await;
    }
}

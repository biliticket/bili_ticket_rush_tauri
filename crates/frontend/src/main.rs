#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;
mod utils;

use crate::commands::*;
use crate::state::AppState;
use tauri::Emitter;
use tauri::Manager;

fn main() {
    if let Err(e) = common::record_log::init() {
        eprintln!("初始化日志失败，原因: {}", e);
    }
    log::info!("日志初始化成功");

    if !common::utils::ensure_single_instance() {
        eprintln!("程序已经在运行中，请勿重复启动！");
        std::thread::sleep(std::time::Duration::from_secs(5));
        std::process::exit(1);
    }

    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            let handle = app.handle().clone();
            common::record_log::add_log_listener(move |message| {
                let _ = handle.emit("log-event", message);
            });

            let handle_task = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(500));

                    let state = handle_task.state::<AppState>();

                    if let Ok(state_inner) = state.inner.lock() {
                        if let Ok(mut task_manager) = state_inner.task_manager.lock() {
                            let results: Vec<common::taskmanager::TaskResult> =
                                task_manager.get_results();
                            for result in results {
                                if let Err(e) = handle_task.emit("task-update", &result) {
                                    log::error!("任务更新事件无法发出: {}", e);
                                }
                            }
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            account::get_accounts,
            account::reload_accounts,
            account::add_account_by_cookie,
            account::delete_account_by_uid,
            account::set_account_active,
            account::set_selected_account,
            account::set_delete_account,
            account::set_account_switch,
            auth::qrcode_login,
            auth::poll_qrcode_status,
            auth::send_loginsms_command,
            auth::submit_loginsms_command,
            auth::password_login_command,
            auth::set_login_method,
            auth::set_show_login_window,
            auth::set_login_input,
            auth::set_cookie_login,
            auth::get_country_list_command,
            task::get_ticket_info,
            task::get_buyer_info,
            task::get_order_list,
            task::poll_task_results,
            task::cancel_task,
            task::start_grab_ticket,
            ticket::set_ticket_id,
            ticket::set_grab_mode,
            ticket::set_show_screen_info,
            ticket::set_confirm_ticket_info,
            ticket::set_show_add_buyer_window,
            ticket::set_show_orderlist_window,
            ticket::set_selected_screen,
            ticket::set_selected_ticket,
            ticket::set_selected_buyer_list,
            ticket::set_buyer_type,
            ticket::set_no_bind_buyer_info,
            ticket::clear_no_bind_buyer_info,
            general::push_test,
            general::get_policy,
            general::get_logs,
            general::get_app_info,
            general::clear_grab_logs,
            general::set_show_qr_windows,
            general::set_skip_words,
            general::set_skip_words_input,
            general::get_state,
            general::add_project,
            general::get_projects,
            general::delete_project,
            general::get_monitor_stats,
            general::get_recent_logs,
            general::save_settings,
            general::clear_logs,
        ])
        .run(tauri::generate_context!())
        .expect("tauri run failed");
}

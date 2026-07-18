mod commands;
mod events;
mod jobs;
mod openai;
mod proposals;
mod providers;
mod secrets;
mod state;
mod storage;
mod tools;

use state::AppState;
use tauri::{
    menu::{Menu, MenuItemBuilder},
    tray::TrayIconBuilder,
    Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};

const DAEMON_WINDOW: &str = "daemon";
const TRIGGER_EVENT: &str = "daemon://trigger";
const DISMISS_EVENT: &str = "daemon://dismiss";

fn summon_daemon(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(DAEMON_WINDOW) {
        let _ = window.show();
        let _ = window.center();
        let _ = window.set_focus();
    }
    let _ = app.emit_to(DAEMON_WINDOW, TRIGGER_EVENT, ());
}

fn dismiss_daemon(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(DAEMON_WINDOW) {
        let _ = window.hide();
    }
    let _ = app.emit_to(DAEMON_WINDOW, DISMISS_EVENT, ());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let database_path = data_dir.join("daemon.sqlite3");
            let state = AppState::new(&database_path)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            app.manage(state);
            WebviewWindowBuilder::new(app, DAEMON_WINDOW, WebviewUrl::App("index.html".into()))
                .transparent(true)
                .shadow(false)
                .decorations(false)
                .resizable(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .inner_size(100.0, 100.0)
                .min_inner_size(1.0, 1.0)
                .build()?;
            let summon = MenuItemBuilder::with_id("daemon_summon", "Summon daemon").build(app)?;
            let dismiss = MenuItemBuilder::with_id("daemon_dismiss", "Dismiss").build(app)?;
            let quit = MenuItemBuilder::with_id("daemon_quit", "Quit").build(app)?;
            let menu = Menu::with_items(app, &[&summon, &dismiss, &quit])?;

            let mut tray = TrayIconBuilder::with_id("daemon-tray")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .tooltip("Daemon")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "daemon_summon" => summon_daemon(app),
                    "daemon_dismiss" => dismiss_daemon(app),
                    "daemon_quit" => {
                        app.cleanup_before_exit();
                        app.exit(0);
                    }
                    _ => {}
                });

            if let Some(icon) = app.default_window_icon().cloned() {
                tray = tray.icon(icon);
            }

            tray.build(app)?;

            Ok(())
        })
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                "daemon_toolbox_settings" => {
                    let _ = app.emit_to(DAEMON_WINDOW, events::TOOLBOX_OPEN, "settings");
                }
                "daemon_toolbox_about" => {
                    let _ = app.emit_to(DAEMON_WINDOW, events::TOOLBOX_OPEN, "about");
                }
                "daemon_toolbox_dismiss" => dismiss_daemon(app),
                "daemon_toolbox_quit" => {
                    app.cleanup_before_exit();
                    app.exit(0);
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::save_api_key,
            commands::get_auth_status,
            commands::disconnect_api_key,
            commands::show_toolbox_menu,
            commands::list_providers,
            commands::save_provider,
            commands::select_provider,
            commands::delete_provider_key,
            commands::delete_provider,
            commands::undo_note,
            commands::create_model_response,
            commands::validate_tool_call,
            commands::describe_repo,
            commands::create_run_codex_proposal,
            commands::approve_proposal,
            commands::deny_proposal,
            commands::pending_proposals,
            commands::submit_conversation_turn,
        ])
        .run(tauri::generate_context!())
        .expect("error");
}

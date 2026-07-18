mod commands;
mod events;
mod jobs;
mod openai;
mod proposals;
mod providers;
mod secrets;
mod screen_aware;
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

fn bundled_model_path(app: &tauri::App) -> std::io::Result<std::path::PathBuf> {
    let name = "moondream-0_5b-int4.bin";
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let candidates = [
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../model")
            .join(name),
        resource_dir.join("model").join(name),
        resource_dir.join(name),
    ];
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Bundled Moondream2 model is missing"))
}

fn summon_daemon(app: &tauri::AppHandle) {
    app.state::<AppState>().screen_aware.set_monitoring_active(true);
    if let Some(window) = app.get_webview_window(DAEMON_WINDOW) {
        let _ = window.show();
        let _ = window.center();
        let _ = window.set_focus();
    }
    let _ = app.emit_to(DAEMON_WINDOW, TRIGGER_EVENT, ());
}

fn dismiss_daemon(app: &tauri::AppHandle) {
    app.state::<AppState>().screen_aware.set_monitoring_active(false);
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
            let model_archive_path = bundled_model_path(app)?;
            let state = AppState::new(
                &database_path,
                model_archive_path,
                data_dir.join("moondream2"),
            )
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            app.manage(state);
            let state = app.state::<AppState>();
            let settings = state
                .storage
                .lock()
                .map_err(|_| std::io::Error::other("Local storage is unavailable"))?
                .screen_aware_settings()
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            state
                .screen_aware
                .restart_monitor(app.handle().clone(), settings.interval_seconds);
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
            commands::get_screen_aware_settings,
            commands::save_screen_aware_settings,
            commands::capture_screen_observation,
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

use tauri::{
    menu::{Menu, MenuItemBuilder},
    tray::TrayIconBuilder,
    Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};

mod ai;

const DAEMON_WINDOW: &str = "daemon";
const TRIGGER_EVENT: &str = "daemon://trigger";
const DISMISS_EVENT: &str = "daemon://dismiss";

fn emit_daemon_event(app: &tauri::AppHandle, event: &str) {
    if let Some(window) = app.get_webview_window(DAEMON_WINDOW) {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit(event, ());
    } else {
        let _ = app.emit_to(DAEMON_WINDOW, event, ());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenvy::dotenv().ok();

    tauri::Builder::default()
        .setup(|app| {
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
                    "daemon_summon" => emit_daemon_event(app, TRIGGER_EVENT),
                    "daemon_dismiss" => emit_daemon_event(app, DISMISS_EVENT),
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
        .invoke_handler(tauri::generate_handler![ai::ask_ai, ai::next_daemon_line])
        .run(tauri::generate_context!())
        .expect("error");
}

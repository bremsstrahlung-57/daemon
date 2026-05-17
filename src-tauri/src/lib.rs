use tauri::{WebviewUrl, WebviewWindowBuilder};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            WebviewWindowBuilder::new(app, "daemon", WebviewUrl::App("index.html".into()))
                .transparent(true)
                .shadow(false)
                .decorations(false)
                .always_on_top(true)
                .inner_size(100.0, 100.0)
                .min_inner_size(100.0, 100.0)
                .max_inner_size(100.0, 100.0)
                .build()?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error");
}

mod auth;
#[cfg(desktop)]
use tauri::Manager;
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));
    builder
        .plugin(tauri_plugin_opener::init())
        .manage(auth::AuthState::default())
        .invoke_handler(tauri::generate_handler![
            greet,
            auth::auth_get_status,
            auth::auth_connect_apps,
            auth::auth_start_login,
            auth::auth_cancel_login,
            auth::auth_logout,
            auth::auth_forget_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

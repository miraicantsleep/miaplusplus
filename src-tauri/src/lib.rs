mod auth;
mod auth_storage;
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(auth::AuthState::default())
        .invoke_handler(tauri::generate_handler![
            greet,
            auth::auth_get_status,
            auth::auth_start_login,
            auth::auth_cancel_login,
            auth::auth_logout
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

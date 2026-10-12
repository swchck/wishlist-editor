mod commands;
mod error;
mod workspace;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::open_workspace,
            commands::list_items,
            commands::write_item,
            commands::create_item,
            commands::delete_item,
            commands::rename_item,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

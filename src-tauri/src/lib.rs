mod auth;
mod commands;
mod create;
mod error;
mod git;
mod github;
mod photos;
mod remove;
mod workspace;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            let data_dir = app.path().app_data_dir()?;
            app.manage(commands::AppState::new(data_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth_status,
            commands::auth_start,
            commands::auth_wait,
            commands::auth_cancel,
            commands::sign_out,
            commands::list_wishlists,
            commands::local_wishlists,
            commands::open_repo,
            commands::close_repo,
            commands::reload,
            commands::repo_status,
            commands::discard_change,
            commands::publish,
            commands::deploy_status,
            commands::list_items,
            commands::write_item,
            commands::create_item,
            commands::delete_item,
            commands::rename_item,
            commands::add_photo,
            commands::read_photo,
            commands::remove_photo,
            create::check_name,
            create::create_repo,
            create::ensure_pages,
            create::check_reservations,
            create::finish_setup,
            remove::local_status,
            remove::forget_local,
            remove::delete_start,
            remove::delete_wait,
            remove::delete_cancel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

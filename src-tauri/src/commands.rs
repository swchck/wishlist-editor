//! Tauri commands exposed to the webview.

use std::sync::Mutex;

use tauri::State;

use crate::error::{AppError, AppResult};
use crate::workspace::{RawItem, Snapshot, Workspace};

#[derive(Default)]
pub struct AppState {
    workspace: Mutex<Option<Workspace>>,
}

impl AppState {
    fn with<T>(&self, f: impl FnOnce(&Workspace) -> AppResult<T>) -> AppResult<T> {
        let guard = self.workspace.lock().unwrap_or_else(|e| e.into_inner());
        f(guard.as_ref().ok_or(AppError::NoWorkspace)?)
    }
}

#[tauri::command]
pub fn open_workspace(path: String, state: State<'_, AppState>) -> AppResult<Snapshot> {
    let ws = Workspace::open(path)?;
    let snapshot = ws.snapshot()?;
    *state.workspace.lock().unwrap_or_else(|e| e.into_inner()) = Some(ws);
    Ok(snapshot)
}

#[tauri::command]
pub fn list_items(state: State<'_, AppState>) -> AppResult<Vec<RawItem>> {
    state.with(Workspace::list_items)
}

#[tauri::command]
pub fn write_item(slug: String, text: String, state: State<'_, AppState>) -> AppResult<()> {
    state.with(|ws| ws.write_item(&slug, &text))
}

#[tauri::command]
pub fn create_item(slug: String, text: String, state: State<'_, AppState>) -> AppResult<()> {
    state.with(|ws| ws.create_item(&slug, &text))
}

#[tauri::command]
pub fn delete_item(slug: String, state: State<'_, AppState>) -> AppResult<()> {
    state.with(|ws| ws.delete_item(&slug))
}

#[tauri::command]
pub fn rename_item(from: String, to: String, state: State<'_, AppState>) -> AppResult<()> {
    state.with(|ws| ws.rename_item(&from, &to))
}

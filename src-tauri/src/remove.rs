//! Removing a wishlist: from this computer only, or from GitHub altogether.
//!
//! Deleting needs the `delete_repo` scope, which the stored sign-in token deliberately lacks.
//! Each deletion asks for it through a fresh device-flow confirmation; that token deletes one
//! repository and is dropped without being written anywhere.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use git2::Repository;
use serde::Serialize;
use tauri::State;
use tauri::async_runtime::spawn_blocking;

use crate::auth::{self, DevicePrompt};
use crate::commands::{AppState, RepoId, lock};
use crate::create::TEMPLATE;
use crate::error::{AppError, AppResult};
use crate::git;
use crate::github::GitHub;
use crate::workspace::CONFIG_FILE;

/// What would be lost locally: work that never reached GitHub.
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LocalStatus {
    cloned: bool,
    /// Uncommitted edits plus commits that were never pushed.
    unpublished: usize,
}

fn clone_dir(state: &AppState, id: &RepoId) -> PathBuf {
    state.repos_dir().join(&id.owner).join(&id.name)
}

#[tauri::command]
pub async fn local_status(
    owner: String,
    name: String,
    state: State<'_, AppState>,
) -> AppResult<LocalStatus> {
    let dir = clone_dir(&state, &RepoId::validated(owner, name)?);
    spawn_blocking(move || {
        if !dir.join(".git").is_dir() {
            return Ok(LocalStatus::default());
        }
        let repo = Repository::open(&dir)?;
        Ok(LocalStatus {
            cloned: true,
            unpublished: git::changes(&repo)?.len() + git::divergence(&repo)?.ahead,
        })
    })
    .await
    .map_err(|e| AppError::Git(e.to_string()))?
}

async fn remove_clone(state: &AppState, id: &RepoId) -> AppResult<()> {
    state.close_if_open(id);
    let dir = clone_dir(state, id);
    let repo_lock = state.repo_lock();
    spawn_blocking(move || {
        let _guard = lock(&repo_lock);
        // remove_dir_all deletes symlinks found inside, never what they point to
        match fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(AppError::from(e)),
        }
    })
    .await
    .map_err(|e| AppError::Git(e.to_string()))?
}

#[tauri::command]
pub async fn forget_local(
    owner: String,
    name: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    remove_clone(&state, &RepoId::validated(owner, name)?).await
}

#[tauri::command]
pub async fn delete_start(name: String, state: State<'_, AppState>) -> AppResult<DevicePrompt> {
    let session = state.session()?;
    let id = RepoId::validated(session.user.login.clone(), name)?;
    if (id.owner.as_str(), id.name.as_str()) == TEMPLATE {
        return Err(AppError::GitHub(
            "шаблон вишлистов удалить отсюда нельзя".into(),
        ));
    }
    let gh = state.github()?;
    // only wishlists: a typo in the name must not be able to reach some other repository
    if gh.repo(&id.owner, &id.name).await?.is_none()
        || !gh.has_file(&id.owner, &id.name, CONFIG_FILE).await?
    {
        return Err(AppError::GitHub(format!(
            "{}/{} — не вишлист, удалять его отсюда нельзя",
            id.owner, id.name
        )));
    }
    let code = auth::request_code(state.http(), auth::DELETE_SCOPE).await?;
    let prompt = DevicePrompt::from(&code);
    state.deletion_generation.fetch_add(1, Ordering::SeqCst);
    *lock(&state.deletion) = Some((code, id));
    Ok(prompt)
}

#[tauri::command]
pub async fn delete_wait(state: State<'_, AppState>) -> AppResult<()> {
    let (code, id) = lock(&state.deletion)
        .clone()
        .ok_or(AppError::AuthCancelled)?;
    let generation = state.deletion_generation.load(Ordering::SeqCst);
    let token = auth::wait_for_token(state.http(), &code, || {
        state.deletion_generation.load(Ordering::SeqCst) != generation
    })
    .await?;
    *lock(&state.deletion) = None;
    let elevated = GitHub::new(state.http().clone(), token);
    // the browser may be signed in to another GitHub account than the app
    let who = elevated.user().await?;
    if who.login != id.owner {
        return Err(AppError::GitHub(format!(
            "код подтвердил аккаунт {}, а вишлист принадлежит {} — войдите в браузере под {}",
            who.login, id.owner, id.owner
        )));
    }
    elevated.delete_repo(&id.owner, &id.name).await?;
    drop(elevated);
    // GitHub answers 404 both for "already gone" and for "not allowed"; only the second
    // leaves the repo visible, and then the local copy must stay
    if state.github()?.repo(&id.owner, &id.name).await?.is_some() {
        return Err(AppError::GitHub(format!(
            "GitHub не удалил {}/{} — проверьте права на репозиторий",
            id.owner, id.name
        )));
    }
    remove_clone(&state, &id).await
}

#[tauri::command]
pub fn delete_cancel(state: State<'_, AppState>) {
    state.deletion_generation.fetch_add(1, Ordering::SeqCst);
    *lock(&state.deletion) = None;
}

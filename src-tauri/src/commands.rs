//! Tauri commands exposed to the webview.
//!
//! Git and file work runs on the blocking pool; network calls are async. Locks are only
//! held to copy state out, never across an await.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use git2::Repository;
use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::async_runtime::spawn_blocking;

use crate::auth::{self, DeviceCode, DevicePrompt};
use crate::error::{AppError, AppResult};
use crate::git::{self, Change, Conflict, Integrate, Pull, Side};
use crate::github::{self, DeployRun, GitHub, RemoteWishlist, User};
use crate::workspace::{RawItem, Snapshot, Workspace};

/// Attempts at fetch → replay → push before giving up on a busy remote.
const PUSH_ATTEMPTS: usize = 3;

/// `owner/name` of a GitHub repository, validated so it is safe as two path segments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoId {
    pub owner: String,
    pub name: String,
}

impl RepoId {
    fn validated(owner: String, name: String) -> AppResult<Self> {
        let owner_ok = !owner.is_empty()
            && owner.len() <= 39
            && owner
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-');
        let name_ok = !name.is_empty()
            && name.len() <= 100
            && name != "."
            && name != ".."
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
        if owner_ok && name_ok {
            Ok(Self { owner, name })
        } else {
            Err(AppError::InvalidRepo(format!("{owner}/{name}")))
        }
    }

    fn clone_url(&self) -> String {
        format!("https://github.com/{}/{}.git", self.owner, self.name)
    }
}

#[derive(Clone)]
struct Session {
    token: String,
    user: User,
}

struct OpenRepo {
    id: RepoId,
    ws: Workspace,
}

pub struct AppState {
    http: reqwest::Client,
    data_dir: PathBuf,
    session: Mutex<Option<Session>>,
    repo: Mutex<Option<OpenRepo>>,
    flow: Mutex<Option<DeviceCode>>,
    flow_generation: AtomicU64,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("wishlist-editor/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("static reqwest config");
        Self {
            http,
            data_dir,
            session: Mutex::new(None),
            repo: Mutex::new(None),
            flow: Mutex::new(None),
            flow_generation: AtomicU64::new(0),
        }
    }

    fn repos_dir(&self) -> PathBuf {
        self.data_dir.join("repos")
    }

    fn user_file(&self) -> PathBuf {
        self.data_dir.join("user.json")
    }

    fn session(&self) -> AppResult<Session> {
        lock(&self.session).clone().ok_or(AppError::NotSignedIn)
    }

    fn github(&self) -> AppResult<GitHub> {
        Ok(GitHub::new(self.http.clone(), self.session()?.token))
    }

    fn open_repo(&self) -> AppResult<(RepoId, PathBuf)> {
        let guard = lock(&self.repo);
        let open = guard.as_ref().ok_or(AppError::NoWorkspace)?;
        Ok((open.id.clone(), open.ws.root().to_path_buf()))
    }

    fn with_ws<T>(&self, f: impl FnOnce(&Workspace) -> AppResult<T>) -> AppResult<T> {
        let guard = lock(&self.repo);
        f(&guard.as_ref().ok_or(AppError::NoWorkspace)?.ws)
    }

    fn remember_user(&self, user: &User) {
        // a display cache for offline starts only; losing it costs nothing
        let _ = fs::create_dir_all(&self.data_dir);
        let _ = fs::write(
            self.user_file(),
            serde_json::to_vec(user).unwrap_or_default(),
        );
    }

    fn cached_user(&self) -> Option<User> {
        serde_json::from_slice(&fs::read(self.user_file()).ok()?).ok()
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    spawn_blocking(f)
        .await
        .map_err(|e| AppError::Git(format!("background task failed: {e}")))?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    user: Option<User>,
    /// True when GitHub was unreachable and `user` comes from the last session.
    offline: bool,
}

#[tauri::command]
pub async fn auth_status(state: State<'_, AppState>) -> AppResult<AuthStatus> {
    if let Some(s) = lock(&state.session).clone() {
        return Ok(AuthStatus {
            user: Some(s.user),
            offline: false,
        });
    }
    let Some(token) = auth::load_token()? else {
        return Ok(AuthStatus {
            user: None,
            offline: false,
        });
    };
    match GitHub::new(state.http.clone(), token.clone()).user().await {
        Ok(user) => {
            state.remember_user(&user);
            *lock(&state.session) = Some(Session {
                token,
                user: user.clone(),
            });
            Ok(AuthStatus {
                user: Some(user),
                offline: false,
            })
        }
        Err(AppError::Unauthorized) => {
            auth::clear_token()?;
            Ok(AuthStatus {
                user: None,
                offline: false,
            })
        }
        Err(AppError::Offline) => {
            let user = state.cached_user();
            if let Some(user) = &user {
                *lock(&state.session) = Some(Session {
                    token,
                    user: user.clone(),
                });
            }
            Ok(AuthStatus {
                user,
                offline: true,
            })
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn auth_start(state: State<'_, AppState>) -> AppResult<DevicePrompt> {
    let code = auth::request_code(&state.http).await?;
    let prompt = DevicePrompt::from(&code);
    state.flow_generation.fetch_add(1, Ordering::SeqCst);
    *lock(&state.flow) = Some(code);
    Ok(prompt)
}

#[tauri::command]
pub async fn auth_wait(state: State<'_, AppState>) -> AppResult<User> {
    let code = lock(&state.flow).clone().ok_or(AppError::AuthCancelled)?;
    let generation = state.flow_generation.load(Ordering::SeqCst);
    let token = auth::wait_for_token(&state.http, &code, || {
        state.flow_generation.load(Ordering::SeqCst) != generation
    })
    .await?;
    let user = GitHub::new(state.http.clone(), token.clone())
        .user()
        .await?;
    auth::save_token(&token)?;
    state.remember_user(&user);
    *lock(&state.flow) = None;
    *lock(&state.session) = Some(Session {
        token,
        user: user.clone(),
    });
    Ok(user)
}

#[tauri::command]
pub fn auth_cancel(state: State<'_, AppState>) {
    state.flow_generation.fetch_add(1, Ordering::SeqCst);
    *lock(&state.flow) = None;
}

#[tauri::command]
pub fn sign_out(state: State<'_, AppState>) -> AppResult<()> {
    auth::clear_token()?;
    let _ = fs::remove_file(state.user_file());
    *lock(&state.session) = None;
    *lock(&state.repo) = None;
    Ok(())
}

#[tauri::command]
pub async fn list_wishlists(state: State<'_, AppState>) -> AppResult<Vec<RemoteWishlist>> {
    state.github()?.wishlists().await
}

/// Wishlists already cloned on this computer, for opening while offline.
#[tauri::command]
pub fn local_wishlists(state: State<'_, AppState>) -> AppResult<Vec<RepoId>> {
    let mut out = Vec::new();
    let Ok(owners) = fs::read_dir(state.repos_dir()) else {
        return Ok(out);
    };
    for owner in owners.flatten() {
        let Ok(repos) = fs::read_dir(owner.path()) else {
            continue;
        };
        for repo in repos.flatten() {
            if !repo.path().join(".git").is_dir() {
                continue;
            }
            let (Some(o), Some(n)) = (
                owner.file_name().into_string().ok(),
                repo.file_name().into_string().ok(),
            ) else {
                continue;
            };
            if let Ok(id) = RepoId::validated(o, n) {
                out.push(id);
            }
        }
    }
    out.sort_by(|a, b| (&a.owner, &a.name).cmp(&(&b.owner, &b.name)));
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoState {
    changes: Vec<Change>,
    /// Local commits GitHub does not have yet, e.g. from a publish that could not push.
    ahead: usize,
    behind: usize,
}

fn repo_state(path: &Path) -> AppResult<RepoState> {
    let repo = Repository::open(path)?;
    let d = git::divergence(&repo)?;
    Ok(RepoState {
        changes: git::changes(&repo)?,
        ahead: d.ahead,
        behind: d.behind,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    repo: RepoId,
    snapshot: Snapshot,
    state: RepoState,
    /// `None` when GitHub was unreachable and the local copy opened as is.
    pull: Option<Pull>,
    site_url: String,
}

#[tauri::command]
pub async fn open_repo(
    owner: String,
    name: String,
    state: State<'_, AppState>,
) -> AppResult<Opened> {
    let id = RepoId::validated(owner, name)?;
    let path = state.repos_dir().join(&id.owner).join(&id.name);
    let token = lock(&state.session).as_ref().map(|s| s.token.clone());
    let clone_url = id.clone_url();

    let (ws, pull, snapshot, repo_st) = blocking(move || {
        let pull = if path.join(".git").is_dir() {
            let repo = Repository::open(&path)?;
            match git::fetch(&repo, token.as_deref()) {
                Ok(()) => Some(git::fast_forward(&repo)?),
                Err(AppError::Offline) => None,
                Err(e) => return Err(e),
            }
        } else {
            let token = token.ok_or(AppError::NotSignedIn)?;
            git::clone(&clone_url, &path, Some(&token))?;
            Some(Pull::UpToDate)
        };
        let ws = Workspace::open(&path)?;
        let snapshot = ws.snapshot()?;
        let st = repo_state(&path)?;
        Ok((ws, pull, snapshot, st))
    })
    .await?;

    *lock(&state.repo) = Some(OpenRepo { id: id.clone(), ws });
    Ok(Opened {
        site_url: github::pages_url(&id.owner, &id.name),
        repo: id,
        snapshot,
        state: repo_st,
        pull,
    })
}

#[tauri::command]
pub fn close_repo(state: State<'_, AppState>) {
    *lock(&state.repo) = None;
}

#[tauri::command]
pub fn reload(state: State<'_, AppState>) -> AppResult<Snapshot> {
    state.with_ws(Workspace::snapshot)
}

#[tauri::command]
pub async fn repo_status(state: State<'_, AppState>) -> AppResult<RepoState> {
    let (_, path) = state.open_repo()?;
    blocking(move || repo_state(&path)).await
}

#[tauri::command]
pub async fn discard_change(path: String, state: State<'_, AppState>) -> AppResult<()> {
    let (_, root) = state.open_repo()?;
    blocking(move || git::discard(&Repository::open(&root)?, &path)).await
}

#[derive(Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub enum Published {
    Pushed { sha: String },
    Conflicts { files: Vec<Conflict> },
    NothingToPublish,
}

/// Commits all changes, replays them onto GitHub's branch and pushes.
///
/// On conflicts the commit stays local; calling again with `choices` for every listed
/// file finishes the publish.
#[tauri::command]
pub async fn publish(
    message: String,
    choices: HashMap<String, Side>,
    state: State<'_, AppState>,
) -> AppResult<Published> {
    let session = state.session()?;
    let (_, root) = state.open_repo()?;
    blocking(move || {
        let repo = Repository::open(&root)?;
        let sig = session.user.signature()?;
        git::commit_all(&repo, message.trim(), &sig)?;
        let mut attempt = 0;
        loop {
            attempt += 1;
            git::fetch(&repo, Some(&session.token))?;
            if let Integrate::Conflicts(files) = git::integrate(&repo, &sig, &choices)? {
                return Ok(Published::Conflicts { files });
            }
            if git::divergence(&repo)?.ahead == 0 {
                return Ok(Published::NothingToPublish);
            }
            match git::push(&repo, Some(&session.token)) {
                Ok(oid) => {
                    return Ok(Published::Pushed {
                        sha: oid.to_string(),
                    });
                }
                Err(AppError::PushRejected) if attempt < PUSH_ATTEMPTS => {}
                Err(e) => return Err(e),
            }
        }
    })
    .await
}

#[tauri::command]
pub async fn deploy_status(
    sha: String,
    state: State<'_, AppState>,
) -> AppResult<Option<DeployRun>> {
    if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::GitHub("bad commit id".into()));
    }
    let (id, _) = state.open_repo()?;
    state.github()?.deploy_run(&id.owner, &id.name, &sha).await
}

#[tauri::command]
pub fn list_items(state: State<'_, AppState>) -> AppResult<Vec<RawItem>> {
    state.with_ws(Workspace::list_items)
}

#[tauri::command]
pub fn write_item(slug: String, text: String, state: State<'_, AppState>) -> AppResult<()> {
    state.with_ws(|ws| ws.write_item(&slug, &text))
}

#[tauri::command]
pub fn create_item(slug: String, text: String, state: State<'_, AppState>) -> AppResult<()> {
    state.with_ws(|ws| ws.create_item(&slug, &text))
}

#[tauri::command]
pub fn delete_item(slug: String, state: State<'_, AppState>) -> AppResult<()> {
    state.with_ws(|ws| ws.delete_item(&slug))
}

#[tauri::command]
pub fn rename_item(from: String, to: String, state: State<'_, AppState>) -> AppResult<()> {
    state.with_ws(|ws| ws.rename_item(&from, &to))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_id_rejects_path_tricks() {
        assert!(RepoId::validated("swchck".into(), "wishlist".into()).is_ok());
        assert!(RepoId::validated("swchck".into(), "swchck.github.io".into()).is_ok());
        for (o, n) in [
            ("..", "x"),
            ("a", ".."),
            ("a/b", "c"),
            ("a", "b/c"),
            ("a", "b\\c"),
            ("", "x"),
            ("a", ""),
        ] {
            assert!(RepoId::validated(o.into(), n.into()).is_err(), "{o}/{n}");
        }
    }
}

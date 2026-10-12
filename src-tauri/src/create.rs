//! Creating a new wishlist: a copy of the template, Pages turned on, and a first commit
//! that writes the owner's settings into `wishlist.config.json`.
//!
//! The wizard calls the steps one by one. Each step checks what already exists and does
//! only the missing part, so a closed window or a failed step resumes by running it again.
//! The first deploy is triggered by the settings commit; the template skips the push that
//! creates the copy, when Pages is not on yet.

use std::collections::HashMap;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::State;

use crate::commands::{AppState, Published, RepoId, commit_and_push, open_or_clone};
use crate::error::{AppError, AppResult};
use crate::github::pages_url;
use crate::workspace::{CONFIG_FILE, SCHEMA_VERSION, Workspace};

pub const TEMPLATE: (&str, &str) = ("swchck", "wishlist-template");
/// The template's files land a few seconds after `generate` returns.
const READY_POLLS: u32 = 30;
const READY_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NameCheck {
    Free,
    /// The repo exists and already holds a wishlist: resume or open it.
    Wishlist,
    Taken,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Reservations {
    pub url: String,
    pub anon_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setup {
    pub title: String,
    pub default_currency: String,
    pub reservations: Option<Reservations>,
    pub keep_examples: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PagesState {
    enabled: bool,
    /// Where to turn Pages on by hand when the token may not do it.
    settings_url: String,
    site_url: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReservationsCheck {
    Ready,
    /// Tables predate lists: the owner must run migrations/002-shared-lists.sql.
    OldSchema,
    /// schema.sql was never run in this project.
    NoSchema,
    BadKey,
    Unreachable,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finished {
    /// `None` when an earlier run already pushed the settings.
    sha: Option<String>,
    site_url: String,
}

fn own_repo(state: &AppState, name: String) -> AppResult<RepoId> {
    RepoId::validated(state.session()?.user.login, name)
}

#[tauri::command]
pub async fn check_name(name: String, state: State<'_, AppState>) -> AppResult<NameCheck> {
    let id = own_repo(&state, name)?;
    let gh = state.github()?;
    if gh.repo(&id.owner, &id.name).await?.is_none() {
        return Ok(NameCheck::Free);
    }
    Ok(if gh.has_file(&id.owner, &id.name, CONFIG_FILE).await? {
        NameCheck::Wishlist
    } else {
        NameCheck::Taken
    })
}

#[tauri::command]
pub async fn create_repo(name: String, state: State<'_, AppState>) -> AppResult<()> {
    let id = own_repo(&state, name)?;
    let gh = state.github()?;
    match gh.repo(&id.owner, &id.name).await? {
        None => {
            gh.generate(TEMPLATE, &id.owner, &id.name, "Мой вишлист")
                .await?;
        }
        Some(info) => {
            let ours = info
                .template_repository
                .is_some_and(|t| t.full_name == format!("{}/{}", TEMPLATE.0, TEMPLATE.1));
            if !ours && !gh.has_file(&id.owner, &id.name, CONFIG_FILE).await? {
                return Err(AppError::GitHub(format!(
                    "репозиторий {}/{} уже есть и это не вишлист — выберите другое имя",
                    id.owner, id.name
                )));
            }
        }
    }
    for _ in 0..READY_POLLS {
        if gh.has_file(&id.owner, &id.name, CONFIG_FILE).await? {
            return Ok(());
        }
        tokio::time::sleep(READY_INTERVAL).await;
    }
    Err(AppError::GitHub(
        "GitHub ещё создаёт копию шаблона — подождите минуту и нажмите «Повторить»".into(),
    ))
}

#[tauri::command]
pub async fn ensure_pages(name: String, state: State<'_, AppState>) -> AppResult<PagesState> {
    let id = own_repo(&state, name)?;
    let gh = state.github()?;
    let settings_url = format!("https://github.com/{}/{}/settings/pages", id.owner, id.name);
    if let Some(site) = gh.pages(&id.owner, &id.name).await? {
        return Ok(PagesState {
            enabled: true,
            settings_url,
            site_url: Some(site),
        });
    }
    let branch = gh
        .repo(&id.owner, &id.name)
        .await?
        .map(|r| r.default_branch)
        .ok_or(AppError::RepoNotFound)?;
    if gh.enable_pages(&id.owner, &id.name, &branch).await? {
        let site = gh.pages(&id.owner, &id.name).await?;
        return Ok(PagesState {
            enabled: true,
            settings_url,
            site_url: site,
        });
    }
    Ok(PagesState {
        enabled: false,
        settings_url,
        site_url: None,
    })
}

#[tauri::command]
pub async fn check_reservations(
    url: String,
    anon_key: String,
    state: State<'_, AppState>,
) -> AppResult<ReservationsCheck> {
    let base = normalize_supabase_url(&url)?;
    let Ok(key) = HeaderValue::from_str(anon_key.trim()) else {
        return Ok(ReservationsCheck::BadKey);
    };
    let resp = state
        .http()
        .get(format!("{base}/rest/v1/reservations?select=list&limit=1"))
        .header("apikey", key)
        .send()
        .await;
    let Ok(resp) = resp else {
        return Ok(ReservationsCheck::Unreachable);
    };
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or_default();
    Ok(classify_probe(status, body["code"].as_str().unwrap_or("")))
}

fn classify_probe(status: StatusCode, code: &str) -> ReservationsCheck {
    match (status, code) {
        (s, _) if s.is_success() => ReservationsCheck::Ready,
        (StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN, _) => ReservationsCheck::BadKey,
        // PostgREST: 42703 = column `list` missing, 42P01/PGRST205 = no such table
        (_, "42703") => ReservationsCheck::OldSchema,
        (_, "42P01" | "PGRST205") => ReservationsCheck::NoSchema,
        (StatusCode::NOT_FOUND, _) => ReservationsCheck::NoSchema,
        _ => ReservationsCheck::Unreachable,
    }
}

/// Accepts `https://ref.supabase.co` with or without a trailing `/rest/v1` or slash.
fn normalize_supabase_url(url: &str) -> AppResult<String> {
    let u = url.trim().trim_end_matches('/');
    let u = u
        .strip_suffix("/rest/v1")
        .unwrap_or(u)
        .trim_end_matches('/');
    let ok = u.starts_with("https://")
        && u.len() > "https://".len()
        && !u["https://".len()..].contains(['/', '?', '#', '@', ' ']);
    if ok {
        Ok(u.to_owned())
    } else {
        Err(AppError::GitHub(
            "адрес Supabase должен выглядеть как https://xxxx.supabase.co".into(),
        ))
    }
}

/// Returns the config with the owner's settings, or `None` when this list id is already set
/// (the settings commit happened in an earlier run).
fn configure(text: &str, list_id: &str, setup: &Setup) -> AppResult<Option<String>> {
    let mut cfg: Value = serde_json::from_str(text)
        .map_err(|e| AppError::GitHub(format!("wishlist.config.json не читается: {e}")))?;
    if cfg["schemaVersion"].as_u64() != Some(SCHEMA_VERSION) {
        return Err(AppError::ReadOnly);
    }
    if cfg["listId"].as_str() == Some(list_id) {
        return Ok(None);
    }
    let title = setup.title.trim();
    if title.is_empty() || title.chars().count() > 80 {
        return Err(AppError::GitHub("название — от 1 до 80 символов".into()));
    }
    let cur = setup.default_currency.trim();
    if cur.len() != 3 || !cur.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(AppError::GitHub(
            "код валюты — три латинские буквы, напр. EUR".into(),
        ));
    }
    let obj = cfg
        .as_object_mut()
        .expect("checked schemaVersion on an object");
    obj.insert("title".into(), json!(title));
    obj.insert("defaultCurrency".into(), json!(cur));
    obj.insert("listId".into(), json!(list_id));
    match &setup.reservations {
        Some(r) => {
            let url = normalize_supabase_url(&r.url)?;
            obj.insert(
                "reservations".into(),
                json!({ "url": url, "anonKey": r.anon_key.trim() }),
            );
        }
        None => {
            obj.remove("reservations");
        }
    }
    Ok(Some(
        serde_json::to_string_pretty(&cfg).expect("json value") + "\n",
    ))
}

fn apply_setup(ws: &Workspace, list_id: &str, setup: &Setup) -> AppResult<bool> {
    let Some(text) = configure(&ws.read_config()?, list_id, setup)? else {
        return Ok(false);
    };
    if !setup.keep_examples {
        for item in ws.list_items()? {
            ws.delete_item(&item.slug)?;
        }
    }
    ws.write_config(&text)?;
    Ok(true)
}

#[tauri::command]
pub async fn finish_setup(
    name: String,
    setup: Setup,
    state: State<'_, AppState>,
) -> AppResult<Finished> {
    let id = own_repo(&state, name)?;
    let session = state.session()?;
    open_or_clone(&state, id.clone()).await?;
    let list_id = format!("{}/{}", id.owner, id.name);
    let published = state
        .locked(move |ws| {
            apply_setup(ws, &list_id, &setup)?;
            // also pushes a settings commit left local by an earlier run that lost the network
            commit_and_push(ws, &session, "Настроить вишлист", &HashMap::new())
        })
        .await?;
    let sha = match published {
        Published::Pushed { sha } => Some(sha),
        Published::NothingToPublish => None,
        Published::Conflicts { .. } => {
            return Err(AppError::GitHub(
                "в новом вишлисте уже кто-то что-то поменял — откройте его из списка".into(),
            ));
        }
    };
    let site_url = state
        .github()?
        .pages(&id.owner, &id.name)
        .await?
        .unwrap_or_else(|| pages_url(&id.owner, &id.name));
    Ok(Finished { sha, site_url })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE_CONFIG: &str = "{\n  \"schemaVersion\": 1,\n  \"title\": \"Мой вишлист\",\n  \"defaultCurrency\": \"RUB\"\n}\n";

    fn setup() -> Setup {
        Setup {
            title: "  Вишлист Марины ".into(),
            default_currency: "EUR".into(),
            reservations: Some(Reservations {
                url: "https://abc.supabase.co/rest/v1/".into(),
                anon_key: " sb_publishable_x ".into(),
            }),
            keep_examples: false,
        }
    }

    #[test]
    fn writes_settings_once() {
        let out = configure(TEMPLATE_CONFIG, "marina/wishlist", &setup())
            .unwrap()
            .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["title"], "Вишлист Марины");
        assert_eq!(v["defaultCurrency"], "EUR");
        assert_eq!(v["listId"], "marina/wishlist");
        assert_eq!(v["reservations"]["url"], "https://abc.supabase.co");
        assert_eq!(v["reservations"]["anonKey"], "sb_publishable_x");
        assert_eq!(v["schemaVersion"], 1);
        assert!(out.ends_with("}\n"));
        assert_eq!(configure(&out, "marina/wishlist", &setup()).unwrap(), None);
    }

    #[test]
    fn rejects_bad_input() {
        let mut s = setup();
        s.title = "  ".into();
        assert!(configure(TEMPLATE_CONFIG, "a/b", &s).is_err());
        let mut s = setup();
        s.default_currency = "eur".into();
        assert!(configure(TEMPLATE_CONFIG, "a/b", &s).is_err());
        let mut s = setup();
        s.reservations.as_mut().unwrap().url = "http://abc.supabase.co".into();
        assert!(configure(TEMPLATE_CONFIG, "a/b", &s).is_err());
        assert!(matches!(
            configure(r#"{"schemaVersion":2}"#, "a/b", &setup()),
            Err(AppError::ReadOnly)
        ));
    }

    #[test]
    fn supabase_urls() {
        for ok in [
            "https://x.supabase.co",
            "https://x.supabase.co/",
            "https://x.supabase.co/rest/v1",
        ] {
            assert_eq!(
                normalize_supabase_url(ok).unwrap(),
                "https://x.supabase.co",
                "{ok}"
            );
        }
        for bad in [
            "x.supabase.co",
            "https://",
            "https://x.co/path",
            "https://user@x.co",
        ] {
            assert!(normalize_supabase_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn probe_outcomes() {
        assert_eq!(classify_probe(StatusCode::OK, ""), ReservationsCheck::Ready);
        assert_eq!(
            classify_probe(StatusCode::UNAUTHORIZED, ""),
            ReservationsCheck::BadKey
        );
        assert_eq!(
            classify_probe(StatusCode::BAD_REQUEST, "42703"),
            ReservationsCheck::OldSchema
        );
        assert_eq!(
            classify_probe(StatusCode::NOT_FOUND, "PGRST205"),
            ReservationsCheck::NoSchema
        );
        assert_eq!(
            classify_probe(StatusCode::BAD_GATEWAY, ""),
            ReservationsCheck::Unreachable
        );
    }

    #[test]
    fn examples_removed_and_config_written() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE), TEMPLATE_CONFIG).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        ws.write_item("example", "---\ntitle: x\n---\n").unwrap();
        assert!(apply_setup(&ws, "a/b", &setup()).unwrap());
        assert!(ws.list_items().unwrap().is_empty());
        assert!(!apply_setup(&ws, "a/b", &setup()).unwrap());
    }
}

//! The few GitHub REST and GraphQL calls the editor makes.

use reqwest::header::{ACCEPT, AUTHORIZATION};
use reqwest::{Client, Method, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{AppError, AppResult};

const API: &str = "https://api.github.com";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub login: String,
    pub id: u64,
    pub name: Option<String>,
    #[serde(alias = "avatar_url")]
    pub avatar_url: String,
}

impl User {
    /// Commit author: the profile name and GitHub's noreply address, so no real email leaks.
    pub fn signature(&self) -> AppResult<git2::Signature<'static>> {
        let name = self
            .name
            .as_deref()
            .filter(|n| !n.is_empty())
            .unwrap_or(&self.login);
        let email = format!("{}+{}@users.noreply.github.com", self.id, self.login);
        Ok(git2::Signature::now(name, &email)?)
    }
}

/// A repository of the signed-in user that holds a wishlist.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWishlist {
    pub owner: String,
    pub name: String,
    pub title: Option<String>,
    /// Namespace of its reservations; `owner/name` when the config does not set one.
    pub list_id: String,
    pub items: usize,
    pub site_url: String,
    pub updated_at: String,
}

/// The Actions run that deploys a pushed commit to Pages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeployRun {
    pub status: String,
    pub conclusion: Option<String>,
    #[serde(alias = "html_url")]
    pub html_url: String,
}

/// Returns the Pages URL GitHub assigns to `owner/name` without a custom domain.
pub fn pages_url(owner: &str, name: &str) -> String {
    let host = format!("{}.github.io", owner.to_lowercase());
    if name.eq_ignore_ascii_case(&host) {
        format!("https://{host}/")
    } else {
        format!("https://{host}/{name}/")
    }
}

#[derive(Clone)]
pub struct GitHub {
    http: Client,
    token: String,
}

impl GitHub {
    pub fn new(http: Client, token: String) -> Self {
        Self { http, token }
    }

    fn req(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{API}{path}"))
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    }

    async fn send<T: DeserializeOwned>(rb: RequestBuilder) -> AppResult<T> {
        let resp = rb.send().await?;
        let status = resp.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err(AppError::Unauthorized);
        }
        if !status.is_success() {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let msg = body["message"].as_str().unwrap_or("").to_owned();
            return Err(AppError::GitHub(
                format!("{status} {msg}").trim().to_owned(),
            ));
        }
        Ok(resp.json().await?)
    }

    pub async fn user(&self) -> AppResult<User> {
        Self::send(self.req(Method::GET, "/user")).await
    }

    /// Lists the user's repositories that contain `wishlist.config.json`, newest first.
    pub async fn wishlists(&self) -> AppResult<Vec<RemoteWishlist>> {
        // one GraphQL query reads the config and the items folder of 100 repos at once,
        // instead of a REST contents call per repository
        const QUERY: &str = r#"
          query($cursor: String) {
            viewer {
              repositories(first: 100, after: $cursor, ownerAffiliations: [OWNER],
                           orderBy: {field: UPDATED_AT, direction: DESC}) {
                pageInfo { hasNextPage endCursor }
                nodes {
                  name isArchived isTemplate updatedAt
                  owner { login }
                  config: object(expression: "HEAD:wishlist.config.json") { ... on Blob { text } }
                  items: object(expression: "HEAD:src/content/items") { ... on Tree { entries { name type } } }
                }
              }
            }
          }"#;
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        // 5 pages = 500 repositories; anyone with more can still type the name in
        for _ in 0..5 {
            let page: GqlResponse = Self::send(
                self.req(Method::POST, "/graphql")
                    .json(&json!({ "query": QUERY, "variables": { "cursor": cursor } })),
            )
            .await?;
            if let Some(err) = page.errors.and_then(|e| e.into_iter().next()) {
                return Err(AppError::GitHub(err.message));
            }
            let repos = page
                .data
                .ok_or_else(|| AppError::GitHub("empty GraphQL response".into()))?
                .viewer
                .repositories;
            out.extend(repos.nodes.into_iter().filter_map(RepoNode::into_wishlist));
            if !repos.page_info.has_next_page {
                break;
            }
            cursor = repos.page_info.end_cursor;
        }
        Ok(out)
    }

    /// Returns the newest Actions run for `sha`, if GitHub has queued one yet.
    pub async fn deploy_run(
        &self,
        owner: &str,
        name: &str,
        sha: &str,
    ) -> AppResult<Option<DeployRun>> {
        #[derive(Deserialize)]
        struct Runs {
            workflow_runs: Vec<DeployRun>,
        }
        let runs: Runs = Self::send(self.req(
            Method::GET,
            &format!("/repos/{owner}/{name}/actions/runs?head_sha={sha}&per_page=1"),
        ))
        .await?;
        Ok(runs.workflow_runs.into_iter().next())
    }

    /// Sends a request whose 404 means "absent", not an error.
    async fn optional<T: DeserializeOwned>(rb: RequestBuilder) -> AppResult<Option<T>> {
        let resp = rb.send().await?;
        match resp.status() {
            StatusCode::NOT_FOUND => Ok(None),
            StatusCode::UNAUTHORIZED => Err(AppError::Unauthorized),
            s if s.is_success() => Ok(Some(resp.json().await?)),
            s => {
                let body: serde_json::Value = resp.json().await.unwrap_or_default();
                let msg = body["message"].as_str().unwrap_or("");
                Err(AppError::GitHub(format!("{s} {msg}").trim().to_owned()))
            }
        }
    }

    pub async fn repo(&self, owner: &str, name: &str) -> AppResult<Option<RepoInfo>> {
        Self::optional(self.req(Method::GET, &format!("/repos/{owner}/{name}"))).await
    }

    /// Counts commits on the default branch, stopping at 2: all callers need is "only one?".
    pub async fn commits_up_to_two(&self, owner: &str, name: &str) -> AppResult<usize> {
        let list: Option<Vec<serde_json::Value>> = Self::optional(self.req(
            Method::GET,
            &format!("/repos/{owner}/{name}/commits?per_page=2"),
        ))
        .await?;
        Ok(list.map_or(0, |l| l.len()))
    }

    pub async fn has_file(&self, owner: &str, name: &str, path: &str) -> AppResult<bool> {
        let found: Option<serde_json::Value> = Self::optional(self.req(
            Method::GET,
            &format!("/repos/{owner}/{name}/contents/{path}"),
        ))
        .await?;
        Ok(found.is_some())
    }

    /// Creates `owner/name` as a public copy of `template`; the files arrive asynchronously.
    pub async fn generate(
        &self,
        template: (&str, &str),
        owner: &str,
        name: &str,
        description: &str,
    ) -> AppResult<()> {
        let _: serde_json::Value = Self::send(
            self.req(Method::POST, &format!("/repos/{}/{}/generate", template.0, template.1))
                .json(&json!({ "owner": owner, "name": name, "description": description, "private": false })),
        )
        .await?;
        Ok(())
    }

    /// Returns the site URL when Pages is enabled.
    pub async fn pages(&self, owner: &str, name: &str) -> AppResult<Option<String>> {
        #[derive(Deserialize)]
        struct Pages {
            html_url: Option<String>,
        }
        let p: Option<Pages> =
            Self::optional(self.req(Method::GET, &format!("/repos/{owner}/{name}/pages"))).await?;
        Ok(p.map(|p| p.html_url.unwrap_or_else(|| pages_url(owner, name))))
    }

    /// Deletes the repository; needs a token with the `delete_repo` scope.
    pub async fn delete_repo(&self, owner: &str, name: &str) -> AppResult<()> {
        let resp = self
            .req(Method::DELETE, &format!("/repos/{owner}/{name}"))
            .send()
            .await?;
        match resp.status() {
            s if s.is_success() || s == StatusCode::NOT_FOUND => Ok(()),
            StatusCode::UNAUTHORIZED => Err(AppError::Unauthorized),
            s => {
                let body: serde_json::Value = resp.json().await.unwrap_or_default();
                let msg = body["message"].as_str().unwrap_or("");
                Err(AppError::GitHub(
                    format!("удаление не удалось: {s} {msg}").trim().to_owned(),
                ))
            }
        }
    }

    /// Turns on Pages deployed from Actions; `false` when the token may not do it.
    pub async fn enable_pages(&self, owner: &str, name: &str, branch: &str) -> AppResult<bool> {
        let resp = self
            .req(Method::POST, &format!("/repos/{owner}/{name}/pages"))
            .json(&json!({ "build_type": "workflow", "source": { "branch": branch } }))
            .send()
            .await?;
        match resp.status() {
            // 409: already enabled
            s if s.is_success() || s == StatusCode::CONFLICT => Ok(true),
            StatusCode::UNAUTHORIZED => Err(AppError::Unauthorized),
            // the docs ask for the `repo` scope here; with `public_repo` GitHub may refuse
            StatusCode::FORBIDDEN | StatusCode::NOT_FOUND | StatusCode::UNPROCESSABLE_ENTITY => {
                Ok(false)
            }
            s => Err(AppError::GitHub(format!("enable Pages: {s}"))),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoInfo {
    pub default_branch: String,
    pub template_repository: Option<TemplateRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TemplateRef {
    pub full_name: String,
}

#[derive(Deserialize)]
struct GqlResponse {
    data: Option<GqlData>,
    errors: Option<Vec<GqlError>>,
}

#[derive(Deserialize)]
struct GqlError {
    message: String,
}

#[derive(Deserialize)]
struct GqlData {
    viewer: Viewer,
}

#[derive(Deserialize)]
struct Viewer {
    repositories: RepoPage,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepoPage {
    page_info: PageInfo,
    nodes: Vec<RepoNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepoNode {
    name: String,
    is_archived: bool,
    is_template: bool,
    updated_at: String,
    owner: Owner,
    config: Option<Blob>,
    items: Option<Tree>,
}

#[derive(Deserialize)]
struct Owner {
    login: String,
}

#[derive(Deserialize)]
struct Blob {
    text: Option<String>,
}

#[derive(Deserialize)]
struct Tree {
    entries: Vec<TreeEntry>,
}

#[derive(Deserialize)]
struct TreeEntry {
    name: String,
    #[serde(rename = "type")]
    kind: String,
}

impl RepoNode {
    fn into_wishlist(self) -> Option<RemoteWishlist> {
        // the template itself also has a config, but nobody keeps a wishlist in it
        if self.is_archived || self.is_template {
            return None;
        }
        let config = self.config?.text?;
        let parsed = serde_json::from_str::<serde_json::Value>(&config).ok();
        let title = parsed
            .as_ref()
            .and_then(|v| v["title"].as_str().map(str::to_owned));
        let list_id = parsed
            .as_ref()
            .and_then(|v| v["listId"].as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("{}/{}", self.owner.login, self.name));
        let items = self.items.map_or(0, |t| {
            t.entries
                .iter()
                .filter(|e| e.kind == "blob" && e.name.ends_with(".md"))
                .count()
        });
        Some(RemoteWishlist {
            site_url: pages_url(&self.owner.login, &self.name),
            owner: self.owner.login,
            name: self.name,
            title,
            list_id,
            items,
            updated_at: self.updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_urls() {
        assert_eq!(
            pages_url("Swchck", "wishlist"),
            "https://swchck.github.io/wishlist/"
        );
        assert_eq!(
            pages_url("swchck", "Swchck.github.io"),
            "https://swchck.github.io/"
        );
    }

    #[test]
    fn parses_graphql_page() {
        let raw = r#"{"data":{"viewer":{"repositories":{
          "pageInfo":{"hasNextPage":false,"endCursor":null},
          "nodes":[
            {"name":"wishlist","isArchived":false,"isTemplate":false,"updatedAt":"2026-10-12T09:00:00Z",
             "owner":{"login":"swchck"},
             "config":{"text":"{\"schemaVersion\":1,\"title\":\"Мой вишлист\"}"},
             "items":{"entries":[{"name":"a.md","type":"blob"},{"name":"images","type":"tree"},{"name":"b.md","type":"blob"}]}},
            {"name":"wishlist-template","isArchived":false,"isTemplate":true,"updatedAt":"x",
             "owner":{"login":"swchck"},"config":{"text":"{}"},"items":null},
            {"name":"dotfiles","isArchived":false,"isTemplate":false,"updatedAt":"x",
             "owner":{"login":"swchck"},"config":null,"items":null}
          ]}}}}"#;
        let page: GqlResponse = serde_json::from_str(raw).unwrap();
        let list: Vec<_> = page
            .data
            .unwrap()
            .viewer
            .repositories
            .nodes
            .into_iter()
            .filter_map(RepoNode::into_wishlist)
            .collect();
        assert_eq!(
            list,
            [RemoteWishlist {
                owner: "swchck".into(),
                name: "wishlist".into(),
                title: Some("Мой вишлист".into()),
                list_id: "swchck/wishlist".into(),
                items: 2,
                site_url: "https://swchck.github.io/wishlist/".into(),
                updated_at: "2026-10-12T09:00:00Z".into(),
            }]
        );
    }

    #[test]
    fn signature_uses_noreply_email() {
        let u = User {
            login: "swchck".into(),
            id: 42,
            name: None,
            avatar_url: String::new(),
        };
        let s = u.signature().unwrap();
        assert_eq!(s.name(), Ok("swchck"));
        assert_eq!(s.email(), Ok("42+swchck@users.noreply.github.com"));
    }
}

/// Hits the real API; run with `GITHUB_TOKEN=… cargo test -- --ignored live_`.
#[cfg(test)]
mod live {
    use super::*;

    fn client() -> Option<GitHub> {
        let token = std::env::var("GITHUB_TOKEN").ok()?;
        Some(GitHub::new(
            Client::builder()
                .user_agent("wishlist-editor-test")
                .build()
                .unwrap(),
            token,
        ))
    }

    #[test]
    #[ignore]
    fn live_lists_wishlists_and_clones_one() {
        let gh = client().expect("GITHUB_TOKEN");
        let (user, list) = tauri::async_runtime::block_on(async {
            (gh.user().await.unwrap(), gh.wishlists().await.unwrap())
        });
        println!("{user:?}\n{list:#?}");
        let first = list.first().expect("at least one wishlist");
        let dir = tempfile::tempdir().unwrap();
        let url = format!("https://github.com/{}/{}.git", first.owner, first.name);
        let repo = crate::git::clone(&url, &dir.path().join("w"), Some(&gh.token)).unwrap();
        crate::git::fetch(&repo, Some(&gh.token)).unwrap();
        assert!(dir.path().join("w/wishlist.config.json").is_file());
    }
}

//! Sync between the local clone and its GitHub remote.
//!
//! The remote branch is the source of truth and only ever moves forward: local work is
//! committed, replayed as one commit on top of the freshly fetched tip and pushed; a rejected
//! push means someone got there first, so the caller fetches and replays again. Nothing is
//! force-pushed. When the same file changed on both sides, the user picks a whole side per
//! file — the editor never merges card text line by line.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use git2::build::{CheckoutBuilder, RepoBuilder};
use git2::{
    Commit, Cred, ErrorClass, ErrorCode, FetchOptions, IndexAddOption, IndexEntry, Oid,
    PushOptions, RemoteCallbacks, Repository, Signature, Sort, Status, StatusOptions,
};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const REMOTE: &str = "origin";

/// Kind of a working-tree change relative to the last commit.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

/// A file changed since the last commit; `path` is repo-relative and `/`-separated.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub kind: ChangeKind,
}

/// How the local branch relates to the remote one as of the last fetch.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
pub struct Divergence {
    pub ahead: usize,
    pub behind: usize,
}

/// Outcome of bringing remote commits into the working copy on open.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Pull {
    UpToDate,
    FastForwarded,
    /// Remote changed files that are also edited locally; they meet at the next publish.
    Blocked,
    /// Local commits exist that the remote lacks (an earlier push failed).
    Diverged,
}

/// One file changed both locally and on the remote.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Conflict {
    pub path: String,
    /// `None` when that side deleted the file.
    pub local: Option<String>,
    pub remote: Option<String>,
}

/// Which version of a conflicting file wins.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Local,
    Remote,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Integrate {
    /// HEAD now contains the remote tip and can be pushed (or there is nothing to push).
    Ready,
    Conflicts(Vec<Conflict>),
}

fn callbacks(token: Option<&str>) -> RemoteCallbacks<'_> {
    let mut cb = RemoteCallbacks::new();
    let mut asked = false;
    cb.credentials(move |_url, _user, _allowed| {
        // libgit2 calls this again after a 401 and would loop forever on a revoked token
        if asked {
            return Err(git2::Error::new(
                ErrorCode::Auth,
                ErrorClass::Http,
                "token rejected",
            ));
        }
        asked = true;
        match token {
            Some(t) => Cred::userpass_plaintext("x-access-token", t),
            None => Cred::default(),
        }
    });
    cb
}

fn fetch_options(token: Option<&str>) -> FetchOptions<'_> {
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks(token));
    fo
}

/// Clones `url` into `dest`; a failed clone leaves no half-written directory behind.
pub fn clone(url: &str, dest: &Path, token: Option<&str>) -> AppResult<Repository> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    RepoBuilder::new()
        .fetch_options(fetch_options(token))
        .clone(url, dest)
        .map_err(|e| {
            let _ = fs::remove_dir_all(dest);
            AppError::from(e)
        })
}

fn branch_name(repo: &Repository) -> AppResult<String> {
    let head = repo.head()?;
    head.shorthand()
        .map(str::to_owned)
        .map_err(|_| AppError::Git("HEAD is not on a branch".into()))
}

fn head_commit(repo: &Repository) -> AppResult<Commit<'_>> {
    Ok(repo.head()?.peel_to_commit()?)
}

fn remote_commit(repo: &Repository) -> AppResult<Option<Commit<'_>>> {
    let name = format!("refs/remotes/{REMOTE}/{}", branch_name(repo)?);
    match repo.find_reference(&name) {
        Ok(r) => Ok(Some(r.peel_to_commit()?)),
        Err(e) if e.code() == ErrorCode::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Updates the remote-tracking ref of the current branch.
pub fn fetch(repo: &Repository, token: Option<&str>) -> AppResult<()> {
    let b = branch_name(repo)?;
    let mut remote = repo.find_remote(REMOTE)?;
    remote.fetch(
        &[format!("+refs/heads/{b}:refs/remotes/{REMOTE}/{b}")],
        Some(&mut fetch_options(token)),
        None,
    )?;
    Ok(())
}

pub fn divergence(repo: &Repository) -> AppResult<Divergence> {
    let Some(remote) = remote_commit(repo)? else {
        return Ok(Divergence::default());
    };
    let (ahead, behind) = repo.graph_ahead_behind(head_commit(repo)?.id(), remote.id())?;
    Ok(Divergence { ahead, behind })
}

/// Lists uncommitted changes, ignoring files the editor writes only transiently.
pub fn changes(repo: &Repository) -> AppResult<Vec<Change>> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false)
        .exclude_submodules(true);
    let mut out = Vec::new();
    for entry in repo.statuses(Some(&mut opts))?.iter() {
        let Ok(path) = entry.path() else { continue };
        let s = entry.status();
        let kind = if s.intersects(Status::WT_NEW | Status::INDEX_NEW) {
            ChangeKind::Added
        } else if s.intersects(Status::WT_DELETED | Status::INDEX_DELETED) {
            ChangeKind::Deleted
        } else if s.intersects(
            Status::WT_MODIFIED
                | Status::INDEX_MODIFIED
                | Status::WT_RENAMED
                | Status::INDEX_RENAMED
                | Status::WT_TYPECHANGE
                | Status::INDEX_TYPECHANGE,
        ) {
            ChangeKind::Modified
        } else {
            continue;
        };
        out.push(Change {
            path: path.to_owned(),
            kind,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Moves the branch to the fetched remote tip when that cannot touch local edits.
pub fn fast_forward(repo: &Repository) -> AppResult<Pull> {
    let Some(remote) = remote_commit(repo)? else {
        return Ok(Pull::UpToDate);
    };
    let head = head_commit(repo)?;
    let (ahead, behind) = repo.graph_ahead_behind(head.id(), remote.id())?;
    if behind == 0 {
        return Ok(Pull::UpToDate);
    }
    if ahead > 0 {
        return Ok(Pull::Diverged);
    }
    // safe mode refuses, before writing anything, to overwrite a locally edited file
    match repo.checkout_tree(remote.as_object(), Some(CheckoutBuilder::new().safe())) {
        Ok(()) => {}
        Err(e) if e.code() == ErrorCode::Conflict => return Ok(Pull::Blocked),
        Err(e) => return Err(e.into()),
    }
    move_branch(repo, remote.id(), "fast-forward")?;
    Ok(Pull::FastForwarded)
}

fn move_branch(repo: &Repository, to: Oid, why: &str) -> AppResult<()> {
    let name = format!("refs/heads/{}", branch_name(repo)?);
    repo.reference(&name, to, true, why)?;
    Ok(())
}

/// Commits every working-tree change; returns `None` when there is nothing to commit.
pub fn commit_all(repo: &Repository, message: &str, sig: &Signature) -> AppResult<Option<Oid>> {
    let mut index = repo.index()?;
    index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
    index.update_all(["*"], None)?;
    index.write()?;
    let tree_id = index.write_tree()?;
    let head = head_commit(repo)?;
    if head.tree_id() == tree_id {
        return Ok(None);
    }
    let tree = repo.find_tree(tree_id)?;
    Ok(Some(repo.commit(
        Some("HEAD"),
        sig,
        sig,
        message,
        &tree,
        &[&head],
    )?))
}

fn blob_text(repo: &Repository, entry: Option<&IndexEntry>) -> AppResult<Option<String>> {
    let Some(entry) = entry else { return Ok(None) };
    let blob = repo.find_blob(entry.id)?;
    if blob.is_binary() {
        return Ok(Some("[двоичный файл]".into()));
    }
    Ok(Some(String::from_utf8_lossy(blob.content()).into_owned()))
}

fn entry_path(e: &IndexEntry) -> String {
    String::from_utf8_lossy(&e.path).into_owned()
}

/// Puts local commits on top of the fetched remote tip as a single commit.
///
/// Must run on a clean working tree (call [`commit_all`] first). Conflicting files are
/// returned until `choices` names a side for every one of them; nothing is written then.
pub fn integrate(
    repo: &Repository,
    committer: &Signature,
    choices: &HashMap<String, Side>,
) -> AppResult<Integrate> {
    let Some(remote) = remote_commit(repo)? else {
        return Ok(Integrate::Ready);
    };
    let head = head_commit(repo)?;
    if head.id() == remote.id() || repo.graph_descendant_of(head.id(), remote.id())? {
        return Ok(Integrate::Ready);
    }
    if repo.graph_descendant_of(remote.id(), head.id())? {
        repo.checkout_tree(remote.as_object(), Some(CheckoutBuilder::new().force()))?;
        move_branch(repo, remote.id(), "fast-forward")?;
        return Ok(Integrate::Ready);
    }

    let base = repo.find_commit(repo.merge_base(head.id(), remote.id())?)?;
    // "ours" is the remote, "theirs" the local work being replayed onto it
    let mut index = repo.merge_trees(&base.tree()?, &remote.tree()?, &head.tree()?, None)?;

    if index.has_conflicts() {
        let mut found = Vec::new();
        for c in index.conflicts()? {
            let c = c?;
            let any = c
                .their
                .as_ref()
                .or(c.our.as_ref())
                .or(c.ancestor.as_ref())
                .expect("conflict has at least one side");
            found.push((entry_path(any), c.our, c.their));
        }
        if found.iter().any(|(p, ..)| !choices.contains_key(p)) {
            let mut out = Vec::with_capacity(found.len());
            for (path, ours, theirs) in &found {
                out.push(Conflict {
                    path: path.clone(),
                    local: blob_text(repo, theirs.as_ref())?,
                    remote: blob_text(repo, ours.as_ref())?,
                });
            }
            out.sort_by(|a, b| a.path.cmp(&b.path));
            return Ok(Integrate::Conflicts(out));
        }
        for (path, ours, theirs) in found {
            index.conflict_remove(Path::new(&path))?;
            let pick = match choices[&path] {
                Side::Local => theirs,
                Side::Remote => ours,
            };
            if let Some(mut entry) = pick {
                // conflict entries carry stage bits 1–3; a resolved entry must be stage 0
                entry.flags &= !0x3000;
                index.add(&entry)?;
            }
        }
    }

    let tree = repo.find_tree(index.write_tree_to(repo)?)?;
    let message = replayed_message(repo, head.id(), remote.id())?;
    let replayed = repo.commit(None, &head.author(), committer, &message, &tree, &[&remote])?;
    // files first, branch second: a crash in between leaves the remote's edits looking like
    // local ones, which replay onto the remote as no-ops instead of silently reverting it
    repo.checkout_tree(
        repo.find_commit(replayed)?.as_object(),
        Some(CheckoutBuilder::new().force()),
    )?;
    move_branch(repo, replayed, "replay local changes onto remote")?;
    Ok(Integrate::Ready)
}

fn replayed_message(repo: &Repository, head: Oid, remote: Oid) -> AppResult<String> {
    let mut walk = repo.revwalk()?;
    walk.push(head)?;
    walk.hide(remote)?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
    let mut messages = Vec::new();
    for oid in walk {
        if let Ok(m) = repo.find_commit(oid?)?.message() {
            messages.push(m.trim().to_owned());
        }
    }
    Ok(match messages.len() {
        0 => "Обновить вишлист".to_owned(),
        1 => messages.remove(0),
        _ => messages.join("\n\n"),
    })
}

/// Pushes the current branch; a non-fast-forward rejection becomes [`AppError::PushRejected`].
pub fn push(repo: &Repository, token: Option<&str>) -> AppResult<Oid> {
    let b = branch_name(repo)?;
    let head = head_commit(repo)?.id();
    let mut rejected: Option<String> = None;
    {
        let mut cb = callbacks(token);
        cb.push_update_reference(|_refname, status| {
            if let Some(s) = status {
                rejected = Some(s.to_owned());
            }
            Ok(())
        });
        let mut opts = PushOptions::new();
        opts.remote_callbacks(cb);
        let mut remote = repo.find_remote(REMOTE)?;
        remote
            .push(&[format!("refs/heads/{b}:refs/heads/{b}")], Some(&mut opts))
            .map_err(|e| {
                if e.code() == ErrorCode::NotFastForward
                    || e.message().contains("non-fast")
                    || e.message().contains("fastforward")
                {
                    AppError::PushRejected
                } else {
                    AppError::from(e)
                }
            })?;
    }
    if let Some(reason) = rejected {
        return Err(
            if reason.contains("fast") || reason.contains("fetch first") {
                AppError::PushRejected
            } else {
                AppError::GitHub(reason)
            },
        );
    }
    repo.reference(&format!("refs/remotes/{REMOTE}/{b}"), head, true, "push")?;
    Ok(head)
}

/// Reverts one uncommitted change; only paths reported by [`changes`] are accepted.
pub fn discard(repo: &Repository, path: &str) -> AppResult<()> {
    let change = changes(repo)?
        .into_iter()
        .find(|c| c.path == path)
        .ok_or_else(|| AppError::NotFound(path.to_owned()))?;
    if change.kind == ChangeKind::Added {
        let workdir = repo
            .workdir()
            .ok_or_else(|| AppError::Git("bare repository".into()))?;
        fs::remove_file(workdir.join(path))?;
        return Ok(());
    }
    repo.checkout_head(Some(CheckoutBuilder::new().force().path(path)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Fixture {
        _dir: tempfile::TempDir,
        bare: PathBuf,
        root: PathBuf,
    }

    fn sig() -> Signature<'static> {
        Signature::now("Tester", "tester@example.com").unwrap()
    }

    fn write(repo: &Repository, path: &str, text: &str) {
        let p = repo.workdir().unwrap().join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    fn read(repo: &Repository, path: &str) -> Option<String> {
        fs::read_to_string(repo.workdir().unwrap().join(path)).ok()
    }

    fn setup() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let bare = dir.path().join("remote.git");
        Repository::init_bare(&bare).unwrap();
        let root = dir.path().to_path_buf();
        let seed = clone(bare.to_str().unwrap(), &root.join("seed"), None).unwrap();
        write(&seed, "wishlist.config.json", "{}\n");
        write(&seed, "src/content/items/a.md", "A0\n");
        write(&seed, "src/content/items/b.md", "B0\n");
        let tree = {
            let mut i = seed.index().unwrap();
            i.add_all(["*"], IndexAddOption::DEFAULT, None).unwrap();
            i.write().unwrap();
            seed.find_tree(i.write_tree().unwrap()).unwrap()
        };
        seed.commit(Some("HEAD"), &sig(), &sig(), "init", &tree, &[])
            .unwrap();
        let b = branch_name(&seed).unwrap();
        seed.find_remote(REMOTE)
            .unwrap()
            .push(&[format!("refs/heads/{b}:refs/heads/{b}")], None)
            .unwrap();
        Fixture {
            _dir: dir,
            bare,
            root,
        }
    }

    impl Fixture {
        fn clone(&self, name: &str) -> Repository {
            clone(self.bare.to_str().unwrap(), &self.root.join(name), None).unwrap()
        }
    }

    /// Simulates an edit made elsewhere (another computer or github.com).
    fn push_elsewhere(fx: &Fixture, path: &str, text: &str) {
        let other = fx.clone(&format!("other-{}", rand_suffix()));
        write(&other, path, text);
        commit_all(&other, "elsewhere", &sig()).unwrap();
        push(&other, None).unwrap();
    }

    fn rand_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    fn publish(repo: &Repository, choices: &HashMap<String, Side>) -> Integrate {
        commit_all(repo, "local", &sig()).unwrap();
        fetch(repo, None).unwrap();
        let r = integrate(repo, &sig(), choices).unwrap();
        if r == Integrate::Ready {
            push(repo, None).unwrap();
        }
        r
    }

    #[test]
    fn reports_changes() {
        let fx = setup();
        let app = fx.clone("app");
        write(&app, "src/content/items/a.md", "A1\n");
        write(&app, "src/content/items/c.md", "C\n");
        fs::remove_file(app.workdir().unwrap().join("src/content/items/b.md")).unwrap();
        let kinds: Vec<_> = changes(&app)
            .unwrap()
            .into_iter()
            .map(|c| (c.path, c.kind))
            .collect();
        assert_eq!(
            kinds,
            [
                ("src/content/items/a.md".into(), ChangeKind::Modified),
                ("src/content/items/b.md".into(), ChangeKind::Deleted),
                ("src/content/items/c.md".into(), ChangeKind::Added),
            ]
        );
    }

    #[test]
    fn fast_forwards_around_unrelated_edits() {
        let fx = setup();
        let app = fx.clone("app");
        write(&app, "src/content/items/a.md", "A-local\n");
        push_elsewhere(&fx, "src/content/items/b.md", "B-remote\n");
        fetch(&app, None).unwrap();
        assert_eq!(
            divergence(&app).unwrap(),
            Divergence {
                ahead: 0,
                behind: 1
            }
        );
        assert_eq!(fast_forward(&app).unwrap(), Pull::FastForwarded);
        assert_eq!(read(&app, "src/content/items/b.md").unwrap(), "B-remote\n");
        assert_eq!(read(&app, "src/content/items/a.md").unwrap(), "A-local\n");
        assert_eq!(fast_forward(&app).unwrap(), Pull::UpToDate);
    }

    #[test]
    fn refuses_to_fast_forward_over_a_local_edit() {
        let fx = setup();
        let app = fx.clone("app");
        write(&app, "src/content/items/a.md", "A-local\n");
        push_elsewhere(&fx, "src/content/items/a.md", "A-remote\n");
        fetch(&app, None).unwrap();
        assert_eq!(fast_forward(&app).unwrap(), Pull::Blocked);
        assert_eq!(read(&app, "src/content/items/a.md").unwrap(), "A-local\n");
    }

    #[test]
    fn publishes_on_top_of_remote_changes_linearly() {
        let fx = setup();
        let app = fx.clone("app");
        write(&app, "src/content/items/a.md", "A-local\n");
        push_elsewhere(&fx, "src/content/items/b.md", "B-remote\n");
        assert_eq!(publish(&app, &HashMap::new()), Integrate::Ready);

        let check = fx.clone("check");
        assert_eq!(read(&check, "src/content/items/a.md").unwrap(), "A-local\n");
        assert_eq!(
            read(&check, "src/content/items/b.md").unwrap(),
            "B-remote\n"
        );
        let head = head_commit(&check).unwrap();
        assert_eq!(
            head.parent_count(),
            1,
            "replay must not create a merge commit"
        );
        assert_eq!(head.message().unwrap(), "local");
        assert!(changes(&app).unwrap().is_empty());
        assert_eq!(divergence(&app).unwrap(), Divergence::default());
    }

    #[test]
    fn conflict_waits_for_a_choice_then_applies_it() {
        for (side, expected) in [(Side::Local, "A-local\n"), (Side::Remote, "A-remote\n")] {
            let fx = setup();
            let app = fx.clone("app");
            write(&app, "src/content/items/a.md", "A-local\n");
            write(&app, "src/content/items/c.md", "C\n");
            push_elsewhere(&fx, "src/content/items/a.md", "A-remote\n");

            let Integrate::Conflicts(found) = publish(&app, &HashMap::new()) else {
                panic!("expected a conflict");
            };
            assert_eq!(
                found,
                [Conflict {
                    path: "src/content/items/a.md".into(),
                    local: Some("A-local\n".into()),
                    remote: Some("A-remote\n".into()),
                }]
            );

            let choices = HashMap::from([("src/content/items/a.md".to_owned(), side)]);
            assert_eq!(integrate(&app, &sig(), &choices).unwrap(), Integrate::Ready);
            push(&app, None).unwrap();
            let check = fx.clone("check");
            assert_eq!(read(&check, "src/content/items/a.md").unwrap(), expected);
            assert_eq!(read(&check, "src/content/items/c.md").unwrap(), "C\n");
        }
    }

    #[test]
    fn delete_versus_edit_conflict() {
        let fx = setup();
        let app = fx.clone("app");
        fs::remove_file(app.workdir().unwrap().join("src/content/items/a.md")).unwrap();
        push_elsewhere(&fx, "src/content/items/a.md", "A-remote\n");
        let Integrate::Conflicts(found) = publish(&app, &HashMap::new()) else {
            panic!("expected a conflict");
        };
        assert_eq!(found[0].local, None);
        let choices = HashMap::from([("src/content/items/a.md".to_owned(), Side::Local)]);
        assert_eq!(integrate(&app, &sig(), &choices).unwrap(), Integrate::Ready);
        push(&app, None).unwrap();
        assert_eq!(read(&fx.clone("check"), "src/content/items/a.md"), None);
    }

    #[test]
    fn push_is_rejected_when_remote_moved_after_fetch() {
        let fx = setup();
        let app = fx.clone("app");
        write(&app, "src/content/items/a.md", "A-local\n");
        commit_all(&app, "local", &sig()).unwrap();
        fetch(&app, None).unwrap();
        assert_eq!(
            integrate(&app, &sig(), &HashMap::new()).unwrap(),
            Integrate::Ready
        );
        push_elsewhere(&fx, "src/content/items/b.md", "B-remote\n");
        assert!(matches!(push(&app, None), Err(AppError::PushRejected)));

        // the retry the caller performs
        fetch(&app, None).unwrap();
        assert_eq!(
            integrate(&app, &sig(), &HashMap::new()).unwrap(),
            Integrate::Ready
        );
        push(&app, None).unwrap();
        let check = fx.clone("check");
        assert_eq!(read(&check, "src/content/items/a.md").unwrap(), "A-local\n");
        assert_eq!(
            read(&check, "src/content/items/b.md").unwrap(),
            "B-remote\n"
        );
    }

    #[test]
    fn nothing_to_commit() {
        let fx = setup();
        let app = fx.clone("app");
        assert_eq!(commit_all(&app, "x", &sig()).unwrap(), None);
    }

    #[test]
    fn discards_one_change() {
        let fx = setup();
        let app = fx.clone("app");
        write(&app, "src/content/items/a.md", "A1\n");
        write(&app, "src/content/items/c.md", "C\n");
        discard(&app, "src/content/items/a.md").unwrap();
        discard(&app, "src/content/items/c.md").unwrap();
        assert_eq!(read(&app, "src/content/items/a.md").unwrap(), "A0\n");
        assert_eq!(read(&app, "src/content/items/c.md"), None);
        assert!(matches!(
            discard(&app, "../../etc/passwd"),
            Err(AppError::NotFound(_))
        ));
    }
}

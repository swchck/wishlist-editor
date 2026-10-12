//! On-disk layout of a wishlist repository (schemaVersion 1) and safe access to its cards.
//!
//! The webview never passes paths for card operations, only slugs; every path is built
//! here from a validated slug. The repository content is untrusted (anyone with push
//! access, or a stranger's repo opened by name), so no path the editor reads or writes
//! may pass through a symlink: a planted link would otherwise turn an autosave into a
//! write anywhere in the user's home directory.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{AppError, AppResult};

pub const CONFIG_FILE: &str = "wishlist.config.json";
pub const ITEMS_DIR: &str = "src/content/items";
/// Item format this build reads and writes; must match `SCHEMA_VERSION` in src/lib/wishlist/schema.ts.
pub const SCHEMA_VERSION: u64 = 1;

const MAX_SLUG_LEN: usize = 80;

/// A card file as stored on disk.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RawItem {
    pub slug: String,
    pub text: String,
}

/// Everything the editor needs right after opening a wishlist.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub root: String,
    pub config_text: String,
    pub items: Vec<RawItem>,
    /// Non-card files under the items folder, relative to it, `/`-separated.
    pub files: Vec<String>,
    /// False when the repo uses another `schemaVersion`; every write is then refused.
    pub writable: bool,
}

/// Reports whether `slug` is a valid card file name: lowercase latin, digits and inner dashes.
pub fn is_valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= MAX_SLUG_LEN
        && !slug.starts_with('-')
        && !slug.ends_with('-')
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// An opened wishlist repository rooted at a directory.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
    writable: bool,
}

impl Workspace {
    /// Opens `root` if it contains a wishlist config file.
    pub fn open(root: impl Into<PathBuf>) -> AppResult<Self> {
        let root = root.into();
        let config = root.join(CONFIG_FILE);
        if is_symlink(&root) || is_symlink(&config) {
            return Err(AppError::UnsafeRepo(CONFIG_FILE.into()));
        }
        if !config.is_file() {
            return Err(AppError::NotAWishlist);
        }
        let version = serde_json::from_slice::<serde_json::Value>(&fs::read(&config)?)
            .ok()
            .and_then(|v| v["schemaVersion"].as_u64());
        Ok(Self {
            root,
            writable: version == Some(SCHEMA_VERSION),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ensure_writable(&self) -> AppResult<()> {
        if self.writable {
            Ok(())
        } else {
            Err(AppError::ReadOnly)
        }
    }

    /// Returns `rel` under the root after checking that no component on the way is a symlink.
    fn safe_path(&self, rel: &str) -> AppResult<PathBuf> {
        let mut path = self.root.clone();
        for part in rel.split('/') {
            path.push(part);
            if is_symlink(&path) {
                return Err(AppError::UnsafeRepo(rel.to_owned()));
            }
        }
        Ok(path)
    }

    fn items_dir(&self) -> AppResult<PathBuf> {
        self.safe_path(ITEMS_DIR)
    }

    fn item_path(&self, slug: &str) -> AppResult<PathBuf> {
        if !is_valid_slug(slug) {
            return Err(AppError::InvalidSlug(slug.to_owned()));
        }
        self.safe_path(&format!("{ITEMS_DIR}/{slug}.md"))
    }

    /// Reads the config and every card, sorted by slug.
    pub fn snapshot(&self) -> AppResult<Snapshot> {
        Ok(Snapshot {
            root: self.root.to_string_lossy().into_owned(),
            config_text: fs::read_to_string(self.safe_path(CONFIG_FILE)?)?,
            items: self.list_items()?,
            files: self.list_files()?,
            writable: self.writable,
        })
    }

    /// Returns every non-`.md` file below the items folder (e.g. `images/op-1.webp`),
    /// so the editor can tell whether a card's local photo exists before the site build does.
    pub fn list_files(&self) -> AppResult<Vec<String>> {
        let dir = self.items_dir()?;
        let mut out = Vec::new();
        if dir.is_dir() {
            collect_files(&dir, &dir, &mut out)?;
        }
        out.sort();
        Ok(out)
    }

    /// Returns every `<slug>.md` directly inside the items folder, sorted by slug.
    pub fn list_items(&self) -> AppResult<Vec<RawItem>> {
        let dir = self.items_dir()?;
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut items = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            // file_type() does not follow links, so a linked card is skipped, not read
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            let Some(slug) = path
                .extension()
                .filter(|ext| *ext == "md")
                .and_then(|_| path.file_stem())
                .and_then(|s| s.to_str())
            else {
                continue;
            };
            // hand-made files with odd names stay on disk untouched rather than
            // being listed under a slug the editor could never write back
            if !is_valid_slug(slug) {
                continue;
            }
            items.push(RawItem {
                slug: slug.to_owned(),
                text: fs::read_to_string(&path)?,
            });
        }
        items.sort_by(|a, b| a.slug.cmp(&b.slug));
        Ok(items)
    }

    /// Overwrites the card, creating it if needed.
    pub fn write_item(&self, slug: &str, text: &str) -> AppResult<()> {
        self.ensure_writable()?;
        let path = self.item_path(slug)?;
        fs::create_dir_all(self.items_dir()?)?;
        self.write_atomic(slug, &path, text.as_bytes())
    }

    /// Creates a card and fails if one with this slug already exists.
    pub fn create_item(&self, slug: &str, text: &str) -> AppResult<()> {
        if fs::symlink_metadata(self.item_path(slug)?).is_ok() {
            return Err(AppError::AlreadyExists(slug.to_owned()));
        }
        self.write_item(slug, text)
    }

    pub fn delete_item(&self, slug: &str) -> AppResult<()> {
        self.ensure_writable()?;
        let path = self.item_path(slug)?;
        if !path.exists() {
            return Err(AppError::NotFound(slug.to_owned()));
        }
        Ok(fs::remove_file(path)?)
    }

    /// Renames a card file, refusing to overwrite another card.
    pub fn rename_item(&self, from: &str, to: &str) -> AppResult<()> {
        self.ensure_writable()?;
        let src = self.item_path(from)?;
        let dst = self.item_path(to)?;
        if !src.exists() {
            return Err(AppError::NotFound(from.to_owned()));
        }
        if fs::symlink_metadata(&dst).is_ok() {
            return Err(AppError::AlreadyExists(to.to_owned()));
        }
        Ok(fs::rename(src, dst)?)
    }

    /// Where half-written files live: inside `.git`, so git never sees or commits them
    /// and nothing in the (untrusted) work tree can occupy the name first.
    fn tmp_dir(&self) -> AppResult<PathBuf> {
        let git = self.root.join(".git");
        if git.is_dir() && !is_symlink(&git) {
            Ok(git)
        } else {
            self.items_dir()
        }
    }

    /// Writes through a temp file and rename, so a crash never leaves a half-written card
    /// that would break the site build.
    fn write_atomic(&self, slug: &str, path: &Path, bytes: &[u8]) -> AppResult<()> {
        let tmp = self.tmp_dir()?.join(format!("wishlist-editor-{slug}.tmp"));
        if fs::symlink_metadata(&tmp).is_ok() {
            fs::remove_file(&tmp)?;
        }
        {
            // create_new is O_CREAT|O_EXCL: it fails on any existing entry, links included
            let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        // rename replaces a link at `path` itself, never what it points to
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

fn collect_files(base: &Path, dir: &Path, out: &mut Vec<String>) -> AppResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_files(base, &path, out)?;
        } else if kind.is_file() && path.extension().is_none_or(|ext| ext != "md") {
            let rel = path.strip_prefix(base).expect("walk stays under base");
            out.push(
                rel.components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/"),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wishlist() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(CONFIG_FILE),
            r#"{"schemaVersion":1,"title":"t"}"#,
        )
        .unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        (dir, ws)
    }

    #[test]
    fn slug_rules() {
        for ok in ["playdate", "op-1", "a", "brk-ulje-za-bradu"] {
            assert!(is_valid_slug(ok), "{ok}");
        }
        for bad in [
            "",
            "-a",
            "a-",
            "A",
            "a b",
            "../x",
            "a/b",
            "a.md",
            "плейдейт",
            &"a".repeat(81),
        ] {
            assert!(!is_valid_slug(bad), "{bad}");
        }
    }

    #[test]
    fn open_requires_config() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            Workspace::open(dir.path()),
            Err(AppError::NotAWishlist)
        ));
    }

    #[test]
    fn write_list_rename_delete() {
        let (_d, ws) = wishlist();
        assert!(ws.list_items().unwrap().is_empty());

        ws.create_item("b", "B").unwrap();
        ws.write_item("a", "A").unwrap();
        let slugs: Vec<_> = ws
            .list_items()
            .unwrap()
            .into_iter()
            .map(|i| i.slug)
            .collect();
        assert_eq!(slugs, ["a", "b"]);

        assert!(matches!(
            ws.create_item("a", "x"),
            Err(AppError::AlreadyExists(_))
        ));
        assert!(matches!(
            ws.rename_item("a", "b"),
            Err(AppError::AlreadyExists(_))
        ));

        ws.rename_item("a", "c").unwrap();
        ws.delete_item("b").unwrap();
        assert_eq!(
            ws.list_items().unwrap(),
            [RawItem {
                slug: "c".into(),
                text: "A".into()
            }]
        );
        assert!(fs::read_dir(ws.root().join(ITEMS_DIR)).unwrap().count() == 1);
    }

    #[test]
    fn rejects_traversal() {
        let (_d, ws) = wishlist();
        assert!(matches!(
            ws.write_item("../../evil", "x"),
            Err(AppError::InvalidSlug(_))
        ));
        assert!(matches!(
            ws.delete_item("../wishlist.config"),
            Err(AppError::InvalidSlug(_))
        ));
    }

    #[test]
    fn skips_unlisted_files() {
        let (_d, ws) = wishlist();
        let dir = ws.root().join(ITEMS_DIR);
        fs::create_dir_all(dir.join("images")).unwrap();
        fs::write(dir.join("Odd Name.md"), "x").unwrap();
        fs::write(dir.join("notes.txt"), "x").unwrap();
        fs::write(dir.join("images/pic.md"), "x").unwrap();
        ws.write_item("ok", "y").unwrap();
        let slugs: Vec<_> = ws
            .list_items()
            .unwrap()
            .into_iter()
            .map(|i| i.slug)
            .collect();
        assert_eq!(slugs, ["ok"]);
        fs::write(dir.join(".DS_Store"), "x").unwrap();
        assert_eq!(ws.list_files().unwrap(), ["notes.txt"]);
        fs::write(dir.join("images/op-1.webp"), "x").unwrap();
        assert_eq!(ws.list_files().unwrap(), ["images/op-1.webp", "notes.txt"]);
    }

    #[cfg(unix)]
    #[test]
    fn planted_symlinks_cannot_redirect_writes() {
        use std::os::unix::fs::symlink;
        let (dir, ws) = wishlist();
        let victim = dir.path().join("victim.txt");
        fs::write(&victim, "precious").unwrap();
        let items = dir.path().join(ITEMS_DIR);
        fs::create_dir_all(&items).unwrap();

        // the attack from the security review: a link where the temp file would go
        symlink(&victim, items.join("gift.md.tmp")).unwrap();
        symlink(&victim, items.join("wishlist-editor-gift.tmp")).unwrap();
        ws.write_item("gift", "attacker text").unwrap();
        assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");

        // a linked card is neither listed nor written through
        symlink(&victim, items.join("linked.md")).unwrap();
        assert!(ws.list_items().unwrap().iter().all(|i| i.slug != "linked"));
        assert!(matches!(
            ws.write_item("linked", "x"),
            Err(AppError::UnsafeRepo(_))
        ));
        assert!(matches!(
            ws.delete_item("linked"),
            Err(AppError::UnsafeRepo(_))
        ));
        assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
    }

    #[cfg(unix)]
    #[test]
    fn linked_items_folder_is_refused() {
        use std::os::unix::fs::symlink;
        let (dir, ws) = wishlist();
        let elsewhere = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/content")).unwrap();
        symlink(elsewhere.path(), dir.path().join(ITEMS_DIR)).unwrap();
        assert!(matches!(ws.list_items(), Err(AppError::UnsafeRepo(_))));
        assert!(matches!(
            ws.write_item("a", "x"),
            Err(AppError::UnsafeRepo(_))
        ));
        assert_eq!(fs::read_dir(elsewhere.path()).unwrap().count(), 0);
    }

    #[test]
    fn other_schema_version_is_read_only() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(CONFIG_FILE), r#"{"schemaVersion":2}"#).unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        assert!(!ws.snapshot().unwrap().writable);
        assert!(matches!(ws.write_item("a", "x"), Err(AppError::ReadOnly)));
    }
}

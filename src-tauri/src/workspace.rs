//! On-disk layout of a wishlist repository (schemaVersion 1) and safe access to its cards.
//!
//! The webview never passes paths for card operations, only slugs; every path is
//! built here from a validated slug, so a card write cannot escape the items folder.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{AppError, AppResult};

pub const CONFIG_FILE: &str = "wishlist.config.json";
pub const ITEMS_DIR: &str = "src/content/items";

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

/// An opened wishlist repository rooted at a directory.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// Opens `root` if it contains a wishlist config file.
    pub fn open(root: impl Into<PathBuf>) -> AppResult<Self> {
        let root = root.into();
        if !root.join(CONFIG_FILE).is_file() {
            return Err(AppError::NotAWishlist);
        }
        Ok(Self { root })
    }

    #[cfg(test)]
    fn root(&self) -> &Path {
        &self.root
    }

    fn items_dir(&self) -> PathBuf {
        self.root.join(ITEMS_DIR)
    }

    fn item_path(&self, slug: &str) -> AppResult<PathBuf> {
        if !is_valid_slug(slug) {
            return Err(AppError::InvalidSlug(slug.to_owned()));
        }
        Ok(self.items_dir().join(format!("{slug}.md")))
    }

    /// Reads the config and every card, sorted by slug.
    pub fn snapshot(&self) -> AppResult<Snapshot> {
        Ok(Snapshot {
            root: self.root.to_string_lossy().into_owned(),
            config_text: fs::read_to_string(self.root.join(CONFIG_FILE))?,
            items: self.list_items()?,
            files: self.list_files()?,
        })
    }

    /// Returns every non-`.md` file below the items folder (e.g. `images/op-1.webp`),
    /// so the editor can tell whether a card's local photo exists before the site build does.
    pub fn list_files(&self) -> AppResult<Vec<String>> {
        let dir = self.items_dir();
        let mut out = Vec::new();
        if dir.is_dir() {
            collect_files(&dir, &dir, &mut out)?;
        }
        out.sort();
        Ok(out)
    }

    /// Returns every `<slug>.md` directly inside the items folder, sorted by slug.
    pub fn list_items(&self) -> AppResult<Vec<RawItem>> {
        let dir = self.items_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut items = Vec::new();
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
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
        let path = self.item_path(slug)?;
        fs::create_dir_all(self.items_dir())?;
        write_atomic(&path, text.as_bytes())
    }

    /// Creates a card and fails if one with this slug already exists.
    pub fn create_item(&self, slug: &str, text: &str) -> AppResult<()> {
        if self.item_path(slug)?.exists() {
            return Err(AppError::AlreadyExists(slug.to_owned()));
        }
        self.write_item(slug, text)
    }

    pub fn delete_item(&self, slug: &str) -> AppResult<()> {
        let path = self.item_path(slug)?;
        if !path.exists() {
            return Err(AppError::NotFound(slug.to_owned()));
        }
        Ok(fs::remove_file(path)?)
    }

    /// Renames a card file, refusing to overwrite another card.
    pub fn rename_item(&self, from: &str, to: &str) -> AppResult<()> {
        let src = self.item_path(from)?;
        let dst = self.item_path(to)?;
        if !src.exists() {
            return Err(AppError::NotFound(from.to_owned()));
        }
        if dst.exists() {
            return Err(AppError::AlreadyExists(to.to_owned()));
        }
        Ok(fs::rename(src, dst)?)
    }
}

fn collect_files(base: &Path, dir: &Path, out: &mut Vec<String>) -> AppResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if entry.file_type()?.is_dir() {
            collect_files(base, &path, out)?;
        } else if path.extension().is_none_or(|ext| ext != "md") {
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

/// Writes via a sibling temp file and rename, so a crash never leaves a half-written card
/// that would break the site build.
fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let tmp = path.with_extension("md.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
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
        assert!(!ws.root().join(ITEMS_DIR).join("c.md.tmp").exists());
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
}

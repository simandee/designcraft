//! `cargo xtask assets`: every non-code asset must be attributed in `ASSETS.md`.
//!
//! Scans all git-tracked (and untracked, not ignored) files with an asset extension, plus
//! everything under `assets/`, `docs/images/` and `examples/`, and fails if a path is not listed
//! as `` `path` `` in ASSETS.md. See the asset policy in AGENTS.md.

use std::path::Path;
use std::process::Command;

const ASSET_EXT: &[&str] = &[
    "png",
    "jpg",
    "jpeg",
    "gif",
    "webp",
    "bmp",
    "tif",
    "tiff",
    "ico",
    "icns",
    "svg",
    "pdf",
    "ai",
    "eps",
    "psd",
    "icc",
    "icm",
    "ttf",
    "otf",
    "woff",
    "woff2",
    "ase",
    "indd",
    "idml",
    "aco",
    "abr",
    "designcraft",
    "mp4",
    "wav",
    "mp3",
];
const ASSET_DIRS: &[&str] = &["assets/", "docs/images/", "examples/"];

pub fn is_asset(path: &str) -> bool {
    let ext = Path::new(path).extension().and_then(std::ffi::OsStr::to_str).map(str::to_ascii_lowercase).unwrap_or_default();
    ASSET_DIRS.iter().any(|d| path.starts_with(d)) || ASSET_EXT.contains(&ext.as_str())
}

/// Paths that need an ASSETS.md entry but lack one. A table row whose first cell is a directory
/// (`` | `dir/` | … ``) covers every file below it, for large sets under one licence.
pub fn missing(files: &[String], assets_md: &str) -> Vec<String> {
    let dirs: Vec<&str> =
        assets_md.lines().filter_map(|l| l.strip_prefix("| `")?.split_once('`').map(|(p, _)| p)).filter(|p| p.ends_with('/')).collect();
    files.iter().filter(|f| is_asset(f) && !assets_md.contains(&format!("`{f}`")) && !dirs.iter().any(|d| f.starts_with(d))).cloned().collect()
}

pub fn run(root: &Path) -> Result<(), String> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .output()
        .map_err(|e| format!("git ls-files: {e}"))?;
    let files: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().filter(|l| root.join(l).exists()).map(str::to_owned).collect();
    let md = std::fs::read_to_string(root.join("ASSETS.md")).map_err(|e| format!("ASSETS.md: {e}"))?;
    let miss = missing(&files, &md);
    if miss.is_empty() {
        println!("assets: all {} asset files attributed in ASSETS.md", files.iter().filter(|f| is_asset(f)).count());
        Ok(())
    } else {
        Err(format!("{} asset file(s) lack an ASSETS.md entry (author, source, licence):\n  {}", miss.len(), miss.join("\n  ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory row covers the files below it, and only those; a directory named in prose does not.
    #[test]
    fn a_directory_row_covers_the_files_below_it() {
        let files = vec!["emoji/svg/1F600.svg".to_string(), "emoji/svg/1F601.svg".into(), "emojis.svg".into(), "docs/brand/logo.svg".into()];
        let md = "Brand files live in `docs/brand/`.\n| `emoji/` | OpenMoji | openmoji.org | CC BY-SA 4.0 |";
        assert_eq!(missing(&files, md), vec!["emojis.svg".to_string(), "docs/brand/logo.svg".into()]);
    }

    #[test]
    fn detects_unattributed_assets() {
        let files = vec!["assets/icons/a.svg".to_string(), "crates/x/src/lib.rs".into(), "docs/images/b.png".into(), "tests/fixture.png".into()];
        let md = "| `assets/icons/a.svg` | me | here | MIT |";
        assert_eq!(missing(&files, md), vec!["docs/images/b.png".to_string(), "tests/fixture.png".into()]);
        assert!(!is_asset("crates/x/src/lib.rs"));
        assert!(is_asset("assets/fonts/OFL.txt"));
    }
}

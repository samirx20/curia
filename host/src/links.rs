//! Folder links (junctions on Windows, symlinks elsewhere), and removing a folder without
//! ever following one: removing through a link once deleted the files it pointed to in
//! Agent Hub's main checkout.
use anyhow::{Context, Result};
use std::path::Path;

/// Links the folder `link` to `target`. Junctions need no special rights on Windows.
#[cfg(windows)]
pub fn link_dir(target: &Path, link: &Path) -> Result<()> {
    let out = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .context("could not run mklink")?;
    if !out.status.success() {
        anyhow::bail!("could not link {} to {}: {}", link.display(), target.display(), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn link_dir(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link).with_context(|| format!("could not link {} to {}", link.display(), target.display()))
}

/// Whether `path` is a link (symlink or junction) rather than a real file or folder.
pub fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink() || reparse_point(&meta))
}

/// A junction is a reparse point that isn't reported as a symlink.
#[cfg(windows)]
fn reparse_point(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn reparse_point(_: &std::fs::Metadata) -> bool {
    false
}

/// Removes every link inside `dir` (the links only, never what they point to), descending
/// into real folders except those named in `skip`.
pub fn unlink_all(dir: &Path, skip: &[&str]) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if is_link(&path) {
            // On Windows a directory link is removed with remove_dir.
            std::fs::remove_dir(&path).or_else(|_| std::fs::remove_file(&path)).with_context(|| format!("could not unlink {}", path.display()))?;
        } else if entry.file_type()?.is_dir() && !skip.iter().any(|s| entry.file_name() == *s) {
            unlink_all(&path, skip)?;
        }
    }
    Ok(())
}

/// Removes a folder Curia made, unlinking the links in it first.
pub fn remove_tree(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    unlink_all(dir, &[])?;
    std::fs::remove_dir_all(dir).with_context(|| format!("could not remove {}", dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removing_a_tree_never_follows_a_link() {
        let base = std::env::temp_dir().join(format!("curia-links-{}", uuid::Uuid::new_v4().simple()));
        let (outside, tree) = (base.join("outside"), base.join("tree"));
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::create_dir_all(tree.join("real")).unwrap();
        std::fs::write(outside.join("precious.txt"), "keep me").unwrap();
        link_dir(&outside, &tree.join("real").join("linked")).unwrap();
        assert!(is_link(&tree.join("real").join("linked")));
        assert!(tree.join("real").join("linked").join("precious.txt").exists(), "the link works");
        remove_tree(&tree).unwrap();
        assert!(!tree.exists());
        assert!(outside.join("precious.txt").exists(), "the link's target must survive");
        let _ = std::fs::remove_dir_all(&base);
    }
}

//! Checkpoints: the working tree (tracked and untracked files, honouring .gitignore) written
//! as a commit through a temporary index, so the user's real index and branches are never
//! touched. Commits are kept alive by refs under `refs/curia/checkpoints/`.
//! Adapted from Agent Hub's parked Rust app (`git/checkpoint.rs`).
use super::{is_repo, run, run_env};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Diffs larger than this are cut (the checkpoints still hold everything).
const MAX_PATCH: usize = 2 * 1024 * 1024;
const AUTHOR: &[(&str, &str)] = &[
    ("GIT_AUTHOR_NAME", "Curia"),
    ("GIT_AUTHOR_EMAIL", "curia@localhost"),
    ("GIT_COMMITTER_NAME", "Curia"),
    ("GIT_COMMITTER_EMAIL", "curia@localhost"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct FileChange {
    pub path: String,
    pub added: u32,
    pub removed: u32,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diff {
    pub files: Vec<FileChange>,
    pub patch: String,
    pub truncated: bool,
}

fn temp_index(cwd: &Path) -> Result<PathBuf> {
    let dir = run(cwd, &["rev-parse", "--git-dir"])?;
    let dir = if Path::new(&dir).is_absolute() { PathBuf::from(dir) } else { cwd.join(dir) };
    Ok(dir.join(format!("curia-index-{}", &crate::store::new_id()[..8])))
}

/// Commits the current working tree and returns the commit; `None` outside a git repo.
pub fn snapshot(cwd: &Path, label: &str) -> Result<Option<String>> {
    if !is_repo(cwd) {
        return Ok(None);
    }
    let idx = temp_index(cwd)?;
    let idx_s = idx.to_string_lossy().into_owned();
    let env = [("GIT_INDEX_FILE", idx_s.as_str())];
    let result = (|| {
        let head = run(cwd, &["rev-parse", "--verify", "-q", "HEAD"]).ok();
        if let Some(h) = &head {
            run_env(cwd, &["read-tree", h], &env)?;
        }
        run_env(cwd, &["add", "-A", "--", "."], &env)?;
        let tree = run_env(cwd, &["write-tree"], &env)?;
        let mut args = vec!["commit-tree", tree.as_str(), "-m", label];
        if let Some(h) = &head {
            args.extend(["-p", h.as_str()]);
        }
        let commit = run_env(cwd, &args, AUTHOR)?;
        run(cwd, &["update-ref", &format!("refs/curia/checkpoints/{commit}"), &commit])?;
        Ok(Some(commit))
    })();
    let _ = std::fs::remove_file(&idx);
    result
}

/// What changed between two checkpoints; `to` = `None` means the working tree now.
pub fn diff(cwd: &Path, from: &str, to: Option<&str>) -> Result<Diff> {
    let now;
    let to = match to {
        Some(t) => t,
        None => {
            now = snapshot(cwd, "curia: working tree")?.context("not a git repository")?;
            now.as_str()
        }
    };
    let with = |extra: &[&str]| run(cwd, &[&["diff", "--no-ext-diff", "--no-color", "-M"][..], extra, &[from, to]].concat());
    let files = with(&["--numstat"])?.lines().filter_map(numstat_line).collect();
    let mut patch = with(&[])?;
    let truncated = patch.len() > MAX_PATCH;
    if truncated {
        let mut cut = MAX_PATCH;
        while !patch.is_char_boundary(cut) {
            cut -= 1;
        }
        patch.truncate(cut);
    }
    Ok(Diff { files, patch, truncated })
}

/// One `--numstat` line. Renames look like `src/{a => b}.rs` or `a => b`; the new path is kept.
fn numstat_line(l: &str) -> Option<FileChange> {
    let mut parts = l.splitn(3, '\t');
    let (a, r, raw) = (parts.next()?, parts.next()?, parts.next()?);
    let path = match raw.split_once(" => ") {
        Some((left, right)) if left.contains('{') => {
            let (pre, _) = left.split_once('{').unwrap_or((left, ""));
            let (new, post) = right.split_once('}').unwrap_or((right, ""));
            format!("{pre}{new}{post}").replace("//", "/")
        }
        Some((_, right)) => right.to_string(),
        None => raw.to_string(),
    };
    Some(FileChange { path, added: a.parse().unwrap_or(0), removed: r.parse().unwrap_or(0), binary: a == "-" })
}

/// Puts the working tree back to `target`, after a safety checkpoint of how it is now
/// (returned, so the restore itself can be undone). The user's staged changes stay as they are.
pub fn restore(cwd: &Path, target: &str) -> Result<String> {
    let safety = snapshot(cwd, "curia: before restore")?.context("not a git repository")?;
    let idx = temp_index(cwd)?;
    let idx_s = idx.to_string_lossy().into_owned();
    let env = [("GIT_INDEX_FILE", idx_s.as_str())];
    let result = run_env(cwd, &["read-tree", &safety], &env).and_then(|_| run_env(cwd, &["read-tree", "-u", "--reset", target], &env));
    let _ = std::fs::remove_file(&idx);
    result.map(|_| safety)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> PathBuf {
        let d = std::env::temp_dir().join(format!("curia-git-{}", &crate::store::new_id()[..8]));
        std::fs::create_dir_all(&d).unwrap();
        run(&d, &["init", "-q"]).unwrap();
        std::fs::write(d.join("a.txt"), "one\n").unwrap();
        run(&d, &["add", "."]).unwrap();
        run_env(&d, &["commit", "-qm", "init"], AUTHOR).unwrap();
        d
    }

    #[test]
    fn snapshot_diff_restore_roundtrip() {
        let d = repo();
        let before = snapshot(&d, "before").unwrap().unwrap();
        std::fs::write(d.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(d.join("new.txt"), "fresh\n").unwrap();
        let after = snapshot(&d, "after").unwrap().unwrap();
        let r = diff(&d, &before, Some(&after)).unwrap();
        let mut paths: Vec<_> = r.files.iter().map(|f| (f.path.as_str(), f.added)).collect();
        paths.sort();
        assert_eq!(paths, vec![("a.txt", 1), ("new.txt", 1)]);
        assert!(r.patch.contains("+two"));
        assert_eq!(run(&d, &["diff", "--cached", "--name-only"]).unwrap(), "", "the real index is untouched");
        let safety = restore(&d, &before).unwrap();
        assert_eq!(std::fs::read_to_string(d.join("a.txt")).unwrap().replace("\r\n", "\n"), "one\n");
        assert!(!d.join("new.txt").exists(), "files added after the checkpoint are removed");
        restore(&d, &safety).unwrap();
        assert!(d.join("new.txt").exists(), "a restore can itself be undone");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn outside_a_repo_there_is_no_checkpoint() {
        let d = std::env::temp_dir().join(format!("curia-nogit-{}", &crate::store::new_id()[..8]));
        std::fs::create_dir_all(&d).unwrap();
        assert_eq!(snapshot(&d, "x").unwrap(), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn parses_renames() {
        assert_eq!(numstat_line("1\t0\tsrc/{a => b}.rs").unwrap().path, "src/b.rs");
        assert!(numstat_line("-\t-\timg.png").unwrap().binary);
    }
}

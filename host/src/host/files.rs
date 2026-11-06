//! A thread's folder as clients see it: what changed, a file's text, a file's changes; and
//! changing a thread's model, effort or freedom.
use super::Host;
use crate::git::run;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};

/// Files larger than this are not sent whole.
const MAX_FILE: u64 = 1024 * 1024;

impl Host {
    /// Changed files in the thread's folder (against its branch's last commit), with line counts.
    pub async fn thread_files(&self, thread_id: &str) -> Result<Value> {
        let cwd = PathBuf::from(self.store.thread_get(thread_id)?.cwd);
        tokio::task::spawn_blocking(move || -> Result<Value> {
            if !crate::git::is_repo(&cwd) {
                return Ok(json!({"git": false, "files": []}));
            }
            let mut files = Vec::new();
            for line in run(&cwd, &["status", "--porcelain=v1", "--untracked-files=all"])?.lines() {
                let (code, path) = line.split_at(3.min(line.len()));
                let path = path.rsplit(" -> ").next().unwrap_or(path).trim_matches('"').to_string();
                let status = match code.trim() {
                    "??" | "A" => "added",
                    "D" => "deleted",
                    _ => "modified",
                };
                let (added, removed) = match status {
                    "added" => (std::fs::read_to_string(cwd.join(&path)).map(|t| t.lines().count()).unwrap_or(0), 0),
                    _ => numstat(&cwd, &path),
                };
                files.push(json!({"path": path, "status": status, "added": added, "removed": removed}));
            }
            Ok(json!({"git": true, "files": files}))
        })
        .await?
    }

    /// A file's text (or a note that it is binary or too large).
    pub fn file_read(&self, thread_id: &str, path: &str) -> Result<Value> {
        let full = inside(&self.store.thread_get(thread_id)?.cwd, path)?;
        let meta = std::fs::metadata(&full).with_context(|| format!("no file {path}"))?;
        if meta.len() > MAX_FILE {
            return Ok(json!({"path": path, "too_large": true, "size": meta.len()}));
        }
        let bytes = std::fs::read(&full)?;
        match String::from_utf8(bytes) {
            Ok(text) if !text.contains('\0') => Ok(json!({"path": path, "text": crate::redact::redact(&text)})),
            _ => Ok(json!({"path": path, "binary": true, "size": meta.len()})),
        }
    }

    /// A file's changes against the folder's last commit, as a unified diff.
    pub async fn file_diff(&self, thread_id: &str, path: &str) -> Result<Value> {
        let row = self.store.thread_get(thread_id)?;
        inside(&row.cwd, path)?;
        let (cwd, p) = (PathBuf::from(row.cwd), path.to_string());
        let patch = tokio::task::spawn_blocking(move || -> Result<String> {
            let tracked = run(&cwd, &["ls-files", "--error-unmatch", "--", &p]).is_ok();
            if tracked {
                run(&cwd, &["diff", "--no-ext-diff", "--no-color", "HEAD", "--", &p])
            } else {
                let text = std::fs::read_to_string(cwd.join(&p)).unwrap_or_default();
                if text.is_empty() {
                    return Ok("A new, empty file.".to_string());
                }
                Ok(text.lines().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n"))
            }
        })
        .await??;
        Ok(json!({"path": path, "patch": crate::redact::redact(&patch)}))
    }

    /// Changes a thread's model, effort or freedom (never above its parent's). A running CLI is
    /// closed, so the next message starts it with the new settings and the same session.
    pub async fn thread_configure(&self, thread_id: &str, model: Option<&str>, effort: Option<&str>, freedom: Option<&str>) -> Result<()> {
        let row = self.store.thread_get(thread_id)?;
        if row.status == "running" {
            bail!("the thread is mid-turn; change it once the turn ends");
        }
        if let (Some(f), Some(parent)) = (freedom, &row.parent_id) {
            let parent = self.store.thread_get(parent)?;
            if crate::freedom::rank(f) > crate::freedom::rank(&parent.permission_mode) {
                bail!("a team thread can't have more freedom than the thread that gave it the task");
            }
        }
        self.store.thread_set_options(thread_id, model, effort, freedom.map(crate::freedom::level))?;
        if let Some(live) = self.live_of(thread_id) {
            live.session.close().await;
        }
        Ok(())
    }
}

fn numstat(cwd: &Path, path: &str) -> (usize, usize) {
    let out = run(cwd, &["diff", "--numstat", "HEAD", "--", path]).unwrap_or_default();
    let mut parts = out.split_whitespace();
    let n = |s: Option<&str>| s.and_then(|v| v.parse().ok()).unwrap_or(0);
    (n(parts.next()), n(parts.next()))
}

/// A path inside the thread's folder; anything that climbs out (`..`, an absolute path) is refused.
fn inside(cwd: &str, path: &str) -> Result<PathBuf> {
    let p = Path::new(path);
    if p.is_absolute() || p.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_) | Component::RootDir)) {
        bail!("{path} is outside the thread's folder");
    }
    Ok(Path::new(cwd).join(p))
}

#[cfg(test)]
mod tests {
    #[test]
    fn paths_must_stay_inside_the_folder() {
        assert!(super::inside("C:\\run", "src/a.rs").is_ok());
        assert!(super::inside("C:\\run", "../secret").is_err());
        assert!(super::inside("C:\\run", "C:\\Windows\\win.ini").is_err());
        assert!(super::inside("/run", "/etc/passwd").is_err());
    }
}

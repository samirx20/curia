//! A thread's whole folder for the Files rail (Design 05): every file with its git status, and
//! short-lived links to a file's bytes (images, video, downloads), as it is now or as of the
//! last commit (image compare). Links are tokens that expire, so a video player or a phone can
//! fetch them without signing in again.
use super::Host;
use crate::git::run;
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// A file a link opens.
#[derive(Clone)]
pub struct Grant {
    pub folder: PathBuf,
    pub path: String,
    /// `None` for the file as it is; `Some("HEAD")` for its last committed version.
    pub rev: Option<String>,
    until: Instant,
}

const LINK_FOR: Duration = Duration::from_secs(600);
/// A folder that isn't a git repository is walked, up to this many files.
const WALK_LIMIT: usize = 5000;

impl Host {
    /// Every file in the thread's folder, with `added`, `modified` or `deleted` where git says so.
    pub async fn thread_tree(&self, thread_id: &str) -> Result<Value> {
        let row = self.store.thread_get(thread_id)?;
        let cwd = PathBuf::from(&row.cwd);
        let branch = row.branch.clone();
        tokio::task::spawn_blocking(move || -> Result<Value> {
            if !crate::git::is_repo(&cwd) {
                return Ok(json!({"git": false, "root": cwd, "files": walk(&cwd)}));
            }
            let mut status: HashMap<String, &str> = HashMap::new();
            for line in run(&cwd, &["status", "--porcelain=v1", "--untracked-files=all"])?.lines() {
                let (code, path) = line.split_at(3.min(line.len()));
                let path = path.rsplit(" -> ").next().unwrap_or(path).trim_matches('"').to_string();
                status.insert(path, match code.trim() { "??" | "A" => "added", "D" => "deleted", _ => "modified" });
            }
            let listed = run(&cwd, &["ls-files", "--cached", "--others", "--exclude-standard"])?;
            let mut paths: Vec<String> = listed.lines().map(str::to_string).collect();
            paths.extend(status.iter().filter(|(_, s)| **s == "deleted").map(|(p, _)| p.clone()));
            paths.sort();
            paths.dedup();
            let files: Vec<Value> = paths.iter().map(|p| json!({"path": p, "status": status.get(p).copied()})).collect();
            let branch = branch.or_else(|| run(&cwd, &["branch", "--show-current"]).ok().map(|b| b.trim().to_string()));
            Ok(json!({"git": true, "root": cwd, "branch": branch, "files": files}))
        })
        .await?
    }

    /// A link to a file's bytes, good for ten minutes.
    pub fn file_link(&self, thread_id: &str, path: &str, rev: Option<&str>) -> Result<Value> {
        let row = self.store.thread_get(thread_id)?;
        let full = super::files::inside(&row.cwd, path)?;
        if rev.is_none() {
            std::fs::metadata(&full).with_context(|| format!("no file {path}"))?;
        }
        let token = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
        let grant = Grant { folder: row.cwd.into(), path: path.replace('\\', "/"), rev: rev.map(str::to_string), until: Instant::now() + LINK_FOR };
        let mut grants = super::lock(&self.file_links);
        grants.retain(|_, g| g.until > Instant::now());
        grants.insert(token.clone(), grant);
        Ok(json!({"url": format!("/file/{token}"), "name": path.rsplit(['/', '\\']).next().unwrap_or(path)}))
    }

    /// What a link opens, if it's still good.
    pub fn file_grant(&self, token: &str) -> Option<Grant> {
        super::lock(&self.file_links).get(token).filter(|g| g.until > Instant::now()).cloned()
    }
}

/// The bytes of a file as of the last commit.
pub fn committed(grant: &Grant) -> Result<Vec<u8>> {
    let out = std::process::Command::new("git")
        .arg("-C").arg(&grant.folder)
        .args(["show", &format!("{}:{}", grant.rev.as_deref().unwrap_or("HEAD"), grant.path)])
        .output()
        .context("could not run git")?;
    if !out.status.success() {
        anyhow::bail!("that file isn't in the last commit");
    }
    Ok(out.stdout)
}

/// Files of a folder that isn't a git repository, skipping the usual heavy folders.
fn walk(root: &std::path::Path) -> Vec<Value> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if [".git", "node_modules", "target", ".venv", "__pycache__"].contains(&name.as_str()) || crate::links::is_link(&e.path()) {
                continue;
            }
            if e.path().is_dir() {
                stack.push(e.path());
            } else if let Ok(rel) = e.path().strip_prefix(root) {
                out.push(json!({"path": rel.to_string_lossy().replace('\\', "/"), "status": null}));
                if out.len() >= WALK_LIMIT {
                    return out;
                }
            }
        }
    }
    out
}

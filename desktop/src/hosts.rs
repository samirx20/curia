//! The hosts this app can follow: the one on this computer (found through its data folder) and
//! hosts on other machines it paired with, the way a phone pairs. The list and the last choice
//! live in `%LOCALAPPDATA%\Curia` beside the local host's data.
use anyhow::{Result, bail};
use curia_client::device::{self, Device};
use curia_client::{Incoming, Link};
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

#[derive(Clone)]
pub enum Target {
    Local(PathBuf),
    Remote(Device),
}

impl Target {
    pub fn id(&self) -> String {
        match self {
            Target::Local(_) => "local".into(),
            Target::Remote(d) => d.device_id.clone(),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Target::Local(_) => std::env::var("COMPUTERNAME").unwrap_or_else(|_| "This computer".into()),
            Target::Remote(d) => d.host_name.clone(),
        }
    }

    pub async fn connect(&self) -> Result<(Link, mpsc::UnboundedReceiver<Incoming>)> {
        match self {
            Target::Local(data) => Link::connect_local(data).await,
            Target::Remote(d) => Link::connect_device(d).await,
        }
    }

    /// The host's web address for file links: from `link.json` here, the paired address there.
    pub fn http_url(&self) -> String {
        match self {
            Target::Local(data) => std::fs::read_to_string(data.join("link.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|v| v["url"].as_str().map(|u| u.replace("ws://", "http://").trim_end_matches("/ws").to_string()))
                .unwrap_or_default(),
            Target::Remote(d) => d.url.trim_end_matches('/').to_string(),
        }
    }
}

fn app_folder() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("HOME")).unwrap_or_default();
    Path::new(&base).join("Curia")
}

/// The local host's data folder: `--data` when given, else where an installed host keeps it.
pub fn data_folder(given: &str) -> PathBuf {
    if given.is_empty() { app_folder().join("data") } else { PathBuf::from(given) }
}

fn hosts_file() -> PathBuf {
    app_folder().join("hosts.json")
}

fn choice_file() -> PathBuf {
    app_folder().join("current-host")
}

/// Where to connect at start: this computer when `--data` is given, else the last host chosen.
pub fn first(data_arg: &str) -> Target {
    let chosen = std::fs::read_to_string(choice_file()).unwrap_or_default();
    match device::load(&hosts_file()).into_iter().find(|d| d.device_id == chosen.trim()) {
        Some(d) if data_arg.is_empty() => Target::Remote(d),
        _ => Target::Local(data_folder(data_arg)),
    }
}

/// Switches to a host (and remembers it for next time).
pub fn choose(id: &str, data_arg: &str) -> Result<Target> {
    let target = if id == "local" {
        Target::Local(data_folder(data_arg))
    } else {
        match device::load(&hosts_file()).into_iter().find(|d| d.device_id == id) {
            Some(d) => Target::Remote(d),
            None => bail!("this app isn't paired with that host any more"),
        }
    };
    std::fs::create_dir_all(app_folder())?;
    std::fs::write(choice_file(), target.id())?;
    Ok(target)
}

/// Keeps a new pairing (replacing an older one with the same host).
pub fn add(new: Device) -> Result<()> {
    let mut all: Vec<Device> = device::load(&hosts_file()).into_iter().filter(|d| d.url != new.url).collect();
    all.push(new);
    device::save(&hosts_file(), &all)
}

pub fn forget(id: &str) -> Result<()> {
    let all: Vec<Device> = device::load(&hosts_file()).into_iter().filter(|d| d.device_id != id).collect();
    device::save(&hosts_file(), &all)
}

/// Every host this app knows, as the switcher shows them.
pub fn list_json(current: &Target) -> String {
    let mut rows = vec![json!({"id": "local", "name": Target::Local(PathBuf::new()).name(), "where": "This computer", "role": "owner"})];
    for d in device::load(&hosts_file()) {
        rows.push(json!({"id": d.device_id, "name": d.host_name, "where": d.url, "role": d.role}));
    }
    for r in rows.iter_mut() {
        r["current"] = json!(r["id"] == current.id());
    }
    serde_json::Value::Array(rows).to_string()
}

/// Starts the host installed beside this app, detached, so it keeps running when the app
/// closes. Only when there is one (an installed copy).
pub fn start_host(data: &Path) -> bool {
    let name = if cfg!(windows) { "curia-host.exe" } else { "curia-host" };
    let Some(exe) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(name))) else { return false };
    if !exe.is_file() {
        return false;
    }
    let _ = std::fs::create_dir_all(data);
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("serve").arg("--data").arg(data);
    if let Ok(log) = std::fs::OpenOptions::new().create(true).append(true).open(data.join("host.log")) {
        if let Ok(err) = log.try_clone() {
            cmd.stdout(log).stderr(err);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0000_0008 | 0x0800_0000); // DETACHED_PROCESS | CREATE_NO_WINDOW
    }
    cmd.spawn().is_ok()
}

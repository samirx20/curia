//! What Settings shows and changes on the host (Design 13–20): the machine and its resources,
//! signing a CLI account in on the host, a team's sign-in state, and starting the host when
//! Sam signs in to Windows.
use super::Host;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

impl Host {
    /// This machine, its resources, and how devices reach it.
    pub async fn host_info(&self) -> Result<Value> {
        let settings = self.store.settings()?;
        let data_dir = self.data_dir().to_path_buf();
        let link: Value = std::fs::read_to_string(data_dir.join("link.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let started = self.started_ms;
        tokio::task::spawn_blocking(move || -> Result<Value> {
            use sysinfo::{Disks, System};
            let mut sys = System::new()?;
            sys.refresh_cpu_usage();
            std::thread::sleep(Duration::from_millis(250));
            sys.refresh_cpu_usage();
            sys.refresh_memory();
            let disks = Disks::new_with_refreshed_list()?;
            let disk = disks.iter().filter(|d| data_dir.starts_with(d.mount_point())).max_by_key(|d| d.mount_point().as_os_str().len());
            let name = settings.host_name.clone().filter(|n| !n.trim().is_empty()).or_else(|| System::host_name().ok()).unwrap_or_else(|| "This computer".into());
            Ok(json!({
                "name": name,
                "os": System::long_os_version().unwrap_or_default(),
                "version": env!("CARGO_PKG_VERSION"),
                "started_ms": started,
                "cpu_percent": sys.global_cpu_usage(),
                "cores": sys.cpus().len(),
                "memory": {"used": sys.used_memory(), "total": sys.total_memory()},
                "disk": disk.map(|d| json!({"used": d.total_space() - d.available_space(), "total": d.total_space()})),
                "data_dir": data_dir,
                "local_url": link["url"].as_str().map(|u| u.replace("ws://", "http://").trim_end_matches("/ws").to_string()),
                "remote_url": link["remote_url"],
                "autostart": autostart_file().map(|f| f.is_file()).unwrap_or(false),
            }))
        })
        .await?
    }

    /// Opens a console window on the host where the CLI signs this account in (some CLIs read
    /// their sign-in code only from a real console). Creates the account folder if it's new.
    pub fn team_sign_in(&self, cli: &str, config_dir: &str) -> Result<()> {
        let dir = PathBuf::from(config_dir);
        std::fs::create_dir_all(&dir).with_context(|| format!("could not make {}", dir.display()))?;
        let (var, value, line) = match cli {
            "claude" => ("CLAUDE_CONFIG_DIR", config_dir.to_string(), "claude".to_string()),
            "codex" => ("CODEX_HOME", config_dir.to_string(), "codex login".to_string()),
            "agy" => ("SSH_CONNECTION", "1".to_string(), format!("agy --gemini_dir=\"{config_dir}\"")),
            other => bail!("Curia can't sign in {other:?} accounts"),
        };
        if !cfg!(windows) {
            bail!("signing in from Settings works on Windows hosts for now; run `{line}` with {var}={value} in a terminal on the host");
        }
        std::process::Command::new("cmd")
            .args(["/c", "start", "Curia: sign in", "cmd", "/k", &format!("echo Sign this {cli} account in, then close this window. && {line}")])
            .env(var, value)
            .spawn()
            .context("could not open a console window")?;
        Ok(())
    }

    /// Turns starting the host when Sam signs in to Windows on or off. The entry runs this
    /// host the way it runs now, and starts it again if it stops.
    pub fn autostart_set(&self, enabled: bool) -> Result<()> {
        let file = autostart_file().context("starting at sign-in is set up on Windows hosts only, for now")?;
        if !enabled {
            if file.is_file() {
                std::fs::remove_file(&file)?;
            }
            return Ok(());
        }
        let exe = std::env::current_exe()?;
        let args: Vec<String> = std::env::args().skip(1).map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a }).collect();
        let log = self.data_dir().join("host.log");
        let script = format!(
            "@echo off\r\nrem Starts the Curia host when you sign in to Windows (Settings > Host). Restarts it if it stops.\r\n\
             title Curia host\r\n:loop\r\n\"{}\" {} >> \"{}\" 2>&1\r\ntimeout /t 5 /nobreak >nul\r\ngoto loop\r\n",
            exe.display(), args.join(" "), log.display()
        );
        std::fs::write(&file, script).with_context(|| format!("could not write {}", file.display()))
    }
}

/// The Windows Startup folder entry for the host.
fn autostart_file() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let appdata = std::env::var_os("APPDATA")?;
    Some(Path::new(&appdata).join(r"Microsoft\Windows\Start Menu\Programs\Startup\Curia host.cmd"))
}

/// Whether the account folder holds a signed-in CLI (each CLI keeps its sign-in there).
pub fn signed_in(cli: &str, config_dir: &str) -> bool {
    let dir = Path::new(config_dir);
    match cli {
        "claude" => dir.join(".credentials.json").is_file() || dir.join(".claude.json").is_file(),
        "codex" => dir.join("auth.json").is_file(),
        "agy" => dir.join("antigravity-cli").join("antigravity-oauth-token").is_file(),
        _ => false,
    }
}

/// A QR code for `text`, as rows of `1` (dark) and `0`.
pub fn qr(text: &str) -> Result<Value> {
    let code = qrcode::QrCode::new(text.as_bytes()).context("the address is too long for a QR code")?;
    let width = code.width();
    let rows: Vec<String> = code.to_colors().chunks(width).map(|row| row.iter().map(|c| if *c == qrcode::Color::Dark { '1' } else { '0' }).collect()).collect();
    Ok(json!({"size": width, "rows": rows}))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_qr_code_is_square_rows_of_cells() {
        let q = super::qr("https://curia.example.ts.net/#pair=ABC123").unwrap();
        let size = q["size"].as_u64().unwrap() as usize;
        let rows = q["rows"].as_array().unwrap();
        assert_eq!(rows.len(), size);
        assert!(rows.iter().all(|r| r.as_str().unwrap().len() == size));
    }
}

//! One shell in a thread's folder, in a pseudo-terminal (ConPTY on Windows). A reader thread
//! feeds its output into the terminal the host keeps; every change bumps `version`, which the
//! clients showing it wait on. What the terminal itself must answer (cursor reports and the
//! like) is written back to the shell.
use super::screen::{Screen, Size, snapshot};
use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::term::Config;
use alacritty_terminal::vte::ansi::Processor;
use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use tokio::sync::watch;

/// Passes the terminal's own replies back to the shell.
#[derive(Clone)]
pub struct Replies(mpsc::Sender<String>);

impl EventListener for Replies {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event {
            let _ = self.0.send(text);
        }
    }
}

struct Screenful {
    term: Term<Replies>,
    parser: Processor,
    size: Size,
}

pub struct Shell {
    pub id: String,
    pub thread_id: String,
    pub title: String,
    screen: Mutex<Screenful>,
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    /// Everything the shell starts runs in this Job Object; closing it ends them all.
    job: Mutex<Option<crate::process::Job>>,
    pub exited: AtomicBool,
    /// When it was last used, in ms (for closing idle shells).
    pub last_used: AtomicI64,
    /// Bumped on every change to the screen.
    pub version: watch::Sender<u64>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// The shell to run: PowerShell 7 if it's installed, else Windows PowerShell; `$SHELL` elsewhere.
fn program() -> (String, String) {
    if cfg!(windows) {
        let pwsh = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).any(|d| d.join("pwsh.exe").is_file());
        if pwsh { ("pwsh.exe".into(), "PowerShell".into()) } else { ("powershell.exe".into(), "Windows PowerShell".into()) }
    } else {
        let sh = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
        let name = Path::new(&sh).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| sh.clone());
        (sh, name)
    }
}

impl Shell {
    pub fn start(id: String, thread_id: String, cwd: &Path, size: Size) -> Result<Arc<Shell>> {
        let pair = native_pty_system()
            .openpty(PtySize { rows: size.rows, cols: size.cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| anyhow::anyhow!("could not open a terminal: {e}"))?;
        let (exe, title) = program();
        let mut cmd = CommandBuilder::new(&exe);
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        let child = pair.slave.spawn_command(cmd).map_err(|e| anyhow::anyhow!("could not start {exe}: {e}"))?;
        // Contain the shell like every CLI, so a server it starts can't outlive it.
        let job = crate::process::Job::new()?;
        #[cfg(windows)]
        if let Some(handle) = child.as_raw_handle() {
            job.assign(handle)?;
        }
        let mut reader = pair.master.try_clone_reader().map_err(|e| anyhow::anyhow!("terminal output: {e}"))?;
        let writer = pair.master.take_writer().map_err(|e| anyhow::anyhow!("terminal input: {e}"))?;
        let (reply_tx, reply_rx) = mpsc::channel::<String>();
        let term = Term::new(Config::default(), &size, Replies(reply_tx));
        let shell = Arc::new(Shell {
            id,
            thread_id,
            title,
            screen: Mutex::new(Screenful { term, parser: Processor::new(), size }),
            writer: Mutex::new(writer),
            master: Mutex::new(pair.master),
            child: Mutex::new(child),
            job: Mutex::new(Some(job)),
            exited: AtomicBool::new(false),
            last_used: AtomicI64::new(crate::store::now_ms()),
            version: watch::channel(0).0,
        });
        let me = shell.clone();
        std::thread::Builder::new().name(format!("term-{}", &me.id[..8])).spawn(move || {
            let mut buf = [0u8; 16 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        {
                            let mut s = lock(&me.screen);
                            let Screenful { term, parser, .. } = &mut *s;
                            parser.advance(term, &buf[..n]);
                        }
                        // The terminal's own answers (e.g. where the cursor is) go back in.
                        while let Ok(reply) = reply_rx.try_recv() {
                            let _ = lock(&me.writer).write_all(reply.as_bytes());
                        }
                        me.touch();
                    }
                }
            }
            me.exited.store(true, Ordering::SeqCst);
            me.version.send_modify(|v| *v += 1);
        }).context("could not start the terminal's reader")?;
        Ok(shell)
    }

    fn touch(&self) {
        self.last_used.store(crate::store::now_ms(), Ordering::Relaxed);
        self.version.send_modify(|v| *v += 1);
    }

    pub fn screen(&self) -> Screen {
        let s = lock(&self.screen);
        snapshot(&s.term, s.size)
    }

    pub fn input(&self, data: &str) -> Result<()> {
        let mut w = lock(&self.writer);
        w.write_all(data.as_bytes()).context("the shell has stopped")?;
        w.flush()?;
        self.last_used.store(crate::store::now_ms(), Ordering::Relaxed);
        Ok(())
    }

    /// One size for everyone looking at it: the last client to resize wins.
    pub fn resize(&self, size: Size) -> Result<()> {
        let size = Size { cols: size.cols.clamp(20, 400), rows: size.rows.clamp(5, 200) };
        lock(&self.master)
            .resize(PtySize { rows: size.rows, cols: size.cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| anyhow::anyhow!("could not resize the terminal: {e}"))?;
        let mut s = lock(&self.screen);
        s.term.resize(size);
        s.size = size;
        drop(s);
        self.touch();
        Ok(())
    }

    pub fn pid(&self) -> Option<u32> {
        lock(&self.child).process_id()
    }

    /// Ends the shell and everything it started.
    pub fn close(&self) {
        let _ = lock(&self.child).kill();
        lock(&self.job).take();
    }
}

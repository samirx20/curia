//! The one object QML talks to: a link to the host. QML sends link calls as JSON and gets
//! replies and pushed events back as JSON signals; it holds no state of its own beyond what
//! it shows (the host owns all state and logic).
//!
//! The link runs on a small tokio runtime of its own; results come back to the Qt thread
//! through CXX-Qt's queue, so QML is only ever touched from its own thread.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, connected)]
        #[qproperty(QString, status)]
        #[qproperty(QString, host_name, cxx_name = "hostName")]
        #[qproperty(QString, host_id, cxx_name = "hostId")]
        type HostLink = super::HostLinkRust;

        /// Connects: to the host on this computer whose data folder is `data_dir` (it holds
        /// `link.json`; "" for the installed one's), or to the paired host chosen last time.
        #[qinvokable]
        fn start(self: Pin<&mut HostLink>, data_dir: &QString);

        /// Every host this app knows (`[{id, name, where, role, current}]` as JSON).
        #[qinvokable]
        #[cxx_name = "hostsJson"]
        fn hosts_json(self: &HostLink) -> QString;

        /// Pairs with a host on another machine (its pairing link, or address and code); the
        /// answer comes back as `reply("paired", ...)` or `failed("paired", message)`.
        #[qinvokable]
        #[cxx_name = "pairHost"]
        fn pair_host(self: Pin<&mut HostLink>, link: &QString, code: &QString);

        /// Follows another host ("local" for this computer's).
        #[qinvokable]
        #[cxx_name = "useHost"]
        fn use_host(self: Pin<&mut HostLink>, id: &QString);

        /// Forgets a paired host (that host still lists this app until it's unpaired there).
        #[qinvokable]
        #[cxx_name = "forgetHost"]
        fn forget_host(self: Pin<&mut HostLink>, id: &QString);

        /// Sends one link call (`{"method": …, "params": …}`); the answer comes back as
        /// `reply(tag, json)` or `failed(tag, message)`.
        #[qinvokable]
        fn call(self: Pin<&mut HostLink>, tag: &QString, call_json: &QString);

        /// A command-line value (`--data`, `--shot`, …), or "".
        #[qinvokable]
        fn arg(self: &HostLink, name: &QString) -> QString;

        /// The host's web address (`http://127.0.0.1:<port>`), for file links; "" before it's known.
        #[qinvokable]
        #[cxx_name = "httpUrl"]
        fn http_url(self: &HostLink) -> QString;

        /// One of the app's asset folders (`fonts`, `providers`), as a `file:` URL ending in `/`.
        #[qinvokable]
        #[cxx_name = "assetUrl"]
        fn asset_url(self: &HostLink, folder: &QString) -> QString;

        #[qsignal]
        fn reply(self: Pin<&mut HostLink>, tag: QString, json: QString);

        #[qsignal]
        fn failed(self: Pin<&mut HostLink>, tag: QString, message: QString);

        /// A pushed event (`{"thread_id", "seq", "event"}`).
        #[qsignal]
        fn event(self: Pin<&mut HostLink>, json: QString);

        /// The host closed the link, and why.
        #[qsignal]
        fn closed(self: Pin<&mut HostLink>, reason: QString);

        /// Something happened in a thread the window doesn't have open.
        #[qsignal]
        fn activity(self: Pin<&mut HostLink>, thread_id: QString, kind: QString);

        /// A terminal this app opened changed (`{id, title, screen, exited}` as JSON).
        #[qsignal]
        fn terminal(self: Pin<&mut HostLink>, json: QString);
    }

    impl cxx_qt::Threading for HostLink {}
}

use crate::hosts::{self, Target};
use core::pin::Pin;
use curia_client::{Incoming, Link};
use curia_core::protocol::Call;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

#[derive(Default)]
pub struct HostLinkRust {
    connected: bool,
    status: QString,
    host_name: QString,
    host_id: QString,
    runtime: Option<tokio::runtime::Runtime>,
    link: Arc<Mutex<Option<Arc<Link>>>>,
    data_arg: String,
    target: Option<Target>,
    /// Tells the connection loop to stop when the app switches hosts.
    switch: Option<watch::Sender<bool>>,
}

impl qobject::HostLink {
    fn start(mut self: Pin<&mut Self>, data_dir: &QString) {
        match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
            Ok(r) => self.as_mut().rust_mut().runtime = Some(r),
            Err(e) => {
                self.as_mut().set_status(QString::from(&format!("Could not start: {e}")));
                return;
            }
        }
        let data_arg = data_dir.to_string();
        let target = hosts::first(&data_arg);
        self.as_mut().rust_mut().data_arg = data_arg;
        self.follow(target);
    }

    /// Connects to `target`, and connects again whenever the link drops (a local host may have
    /// restarted on new ports, so link.json is read afresh each time), waiting 1 s, 2 s ... 10 s.
    fn follow(mut self: Pin<&mut Self>, target: Target) {
        if let Some(old) = self.as_mut().rust_mut().switch.take() {
            let _ = old.send(true);
        }
        *self.link.lock().unwrap() = None;
        let (switch, mut switched) = watch::channel(false);
        self.as_mut().rust_mut().switch = Some(switch);
        self.as_mut().rust_mut().target = Some(target.clone());
        self.as_mut().set_connected(false);
        self.as_mut().set_host_name(QString::from(&target.name()));
        self.as_mut().set_host_id(QString::from(&target.id()));
        self.as_mut().set_status(QString::from("Connecting…"));
        let qt = self.qt_thread();
        let slot = self.link.clone();
        let Some(runtime) = self.runtime.as_ref() else { return };
        runtime.spawn(async move {
            let mut wait = 1u64;
            let mut started_host = false;
            loop {
                match target.connect().await {
                    Ok((link, mut incoming)) => {
                        wait = 1;
                        if *switched.borrow() {
                            return;
                        }
                        *slot.lock().unwrap() = Some(Arc::new(link));
                        let _ = qt.queue(|mut h| {
                            h.as_mut().set_connected(true);
                            h.as_mut().set_status(QString::from("Connected"));
                        });
                        loop {
                            let next = tokio::select! {
                                i = incoming.recv() => i,
                                _ = switched.changed() => return,
                            };
                            let Some(i) = next else { break };
                            let sent = match i {
                                Incoming::Event(e) => {
                                    let json = QString::from(&serde_json::to_string(&e).unwrap_or_default());
                                    qt.queue(move |mut h| h.as_mut().event(json))
                                }
                                Incoming::Closing(reason) => {
                                    let reason = QString::from(&reason);
                                    qt.queue(move |mut h| h.as_mut().closed(reason))
                                }
                                Incoming::Activity { thread_id, kind } => {
                                    let (t, k) = (QString::from(&thread_id), QString::from(&kind));
                                    qt.queue(move |mut h| h.as_mut().activity(t, k))
                                }
                                Incoming::Terminal(v) => {
                                    let json = QString::from(&v.to_string());
                                    qt.queue(move |mut h| h.as_mut().terminal(json))
                                }
                            };
                            if sent.is_err() {
                                return; // the window is gone
                            }
                        }
                        *slot.lock().unwrap() = None;
                        let gone = qt.queue(|mut h| {
                            h.as_mut().set_connected(false);
                            h.as_mut().set_status(QString::from("The connection to the host was lost."));
                        });
                        if gone.is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        if let Target::Local(data) = &target {
                            if !started_host {
                                started_host = hosts::start_host(data);
                            }
                        }
                        let message = QString::from(&format!("{e:#}"));
                        if qt.queue(move |mut h| h.as_mut().set_status(message)).is_err() {
                            return;
                        }
                    }
                }
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(wait)) => {}
                    _ = switched.changed() => return,
                }
                wait = (wait * 2).min(10);
            }
        });
    }

    fn hosts_json(&self) -> QString {
        let current = self.target.clone().unwrap_or_else(|| hosts::first(&self.data_arg));
        QString::from(&hosts::list_json(&current))
    }

    fn pair_host(self: Pin<&mut Self>, link: &QString, code: &QString) {
        let (link, code) = (link.to_string(), code.to_string());
        let qt = self.qt_thread();
        let Some(runtime) = self.runtime.as_ref() else { return };
        runtime.spawn(async move {
            let name = format!("{} (desktop)", std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Curia".into()));
            let paired = async {
                let (url, code) = curia_client::device::parse_pairing(&link, &code)?;
                let device = curia_client::device::pair(&url, &code, &name).await?;
                let id = device.device_id.clone();
                hosts::add(device)?;
                anyhow::Ok(id)
            };
            let tag = QString::from("paired");
            let _ = match paired.await {
                Ok(id) => {
                    let json = QString::from(&serde_json::json!({ "id": id }).to_string());
                    qt.queue(move |mut h| h.as_mut().reply(tag, json))
                }
                Err(e) => {
                    let message = QString::from(&format!("{e:#}"));
                    qt.queue(move |mut h| h.as_mut().failed(tag, message))
                }
            };
        });
    }

    fn use_host(mut self: Pin<&mut Self>, id: &QString) {
        match hosts::choose(&id.to_string(), &self.data_arg) {
            Ok(target) => self.follow(target),
            Err(e) => self.as_mut().failed(QString::from("host"), QString::from(&format!("{e:#}"))),
        }
    }

    fn forget_host(mut self: Pin<&mut Self>, id: &QString) {
        let id = id.to_string();
        if let Err(e) = hosts::forget(&id) {
            self.as_mut().failed(QString::from("host"), QString::from(&format!("{e:#}")));
            return;
        }
        if self.target.as_ref().is_some_and(|t| t.id() == id) {
            self.use_host(&QString::from("local"));
        }
    }

    fn call(self: Pin<&mut Self>, tag: &QString, call_json: &QString) {
        let tag = tag.to_string();
        let parsed: Result<Call, _> = serde_json::from_str(&call_json.to_string());
        let link = self.link.lock().unwrap().clone();
        let qt = self.qt_thread();
        let Some(runtime) = self.runtime.as_ref() else { return };
        runtime.spawn(async move {
            let result = match (parsed, link) {
                (Err(e), _) => Err(format!("not a call: {e}")),
                (_, None) => Err("not connected to a host".to_string()),
                (Ok(call), Some(link)) => link.call(call).await.map_err(|e| format!("{e:#}")),
            };
            let tag = QString::from(&tag);
            let _ = match result {
                Ok(v) => {
                    let json = QString::from(&v.to_string());
                    qt.queue(move |mut h| h.as_mut().reply(tag, json))
                }
                Err(message) => {
                    let message = QString::from(&message);
                    qt.queue(move |mut h| h.as_mut().failed(tag, message))
                }
            };
        });
    }

    fn asset_url(&self, folder: &QString) -> QString {
        // Beside the program when installed; the source folder while developing.
        let folder = folder.to_string();
        let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(&folder)));
        let dir = beside.filter(|d| d.is_dir()).unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(&folder));
        QString::from(&format!("file:///{}/", dir.to_string_lossy().replace('\\', "/")))
    }

    fn http_url(&self) -> QString {
        QString::from(&self.target.as_ref().map(Target::http_url).unwrap_or_default())
    }

    fn arg(&self, name: &QString) -> QString {
        let name = name.to_string();
        let args: Vec<String> = std::env::args().collect();
        let value = args.iter().position(|a| *a == name).and_then(|i| args.get(i + 1)).cloned().unwrap_or_default();
        QString::from(&value)
    }
}

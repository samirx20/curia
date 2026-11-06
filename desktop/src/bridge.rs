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
        type HostLink = super::HostLinkRust;

        /// Connects to the host whose data folder is `data_dir` (it holds `link.json`).
        #[qinvokable]
        fn start(self: Pin<&mut HostLink>, data_dir: &QString);

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

use core::pin::Pin;
use curia_client::{Incoming, Link};
use curia_core::protocol::Call;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct HostLinkRust {
    connected: bool,
    status: QString,
    runtime: Option<tokio::runtime::Runtime>,
    link: Arc<Mutex<Option<Arc<Link>>>>,
}

impl qobject::HostLink {
    fn start(mut self: Pin<&mut Self>, data_dir: &QString) {
        let runtime = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
            Ok(r) => r,
            Err(e) => {
                self.as_mut().set_status(QString::from(&format!("Could not start: {e}")));
                return;
            }
        };
        let qt = self.qt_thread();
        let slot = self.link.clone();
        let data = std::path::PathBuf::from(data_dir.to_string());
        self.as_mut().set_status(QString::from("Connecting…"));
        runtime.spawn(async move {
            match Link::connect_local(&data).await {
                Ok((link, mut incoming)) => {
                    *slot.lock().unwrap() = Some(Arc::new(link));
                    let _ = qt.queue(|mut h| {
                        h.as_mut().set_connected(true);
                        h.as_mut().set_status(QString::from("Connected"));
                    });
                    while let Some(i) = incoming.recv().await {
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
                    let _ = qt.queue(|mut h| {
                        h.as_mut().set_connected(false);
                        h.as_mut().set_status(QString::from("The host closed the connection"));
                    });
                }
                Err(e) => {
                    let message = QString::from(&format!("{e:#}"));
                    let _ = qt.queue(move |mut h| h.as_mut().set_status(message));
                }
            }
        });
        self.as_mut().rust_mut().runtime = Some(runtime);
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
        let data = self.arg(&QString::from("--data")).to_string();
        let url = std::fs::read_to_string(std::path::Path::new(&data).join("link.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v["url"].as_str().map(|u| u.replace("ws://", "http://").trim_end_matches("/ws").to_string()))
            .unwrap_or_default();
        QString::from(&url)
    }

    fn arg(&self, name: &QString) -> QString {
        let name = name.to_string();
        let args: Vec<String> = std::env::args().collect();
        let value = args.iter().position(|a| *a == name).and_then(|i| args.get(i + 1)).cloned().unwrap_or_default();
        QString::from(&value)
    }
}

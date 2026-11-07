//! Live QML for working on the screens: `curia-desktop --live <qml folder>` runs the window from
//! that folder instead of the copy built into the program, and shows a change to any file there
//! within a moment, without a rebuild (Live.qml).
//!
//! Qt keeps every QML file it has loaded, by address, for the life of the program, and the Qt
//! bindings used here can't empty that store. So each change is copied into a fresh snapshot
//! folder: new addresses, read anew.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type LiveQml = super::LiveQmlRust;

        /// Changes whenever a QML file in the live folder is added, removed or saved.
        #[qinvokable]
        fn stamp(self: &LiveQml) -> QString;

        /// Copies the live folder into a fresh snapshot; the address of its Main.qml, or "" with
        /// the reason on stderr.
        #[qinvokable]
        fn snapshot(self: Pin<&mut LiveQml>) -> QString;

        /// `text` with snapshot addresses turned back into the live folder's, so an error names
        /// the file being edited.
        #[qinvokable]
        #[cxx_name = "toSource"]
        fn to_source(self: &LiveQml, text: &QString) -> QString;

        /// Gives a newly opened window its minimize and maximize abilities (src/native.rs).
        #[qinvokable]
        #[cxx_name = "windowOpened"]
        fn window_opened(self: &LiveQml);
    }
}

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct LiveQmlRust {
    count: u32,
    last: Option<PathBuf>,
}

/// The folder given after `--live`.
fn source() -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == "--live").and_then(|i| args.get(i + 1)).map(PathBuf::from)
}

fn qml_files(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|r| r.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "qml")).collect())
        .unwrap_or_default()
}

/// A folder as a file:/// address, the way Qt writes it.
fn url(dir: &Path) -> String {
    let full = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let s = full.display().to_string().replace('\\', "/");
    format!("file:///{}", s.trim_start_matches("//?/"))
}

impl qobject::LiveQml {
    fn stamp(&self) -> QString {
        let files = source().map(|d| qml_files(&d)).unwrap_or_default();
        let newest = files.iter().filter_map(|f| f.metadata().and_then(|m| m.modified()).ok()).max();
        QString::from(&format!("{}:{newest:?}", files.len()))
    }

    fn snapshot(mut self: Pin<&mut Self>) -> QString {
        let Some(from) = source() else {
            eprintln!("live: no folder after --live");
            return QString::default();
        };
        let n = self.as_ref().count + 1;
        let base = std::env::temp_dir().join("curia-live-qml");
        if n == 1 {
            let _ = std::fs::remove_dir_all(&base); // snapshots left by earlier runs
        }
        let to = base.join(format!("{}-{n}", std::process::id()));
        let copied = std::fs::create_dir_all(&to).and_then(|_| {
            for f in qml_files(&from) {
                std::fs::copy(&f, to.join(f.file_name().unwrap_or_default()))?;
            }
            Ok(())
        });
        if let Err(e) = copied {
            eprintln!("live: could not copy {} to {}: {e}", from.display(), to.display());
            return QString::default();
        }
        // The window before the last one is gone by now; its snapshot can go too.
        let mut state = self.as_mut().rust_mut();
        if let Some(old) = state.last.replace(to.clone()) {
            let _ = std::fs::remove_dir_all(old.with_file_name(format!("{}-{}", std::process::id(), n.saturating_sub(2))));
        }
        state.count = n;
        QString::from(&format!("{}/Main.qml", url(&to)))
    }

    fn to_source(&self, text: &QString) -> QString {
        let (Some(last), Some(from)) = (&self.last, source()) else { return text.clone() };
        QString::from(&text.to_string().replace(&url(last), &url(&from)))
    }

    fn window_opened(&self) {
        crate::native::restore_window_abilities();
    }
}

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("Curia.Desktop").qml_files([
        "qml/Main.qml",
        "qml/Theme.qml",
        "qml/Sidebar.qml",
        "qml/Conversation.qml",
        "qml/EventRow.qml",
        "qml/Composer.qml",
        "qml/Rail.qml",
        "qml/Files.qml",
        "qml/Inbox.qml",
        "qml/UsagePanel.qml",
        "qml/Picker.qml",
        "qml/NewRun.qml",
        "qml/Driver.qml",
        "qml/Icon.qml",
        "qml/StatusMark.qml",
        "qml/TeamBadge.qml",
        "qml/Btn.qml",
    ]))
    .qt_module("Gui")
    .qt_module("Qml")
    .qt_module("Quick")
    .files(["src/bridge.rs"])
    .build();
}

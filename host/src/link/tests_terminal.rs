//! A terminal end to end over real sockets: one client starts a shell in a thread's folder and
//! types into it; a second client opens the same terminal and sees the same screen.
use crate::host::Host;
use crate::store::NewThread;
use curia_client::{Incoming, Link};
use curia_core::protocol::Call;
use std::time::Duration;

fn text(screen: &serde_json::Value) -> String {
    screen["lines"].as_array().into_iter().flatten()
        .map(|line| line.as_array().into_iter().flatten().map(|r| r["t"].as_str().unwrap_or("")).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Waits until a `terminal` notification's screen contains `needle`.
async fn until_shows(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Incoming>, link: &Link, id: &str, needle: &str) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let shown = text(&link.call(Call::TerminalOpen { id: id.into() }).await.unwrap()["screen"]);
        if shown.contains(needle) {
            return shown;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("the terminal never showed {needle:?}; it shows:\n{shown}");
        }
        let _ = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_shell_runs_in_the_thread_folder_and_two_clients_share_it() {
    let data = std::env::temp_dir().join(format!("curia-term-test-{}", crate::store::new_id()));
    let work = data.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let (host, _) = Host::open(&data).unwrap();
    let t = host
        .create_thread(&NewThread {
            title: "t".into(), cli: "claude".into(), config_dir: "c".into(), cwd: work.to_string_lossy().into_owned(),
            model: None, effort: None, permission_mode: "manual".into(), ..Default::default()
        })
        .unwrap();
    let (_stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(super::serve(host.clone(), 0, None, None, async move {
        let _ = stopped.await;
    }));
    for _ in 0..50 {
        if data.join("link.json").exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let (a, mut a_rx) = Link::connect_local(&data).await.unwrap();
    let (b, mut b_rx) = Link::connect_local(&data).await.unwrap();

    let started = a.call(Call::TerminalStart { thread_id: t.id.clone(), cols: 80, rows: 24 }).await.unwrap();
    let id = started["id"].as_str().unwrap().to_string();
    let listed = b.call(Call::TerminalList { thread_id: t.id.clone() }).await.unwrap();
    assert_eq!(listed[0]["id"], id.as_str(), "the second client can find it");
    a.call(Call::TerminalOpen { id: id.clone() }).await.unwrap();
    b.call(Call::TerminalOpen { id: id.clone() }).await.unwrap();

    // Typed by one client and run by the shell (the answer, 42, isn't in what was typed);
    // seen by both; the shell runs in the thread's folder.
    let sum = if cfg!(windows) { "Write-Output (6*7)\r" } else { "echo $((6*7))\r" };
    a.call(Call::TerminalInput { id: id.clone(), data: sum.into() }).await.unwrap();
    let shown = until_shows(&mut a_rx, &a, &id, "\n42").await;
    assert!(!shown.contains(">> "), "the line waited for more instead of running:\n{shown}");
    // The cursor sits on the last line with text (the new prompt), not below it.
    tokio::time::sleep(Duration::from_millis(800)).await;
    let view = a.call(Call::TerminalOpen { id: id.clone() }).await.unwrap();
    let lines: Vec<String> = text(&view["screen"]).split('\n').map(str::to_string).collect();
    let prompt = lines.iter().rposition(|l| !l.trim().is_empty()).unwrap();
    assert_eq!(view["screen"]["cursor"][0].as_u64().unwrap() as usize, prompt, "cursor {:?} on:\n{}", view["screen"]["cursor"], lines.join("\n"));
    a.call(Call::TerminalInput { id: id.clone(), data: "pwd\r".into() }).await.unwrap();
    let folder = work.file_name().unwrap().to_string_lossy().into_owned();
    until_shows(&mut b_rx, &b, &id, &folder).await;

    a.call(Call::TerminalClose { id: id.clone() }).await.unwrap();
    assert!(b.call(Call::TerminalInput { id, data: "x".into() }).await.is_err(), "a closed terminal takes no input");
    let _ = std::fs::remove_dir_all(&data);
}

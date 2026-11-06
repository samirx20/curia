//! The control room without a live CLI: events are injected as if Claude produced them, and
//! git is real (scratch repositories).
use super::Host;
use crate::git::{merge, run, tests_support::repo, worktree};
use crate::store::{NewThread, ThreadRow};
use curia_core::{Decision, Event, ItemId, MergeReport, TextKind, TurnOutcome};
use std::path::Path;
use std::sync::Arc;

fn host() -> Arc<Host> {
    Host::open(&std::env::temp_dir().join(format!("curia-host-test-{}", crate::store::new_id()))).unwrap().0
}

fn thread(h: &Host, cwd: &Path, parent: Option<&ThreadRow>, team: Option<&str>, branch: Option<&str>) -> ThreadRow {
    h.store
        .thread_create(&NewThread {
            title: "t".into(),
            cli: "claude".into(),
            config_dir: "c".into(),
            cwd: cwd.to_string_lossy().into_owned(),
            permission_mode: "manual".into(),
            team_id: team.map(str::to_string),
            parent_id: parent.map(|p| p.id.clone()),
            depth: parent.map_or(0, |p| p.depth + 1),
            branch: branch.map(str::to_string),
            ..Default::default()
        })
        .unwrap()
}

fn events(h: &Host, thread_id: &str) -> Vec<Event> {
    h.store.events_after(thread_id, 0).unwrap().into_iter().map(|s| s.event).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_finished_team_hands_back_once_merges_and_the_merge_undoes() {
    let (h, r) = (host(), repo());
    let main = thread(&h, &r, None, None, None);
    let wt = r.with_extension("team");
    worktree::add(&r, &wt, "curia/t1", &worktree::head(&r).unwrap()).unwrap();
    let team = thread(&h, &wt, Some(&main), None, Some("curia/t1"));
    h.store.result_expect(&team.id, &main.id).unwrap();
    // The main thread is mid-turn, so the result is queued rather than starting a CLI.
    h.inject(&main.id, &[Event::TurnStarted { prompt: Some("go".into()), by: None }]).unwrap();

    // The team writes a file without committing, says what it did, and never calls report_result.
    std::fs::write(wt.join("team.txt"), "work\n").unwrap();
    let item = ItemId("item-1".into());
    h.inject(&team.id, &[
        Event::TurnStarted { prompt: Some("task".into()), by: None },
        Event::TextStarted { item: item.clone(), kind: TextKind::Text, text: "".into() },
        Event::TextDone { item, text: "I wrote team.txt".into() },
        Event::TurnFinished { outcome: TurnOutcome::Completed, cost_usd: None },
    ]).unwrap();
    let turn = h.store.turns(&team.id).unwrap()[0].id.clone();
    h.hand_back(&team.id, &turn, &TurnOutcome::Completed).await.unwrap();
    h.hand_back(&team.id, &turn, &TurnOutcome::Completed).await.unwrap(); // a second call delivers nothing

    let backs: Vec<_> = events(&h, &main.id).into_iter().filter_map(|e| match e {
        Event::HandBack { summary, inferred, ok, merge, .. } => Some((summary, inferred, ok, merge)),
        _ => None,
    }).collect();
    assert_eq!(backs.len(), 1, "exactly one hand-back");
    let (summary, inferred, ok, report) = &backs[0];
    assert_eq!((summary.as_str(), *inferred, *ok), ("I wrote team.txt", true, true));
    let MergeReport::Merged { merge_id, files, .. } = report else { panic!("expected a merge, got {report:?}") };
    assert_eq!(files, &vec!["team.txt".to_string()]);
    assert!(r.join("team.txt").exists(), "the team's work is in the main folder");
    let queued = h.store.mailbox_take(&main.id).unwrap();
    assert_eq!(queued.len(), 1);
    assert!(queued[0].1.contains("I wrote team.txt") && queued[0].1.contains("merged"));

    h.undo_merge(merge_id).await.unwrap();
    assert!(!r.join("team.txt").exists(), "undo takes it out again");
    assert!(h.undo_merge(merge_id).await.is_err(), "a merge is undone once");
    assert!(events(&h, &main.id).contains(&Event::MergeUndone { merge_id: merge_id.clone() }));
    let _ = worktree::remove(&r, &wt);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_team_turn_is_reported_but_not_merged() {
    let (h, r) = (host(), repo());
    let main = thread(&h, &r, None, None, None);
    let wt = r.with_extension("team");
    worktree::add(&r, &wt, "curia/t2", &worktree::head(&r).unwrap()).unwrap();
    let team = thread(&h, &wt, Some(&main), None, Some("curia/t2"));
    h.store.result_expect(&team.id, &main.id).unwrap();
    h.store.result_report(&team.id, "half done").unwrap();
    h.inject(&main.id, &[Event::TurnStarted { prompt: None, by: None }]).unwrap();
    std::fs::write(wt.join("half.txt"), "x").unwrap();
    merge::commit_leftovers(&wt, "half").unwrap();
    h.inject(&team.id, &[Event::TurnStarted { prompt: None, by: None }]).unwrap();
    let turn = h.store.turns(&team.id).unwrap()[0].id.clone();
    let cut = TurnOutcome::Interrupted;
    h.hand_back(&team.id, &turn, &cut).await.unwrap();
    let back = events(&h, &main.id).into_iter().find(|e| matches!(e, Event::HandBack { .. })).unwrap();
    let Event::HandBack { ok, inferred, merge, .. } = back else { unreachable!() };
    assert!(!ok && !inferred);
    assert!(matches!(merge, MergeReport::Failed { .. }));
    assert!(!r.join("half.txt").exists(), "nothing merged from a cut-off turn");
    let _ = worktree::remove(&r, &wt);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hand_offs_are_refused_too_deep_to_a_busy_team_or_outside_git() {
    let (h, r) = (host(), repo());
    let team = h.store.team_create("Claude b", "claude", "c", None, None).unwrap();
    let main = thread(&h, &r, None, None, None);
    let mid = thread(&h, &r, Some(&main), Some(&team.id), None);
    let deep = thread(&h, &r, Some(&mid), Some(&team.id), None);
    let err = h.hand_off(&deep.id, "Claude b", "x", "task", None).await.unwrap_err();
    assert!(format!("{err:#}").contains("levels deep"), "{err:#}");

    h.inject(&mid.id, &[Event::TurnStarted { prompt: None, by: None }]).unwrap();
    let err = h.hand_off(&main.id, "Claude b", "x", "task", None).await.unwrap_err();
    assert!(format!("{err:#}").contains("busy"), "{err:#}");

    let plain = std::env::temp_dir().join(format!("curia-nogit-{}", &crate::store::new_id()[..8]));
    std::fs::create_dir_all(&plain).unwrap();
    let outside = thread(&h, &plain, None, None, None);
    let other = h.store.team_create("Claude c", "claude", "c", None, None).unwrap();
    let err = h.hand_off(&outside.id, &other.name, "x", "task", None).await.unwrap_err();
    assert!(format!("{err:#}").contains("git repository"), "{err:#}");
    assert!(run(&r, &["branch", "--list", "curia/*"]).unwrap().is_empty(), "refusals leave no branches behind");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ask_human_waits_for_the_answer() {
    let (h, r) = (host(), repo());
    let main = thread(&h, &r, None, None, None);
    let (asker, id) = (h.clone(), main.id.clone());
    let waiting = tokio::spawn(async move { asker.ask_human(&id, "Which colour?", &["Blue".into(), "Red".into()]).await });
    let mut request = None;
    for _ in 0..50 {
        request = events(&h, &main.id).into_iter().find_map(|e| match e {
            Event::ApprovalRequested { request_id, .. } => Some(request_id),
            _ => None,
        });
        if request.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let mut answers = serde_json::Map::new();
    answers.insert("Which colour?".into(), "Blue".into());
    h.answer(&main.id, &request.unwrap(), &Decision::Answer { answers }).await.unwrap();
    let got = waiting.await.unwrap().unwrap();
    assert_eq!(got["answer"], "Blue");
}

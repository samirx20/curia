//! One client's outgoing queue. Writes are batched every 16 ms with neighbouring text pieces
//! merged. A client more than 1,000 items or 8 MB behind is told to reconnect for a snapshot
//! and dropped, so one slow phone can't hold up the host (CARRY_OVER §17).
use axum::extract::ws::{Message, WebSocket};
use curia_core::Event;
use curia_core::protocol::{Notification, WireEvent};
use futures_util::SinkExt;
use futures_util::stream::SplitSink;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{Notify, mpsc};

const MAX_ITEMS: usize = 1000;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const BATCH_WINDOW: Duration = Duration::from_millis(16);

pub enum Out {
    Reply(Value),
    Event(WireEvent),
    /// A ready-made notification.
    Note(Value),
}

#[derive(Clone)]
pub struct Outbox {
    tx: mpsc::UnboundedSender<(Out, usize)>,
    items: Arc<AtomicUsize>,
    bytes: Arc<AtomicUsize>,
    closing: Arc<Mutex<Option<String>>>,
    wake: Arc<Notify>,
}

impl Outbox {
    /// Starts the writer for `sink`; returns the queue that feeds it.
    pub fn start(sink: SplitSink<WebSocket, Message>) -> Outbox {
        let (tx, rx) = mpsc::unbounded_channel();
        let o = Outbox { tx, items: Arc::default(), bytes: Arc::default(), closing: Arc::default(), wake: Arc::default() };
        tokio::spawn(o.clone().write(sink, rx));
        o
    }

    /// Queues one item. A client past its limits is closed instead.
    pub fn push(&self, out: Out) {
        if self.closing.lock().unwrap().is_some() {
            return;
        }
        let size = match &out {
            Out::Reply(v) | Out::Note(v) => v.to_string().len(),
            Out::Event(e) => serde_json::to_string(e).map_or(0, |s| s.len()),
        };
        let items = self.items.fetch_add(1, Ordering::SeqCst) + 1;
        let bytes = self.bytes.fetch_add(size, Ordering::SeqCst) + size;
        if items > MAX_ITEMS || bytes > MAX_BYTES {
            self.close("fell too far behind; reconnect to get a fresh snapshot");
            return;
        }
        let _ = self.tx.send((out, size));
    }

    /// Tells the client why and closes the connection; anything still queued is dropped.
    pub fn close(&self, reason: &str) {
        let mut c = self.closing.lock().unwrap();
        if c.is_none() {
            *c = Some(reason.to_string());
            self.wake.notify_one();
        }
    }

    async fn write(self, mut sink: SplitSink<WebSocket, Message>, mut rx: mpsc::UnboundedReceiver<(Out, usize)>) {
        loop {
            let first = tokio::select! {
                item = rx.recv() => item,
                _ = self.wake.notified() => None,
            };
            let closing = self.closing.lock().unwrap().clone();
            if let Some(reason) = closing {
                let bye = json!({"jsonrpc": "2.0", "method": Notification::CLOSING, "params": {"reason": reason}});
                let _ = sink.send(Message::Text(bye.to_string().into())).await;
                let _ = sink.close().await;
                return;
            }
            let Some(first) = first else { return };
            let mut batch = vec![first];
            let deadline = tokio::time::Instant::now() + BATCH_WINDOW;
            while let Ok(Some(next)) = tokio::time::timeout_at(deadline, rx.recv()).await {
                batch.push(next);
            }
            let (items, bytes) = (batch.len(), batch.iter().map(|(_, s)| s).sum::<usize>());
            let messages = merge(batch.into_iter().map(|(o, _)| o).collect());
            let text = if messages.len() == 1 { messages[0].to_string() } else { Value::Array(messages).to_string() };
            if sink.send(Message::Text(text.into())).await.is_err() {
                return;
            }
            self.items.fetch_sub(items, Ordering::SeqCst);
            self.bytes.fetch_sub(bytes, Ordering::SeqCst);
        }
    }
}

/// Turns a batch into JSON-RPC messages, joining neighbouring text pieces of the same item.
fn merge(batch: Vec<Out>) -> Vec<Value> {
    let mut out: Vec<Out> = Vec::with_capacity(batch.len());
    for o in batch {
        if let (Some(Out::Event(prev)), Out::Event(next)) = (out.last_mut(), &o) {
            if let (Event::TextDelta { item: a, text }, Event::TextDelta { item: b, text: more }) = (&mut prev.event, &next.event) {
                if a == b && prev.thread_id == next.thread_id {
                    text.push_str(more);
                    continue;
                }
            }
        }
        out.push(o);
    }
    out.into_iter()
        .map(|o| match o {
            Out::Reply(v) | Out::Note(v) => v,
            Out::Event(e) => json!({"jsonrpc": "2.0", "method": Notification::EVENT, "params": e}),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use curia_core::ItemId;

    fn delta(item: &str, text: &str) -> Out {
        let event = Event::TextDelta { item: ItemId(item.into()), text: text.into() };
        Out::Event(WireEvent { thread_id: "t".into(), seq: None, event })
    }

    #[test]
    fn neighbouring_text_pieces_are_joined() {
        let merged = merge(vec![delta("a", "Hel"), delta("a", "lo"), delta("b", "x"), Out::Reply(json!({"id": 1}))]);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0]["params"]["event"]["text"], "Hello");
        assert_eq!(merged[1]["params"]["event"]["item"], "b");
    }
}

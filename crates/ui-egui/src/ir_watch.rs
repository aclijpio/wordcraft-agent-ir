//! Debounced watch of the document's Agent IR folder → `doc.syncIr` with keep_view.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde_json::json;

use crate::WordApp;

const POLL: Duration = Duration::from_millis(750);
const DEBOUNCE: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub struct IrWatch {
    path: Option<PathBuf>,
    last_mtime: Option<SystemTime>,
    last_poll: Option<Instant>,
    pending_since: Option<Instant>,
    last_doc: Option<(u64, Option<PathBuf>)>,
}

impl IrWatch {
    pub fn poll(app: &mut WordApp) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            poll_native(app);
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = app;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn poll_native(app: &mut WordApp) {
    let doc_id = app.session.document_id();
    let doc_path = app.session.path.clone();
    let key = (doc_id, doc_path.clone());
    if app.ir_watch.last_doc.as_ref() != Some(&key) {
        app.ir_watch.last_doc = Some(key);
        app.ir_watch.path = None;
        app.ir_watch.last_mtime = None;
        app.ir_watch.pending_since = None;
    }

    let now = Instant::now();
    if app.ir_watch.last_poll.is_some_and(|t| now.duration_since(t) < POLL) {
        return;
    }
    app.ir_watch.last_poll = Some(now);

    let ir_dir = match ir_dir_for(&mut app.session) {
        Some(p) if p.is_dir() => p,
        _ => {
            app.ir_watch.path = None;
            app.ir_watch.last_mtime = None;
            app.ir_watch.pending_since = None;
            return;
        }
    };
    app.ir_watch.path = Some(ir_dir.clone());

    let mtime = folder_mtime(&ir_dir);
    match (mtime, app.ir_watch.last_mtime) {
        (Some(m), Some(prev)) if m > prev => {
            app.ir_watch.last_mtime = Some(m);
            app.ir_watch.pending_since = Some(now);
        }
        (Some(m), None) => {
            app.ir_watch.last_mtime = Some(m);
        }
        (Some(m), Some(_)) => {
            app.ir_watch.last_mtime = Some(m);
        }
        (None, _) => {}
    }

    let Some(since) = app.ir_watch.pending_since else {
        return;
    };
    if now.duration_since(since) < DEBOUNCE {
        return;
    }
    app.ir_watch.pending_since = None;
    let path = ir_dir.to_string_lossy().to_string();
    let _ = app.execute_keep_view("doc.syncIr", json!({ "path": path }));
}

#[cfg(not(target_arch = "wasm32"))]
fn ir_dir_for(s: &mut wordcraft_engine::Session) -> Option<PathBuf> {
    let v = s.run("doc.irPath", &json!({})).ok()?;
    let path = v.get("path").and_then(|x| x.as_str())?;
    Some(PathBuf::from(path))
}

#[cfg(not(target_arch = "wasm32"))]
fn folder_mtime(dir: &Path) -> Option<SystemTime> {
    let mut latest: Option<SystemTime> = None;
    let mut stack = vec![dir.to_path_buf()];
    let mut n = 0usize;
    while let Some(cur) = stack.pop() {
        let entries = std::fs::read_dir(&cur).ok()?;
        for entry in entries.flatten() {
            n = n.saturating_add(1);
            if n > 5000 {
                return latest;
            }
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let Ok(m) = meta.modified() else { continue };
            latest = Some(match latest {
                Some(prev) if prev >= m => prev,
                _ => m,
            });
        }
    }
    latest
}

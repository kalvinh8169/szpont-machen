use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};

use crate::adapters;
use crate::allowlist::SessionAllowlist;
use crate::app::AppEvent;
use crate::core::snapshot::collect_sessions_filtered;
use crate::store::Store;

pub enum ScanCommand {
    Refresh,
    Quit,
}

pub struct ScannerHandle {
    cmd_tx: Sender<ScanCommand>,
}

impl ScannerHandle {
    pub fn refresh(&self) {
        let _ = self.cmd_tx.send(ScanCommand::Refresh);
    }

    pub fn quit(&self) {
        let _ = self.cmd_tx.send(ScanCommand::Quit);
    }
}

pub fn spawn(
    db_path: Option<PathBuf>,
    refresh_secs: u64,
    no_watch: bool,
    allowlist: Option<SessionAllowlist>,
    event_tx: Sender<AppEvent>,
) -> ScannerHandle {
    let (cmd_tx, cmd_rx) = channel();
    let watcher_tx = cmd_tx.clone();
    std::thread::spawn(move || {
        run_scanner(
            db_path.as_deref(),
            refresh_secs,
            no_watch,
            allowlist.as_ref(),
            &event_tx,
            &cmd_rx,
            watcher_tx,
        );
    });
    ScannerHandle { cmd_tx }
}

fn run_scanner(
    db_path: Option<&std::path::Path>,
    refresh_secs: u64,
    no_watch: bool,
    allowlist: Option<&SessionAllowlist>,
    event_tx: &Sender<AppEvent>,
    cmd_rx: &Receiver<ScanCommand>,
    watcher_tx: Sender<ScanCommand>,
) {
    let mut store = match Store::open(db_path) {
        Ok(store) => store,
        Err(err) => {
            let _ = event_tx.send(AppEvent::ScanError(format!("cannot open store: {err:#}")));
            return;
        }
    };
    let _watcher = if no_watch {
        None
    } else {
        setup_watcher(watcher_tx)
    };
    let refresh_interval = Duration::from_secs(refresh_secs.max(1));
    if allowlist.is_none() {
        let cached = crate::limits::load_cached(&store);
        if !cached.is_empty() {
            let _ = event_tx.send(AppEvent::Limits(cached));
        }
    }
    let mut last_limits_at: i64 = 0;
    loop {
        let started = std::time::Instant::now();
        let mut report = |message: &str| {
            let _ = event_tx.send(AppEvent::Progress(message.to_string()));
        };
        let scanned = match collect_sessions_filtered(&mut store, &mut report, allowlist) {
            Ok(rows) => {
                let count = rows.len();
                if event_tx.send(AppEvent::Snapshot(rows)).is_err() {
                    return;
                }
                Some(count)
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::ScanError(format!("{err:#}")));
                None
            }
        };
        let now = crate::core::now_ms();
        if allowlist.is_none() && now - last_limits_at >= 60_000 {
            last_limits_at = now;
            let _ = event_tx.send(AppEvent::Progress("scan: probing limits…".to_string()));
            let limits = crate::limits::collect(&store);
            if event_tx.send(AppEvent::Limits(limits)).is_err() {
                return;
            }
        }
        if let Some(count) = scanned {
            let _ = event_tx.send(AppEvent::ScanDone(format!(
                "scan finished: {count} sessions in {}ms",
                started.elapsed().as_millis()
            )));
        }
        match cmd_rx.recv_timeout(refresh_interval) {
            Ok(ScanCommand::Quit) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(ScanCommand::Refresh) => {
                std::thread::sleep(Duration::from_millis(250));
                loop {
                    match cmd_rx.try_recv() {
                        Ok(ScanCommand::Quit) => return,
                        Ok(ScanCommand::Refresh) => {}
                        Err(_) => break,
                    }
                }
                if let Some(remaining) = MIN_SCAN_GAP.checked_sub(started.elapsed()) {
                    std::thread::sleep(remaining);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

const MIN_SCAN_GAP: Duration = Duration::from_secs(3);

fn setup_watcher(cmd_tx: Sender<ScanCommand>) -> Option<notify::RecommendedWatcher> {
    let mut watcher =
        notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
            Ok(_) => {
                let _ = cmd_tx.send(ScanCommand::Refresh);
            }
            Err(err) => crate::logging::warn(&format!(
                "file watcher error (falling back to periodic rescans): {err}"
            )),
        })
        .ok()?;
    for adapter in adapters::installed() {
        for path in adapter.watch_paths() {
            if path.exists()
                && let Err(err) = watcher.watch(&path, RecursiveMode::Recursive)
            {
                crate::logging::warn(&format!("cannot watch {}: {err}", path.display()));
            }
        }
    }
    Some(watcher)
}

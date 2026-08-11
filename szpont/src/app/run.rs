use std::io;
use std::process::Command;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use crossterm::cursor;
use crossterm::event::{Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use super::{Action, App, AppEvent, screens};
use crate::cli::Cli;
use crate::core::LaunchSpec;
use crate::scanner::{self, ScannerHandle};
use crate::store::Store;

pub fn run(cli: &Cli) -> anyhow::Result<()> {
    let allowlist = cli
        .session_allowlist
        .as_deref()
        .map(crate::allowlist::SessionAllowlist::load)
        .transpose()?;
    let store = Store::open(cli.db.as_deref())?;
    let (repo, repo_forced) = resolve_repo_context(cli);
    let (event_tx, event_rx) = channel();
    let scanner = scanner::spawn(
        cli.db.clone(),
        cli.refresh_secs,
        cli.no_watch,
        allowlist.clone(),
        event_tx,
    );
    install_panic_hook();
    let mut terminal = setup_terminal()?;
    let mut app = App::new(store, repo, repo_forced);
    if let Some(allowlist) = &allowlist {
        app.allowlist_mode = true;
        app.limits_disabled = true;
        app.status = Some(format!(
            "promo allowlist: {} enabled sessions",
            allowlist.len()
        ));
    }
    let result = event_loop(&mut terminal, &mut app, &event_rx, &scanner);
    restore_terminal(&mut terminal)?;
    scanner.quit();
    result
}

fn resolve_repo_context(cli: &Cli) -> (Option<crate::core::repo::RepoContext>, bool) {
    if cli.global {
        return (None, false);
    }
    if let Some(path) = &cli.repo {
        let ctx = crate::core::repo::detect(path)
            .unwrap_or_else(|| crate::core::repo::plain_directory_context(path));
        return (Some(ctx), true);
    }
    let cwd = std::env::current_dir().ok();
    let ctx = cwd.as_deref().and_then(crate::core::repo::detect);
    (ctx, false)
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    event_rx: &Receiver<AppEvent>,
    scanner: &ScannerHandle,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|frame| screens::draw(frame, app))?;
        if crossterm::event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = crossterm::event::read()?
            && key.kind == KeyEventKind::Press
        {
            match app.handle_key(key) {
                Action::Quit => return Ok(()),
                Action::Refresh => {
                    app.apply_next_snapshot = true;
                    scanner.refresh();
                }
                Action::Launch { spec, exec } => {
                    if exec {
                        restore_terminal(terminal)?;
                        scanner.quit();
                        return exec_replace(&spec);
                    }
                    let outcome = suspend_and_run(terminal, &spec);
                    app.status = Some(outcome);
                    app.apply_next_snapshot = true;
                    scanner.refresh();
                }
                Action::None => {}
            }
        }
        while let Ok(event) = event_rx.try_recv() {
            match event {
                AppEvent::Snapshot(rows) => app.apply_snapshot(rows),
                AppEvent::Limits(limits) => app.limits = limits,
                AppEvent::Progress(message) => {
                    app.scan_status = Some(message);
                    app.scanning = true;
                }
                AppEvent::ScanDone(message) => {
                    app.scan_status = Some(message);
                    app.scanning = false;
                }
                AppEvent::ScanError(message) => {
                    app.status = Some(message);
                    app.scanning = false;
                }
            }
        }
    }
}

fn suspend_and_run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    spec: &LaunchSpec,
) -> String {
    if let Err(err) = restore_terminal(terminal) {
        return format!("terminal error: {err:#}");
    }
    let status = launch_command(spec).status();
    let outcome = match status {
        Ok(status) if status.success() => format!("{} exited", spec.program),
        Ok(status) => format!("{} exited with {status}", spec.program),
        Err(err) => format!("failed to launch {}: {err}", spec.program),
    };
    if let Err(err) = enter_tui(terminal) {
        return format!("terminal error: {err:#}");
    }
    outcome
}

fn exec_replace(spec: &LaunchSpec) -> anyhow::Result<()> {
    use std::os::unix::process::CommandExt;
    let err = launch_command(spec).exec();
    Err(anyhow::anyhow!("failed to exec {}: {err}", spec.program))
}

fn launch_command(spec: &LaunchSpec) -> Command {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args).current_dir(&spec.cwd);
    if let Some(path) = sanitized_path_env() {
        command.env("PATH", path);
    }
    command
}

fn sanitized_path_env() -> Option<std::ffi::OsString> {
    let paths = std::env::var_os("PATH")?;
    let filtered = std::env::split_paths(&paths)
        .filter(|p| !p.as_os_str().is_empty() && p != std::path::Path::new("."));
    std::env::join_paths(filtered).ok()
}

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)?;
    Ok(())
}

fn enter_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen, cursor::Hide)?;
    terminal.clear()?;
    Ok(())
}

fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
        original(info);
    }));
}

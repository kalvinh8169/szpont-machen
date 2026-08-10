use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static SINK: OnceLock<Sink> = OnceLock::new();

struct Sink {
    file: Option<Mutex<std::fs::File>>,
    stderr_allowed: bool,
}

pub fn init(log_path: Option<&Path>, stderr_allowed: bool) {
    use std::os::unix::fs::OpenOptionsExt;
    let file = log_path.and_then(|path| {
        OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(path)
            .ok()
            .map(|file| {
                crate::core::restrict_permissions(path, 0o600);
                Mutex::new(file)
            })
    });
    let _ = SINK.set(Sink {
        file,
        stderr_allowed,
    });
}

pub fn warn(message: &str) {
    let message = crate::core::sanitize_for_terminal(message);
    let Some(sink) = SINK.get() else {
        eprintln!("warning: {message}");
        return;
    };
    if let Some(file) = &sink.file
        && let Ok(mut file) = file.lock()
    {
        let _ = writeln!(file, "[{}] {message}", crate::core::now_ms());
    }
    if sink.stderr_allowed {
        eprintln!("warning: {message}");
    }
}

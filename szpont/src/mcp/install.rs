use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const MAX_FAILURE_OUTPUT_BYTES: usize = 400;

pub fn install(tool: &str, dry_run: bool) -> anyhow::Result<()> {
    let szpont = std::env::current_exe()?;
    let szpont = std::fs::canonicalize(&szpont).unwrap_or(szpont);
    if !szpont.is_absolute() {
        anyhow::bail!(
            "cannot resolve an absolute path to the szpont binary (got {})",
            szpont.display()
        );
    }
    if is_build_tree_path(&szpont) {
        println!(
            "warning: registering a build-tree binary ({}); run cargo install --path . and re-run install-mcp for a stable path",
            szpont.display()
        );
    }
    let targets: Vec<&str> = match tool {
        "all" => vec!["claude", "codex", "kimi"],
        other => vec![other],
    };
    for target in targets {
        match target {
            "claude" => install_via_command(
                "claude",
                &["mcp", "add", "--scope", "user", "szpont", "--"],
                &szpont,
                dry_run,
            ),
            "codex" => {
                install_via_command("codex", &["mcp", "add", "szpont", "--"], &szpont, dry_run);
            }
            "kimi" => install_kimi(&szpont, dry_run)?,
            other => anyhow::bail!("unknown tool {other:?}, expected claude, codex, kimi or all"),
        }
    }
    Ok(())
}

fn install_via_command(program: &str, prefix_args: &[&str], szpont: &Path, dry_run: bool) {
    let mut display = vec![program.to_string()];
    display.extend(prefix_args.iter().map(std::string::ToString::to_string));
    display.push(szpont.display().to_string());
    display.push("mcp".to_string());
    let command_line = display.join(" ");
    if dry_run {
        println!("would run: {command_line}");
        return;
    }
    let output = Command::new(program)
        .args(prefix_args)
        .arg(szpont)
        .arg("mcp")
        .output();
    match output {
        Ok(output) if output.status.success() => {
            println!("{program}: registered szpont MCP server");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let combined = format!("{} {}", stdout.trim(), stderr.trim());
            println!(
                "{program}: registration failed ({}): {} — re-run `{command_line}` manually for the full output",
                output.status,
                crate::core::sanitize_for_terminal(&crate::core::truncate(
                    combined.trim(),
                    MAX_FAILURE_OUTPUT_BYTES
                )),
            );
        }
        Err(err) => {
            println!("{program}: cannot run ({err}); is it installed?");
        }
    }
}

fn is_build_tree_path(path: &Path) -> bool {
    let components: Vec<_> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    components
        .windows(2)
        .any(|w| w[0] == "target" && (w[1] == "debug" || w[1] == "release"))
}

fn install_kimi(szpont: &Path, dry_run: bool) -> anyhow::Result<()> {
    let Some(root) = crate::core::ToolId::Kimi.home() else {
        anyhow::bail!("cannot resolve home directory");
    };
    if !root.exists() {
        println!("kimi: ~/.kimi-code not found; skipping");
        return Ok(());
    }
    let path = root.join("mcp.json");
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        anyhow::bail!("{} is a symlink; refusing to rewrite it", path.display());
    }
    let entry = json!({
        "command": szpont.display().to_string(),
        "args": ["mcp"]
    });
    if dry_run {
        println!(
            "would merge into {}: {{\"mcpServers\":{{\"szpont\":{entry}}}}}",
            path.display()
        );
        return Ok(());
    }
    let existing = crate::core::read_to_string_capped(&path, crate::core::MAX_CONFIG_BYTES);
    let mut config: Value = match &existing {
        Some(text) => serde_json::from_str(text)
            .map_err(|err| anyhow::anyhow!("{} is not valid JSON: {err}", path.display()))?,
        None if path.exists() => {
            anyhow::bail!("{} is unreadable or larger than 1MB", path.display())
        }
        None => json!({}),
    };
    if !config.is_object() {
        anyhow::bail!("{} does not contain a JSON object", path.display());
    }
    let backup = existing.as_ref().map(|text| -> anyhow::Result<PathBuf> {
        let backup = backup_path(&path);
        write_private_file(&backup, text)?;
        Ok(backup)
    });
    let backup = backup.transpose()?;
    let servers = config
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    if !servers.is_object() {
        anyhow::bail!("mcpServers in {} is not an object", path.display());
    }
    servers
        .as_object_mut()
        .unwrap()
        .insert("szpont".to_string(), entry);
    write_private_file(&path, &serde_json::to_string_pretty(&config)?)?;
    match backup {
        Some(backup) => println!(
            "kimi: wrote szpont entry to {} (previous file kept at {})",
            path.display(),
            backup.display()
        ),
        None => println!("kimi: wrote szpont entry to {}", path.display()),
    }
    Ok(())
}

fn write_private_file(path: &Path, content: &str) -> anyhow::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        anyhow::bail!("{} is a symlink; refusing to write it", path.display());
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(content.as_bytes())?;
    crate::core::restrict_permissions(path, 0o600);
    Ok(())
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.bak")
}

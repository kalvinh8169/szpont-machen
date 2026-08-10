use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "szpont",
    version,
    about = "szpont machen (pronounced \"shpont mah-khen\") — monitor, resume and archive AI CLI tool sessions"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[arg(long, global = true, help = "Path to the szpont database")]
    pub db: Option<PathBuf>,

    #[arg(long, global = true, default_value_t = 15)]
    pub refresh_secs: u64,

    #[arg(long, global = true)]
    pub no_watch: bool,

    #[arg(long, global = true, help = "Append logs to this file")]
    pub log: Option<PathBuf>,

    #[arg(long, conflicts_with = "repo", help = "Open the global monitor view")]
    pub global: bool,

    #[arg(long, help = "Open the repo view for this path")]
    pub repo: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "List sessions (headless)")]
    Sessions(SessionsArgs),
    #[command(about = "Mark a session as completed")]
    Complete { tool: String, session_id: String },
    #[command(about = "Reopen a completed session")]
    Reopen { tool: String, session_id: String },
    #[command(about = "Show rate-limit usage (headless)")]
    Limits {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Run the MCP server on stdio")]
    Mcp,
    #[command(about = "Register the szpont MCP server with the CLI tools")]
    InstallMcp {
        #[arg(long, default_value = "all")]
        tool: String,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Print shell completions (bash, zsh, fish, elvish, powershell)")]
    Completions { shell: clap_complete::Shell },
}

#[derive(Args)]
pub struct SessionsArgs {
    #[arg(long)]
    pub json: bool,
    #[arg(long, help = "Include completed sessions")]
    pub all: bool,
    #[arg(long, help = "Only sessions for this repo path")]
    pub repo: Option<PathBuf>,
}

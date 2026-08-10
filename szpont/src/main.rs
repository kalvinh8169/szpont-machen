mod adapters;
mod app;
mod cli;
mod commands;
mod core;
mod limits;
mod logging;
mod mcp;
mod scanner;
mod store;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    let stderr_allowed = !matches!(args.command, None | Some(cli::Command::Mcp));
    logging::init(args.log.as_deref(), stderr_allowed);
    match &args.command {
        Some(cli::Command::Sessions(sessions_args)) => commands::sessions(&args, sessions_args),
        Some(cli::Command::Complete { tool, session_id }) => {
            commands::complete(&args, tool, session_id)
        }
        Some(cli::Command::Reopen { tool, session_id }) => {
            commands::reopen(&args, tool, session_id)
        }
        Some(cli::Command::Limits { json }) => commands::limits(&args, *json),
        Some(cli::Command::Mcp) => mcp::serve(&args),
        Some(cli::Command::InstallMcp { tool, dry_run }) => mcp::install(tool, *dry_run),
        Some(cli::Command::Completions { shell }) => {
            use clap::CommandFactory;
            clap_complete::generate(
                *shell,
                &mut cli::Cli::command(),
                "szpont",
                &mut std::io::stdout(),
            );
            Ok(())
        }
        None => app::run::run(&args),
    }
}

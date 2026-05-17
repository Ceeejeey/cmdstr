use clap::{Parser, Subcommand};
use cmd_store::cli;

#[derive(Parser)]
#[command(name = "cmdstr", about = "Smart command storage and recall for the terminal")]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    /// Tag name to look up and run its associated command
    tag: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Capture a command (called automatically by shell hook)
    Capture {
        command: String,
        exit_code: i32,
        duration_ms: i64,
        cwd: String,
        session_id: String,
    },

    /// Manually add a command to the store
    Add(cli::add::AddArgs),

    /// Search and list commands
    #[command(alias = "list")]
    Search(cli::query::QueryArgs),

    /// Tag a stored command
    Tag(cli::tag::TagArgs),

    /// Add a note to a command
    #[command(alias = "note")]
    Annotate(cli::annotate::AnnotateArgs),

    /// Show statistics about stored commands
    Stats(cli::stats::StatsArgs),

    /// Install shell hooks
    Install(cli::install::InstallArgs),

    /// Export commands to JSON or CSV
    Export(cli::export::ExportArgs),

    /// Look up a tag and run the associated command
    Run {
        /// Tag name to look up and execute
        tag: String,
    },

    /// Launch the interactive TUI
    #[command(alias = "dashboard")]
    Tui,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Capture { command, exit_code, duration_ms, cwd, session_id }) => {
            cmd_store::capture::capture_command(&command, exit_code, duration_ms, &cwd, &session_id)?;
        }
        Some(Command::Add(args)) => cli::add::execute(&args)?,
        Some(Command::Search(args)) => cli::query::execute(&args)?,
        Some(Command::Tag(args)) => cli::tag::execute(&args)?,
        Some(Command::Annotate(args)) => cli::annotate::execute(&args)?,
        Some(Command::Stats(args)) => cli::stats::execute(&args)?,
        Some(Command::Install(args)) => cli::install::execute(&args)?,
        Some(Command::Export(args)) => cli::export::execute(&args)?,
        Some(Command::Run { tag }) => cli::run::execute(&tag)?,
        Some(Command::Tui) => cmd_store::tui::run()?,
        None => {
            if let Some(tag) = &cli.tag {
                cli::run::execute(tag)?;
            } else {
                // No subcommand and no tag — show help
                let mut app = clap::Command::new("cmdstr")
                    .about("Smart command storage and recall for the terminal");
                app.print_help()?;
                println!();
            }
        }
    }

    Ok(())
}

use clap::{Parser, Subcommand};
use cmd_store::cli;

#[derive(Parser)]
#[command(
    name = "cmdstr", 
    version, 
    about = "Smart command storage and recall for the terminal",
    long_about = "cmdstr — Smart Command Storage & Recall for the Terminal\n\n\
                  FEATURES:\n  \
                  ✦ Auto-Capture    Shell hooks record every command with full metadata\n  \
                  ✦ Rich Search     Fuzzy text, tag, date, and failure-state filtering\n  \
                  ✦ Tags & Notes    Organize with tags, annotate with explanations\n  \
                  ✦ Bookmarks       Star important commands for instant recall\n  \
                  ✦ Analytics       Frequency stats, failure rates, and top commands\n  \
                  ✦ Export          Serialize full history to JSON or CSV format\n  \
                  ✦ Interactive TUI Full-screen dashboard with keyboard navigation\n\n\
                  Metadata tracked per command: exit code, runtime duration,\n  \
                  working directory, hostname, session ID, and timestamp.",
    after_help = "QUICK START:\n\n  \
       $ cmdstr install                Setup shell hooks (run once after install)\n  \
       $ cmdstr tui                    Launch interactive dashboard\n  \
       $ cmdstr search \"docker\"        Find commands by keyword\n  \
       $ cmdstr search --failed        List commands that exited with errors\n  \
       $ cmdstr stats                  View usage analytics\n  \
       $ cmdstr export -o history.json Export full history to JSON\n  \
       $ cmdstr export -f csv -o h.csv Export full history to CSV\n\n\
     WORKFLOW EXAMPLE:\n\n  \
       $ cmdstr tag \"docker run -p 8080:80 nginx\" \"webserver,nginx\"\n  \
       $ cmdstr annotate 'docker run' \"Launches Nginx proxy for dev\"\n  \
       $ cmdstr webserver              Execute via tag alias"
)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    /// Tag name to look up and run its associated command
    #[arg(help = "The tag alias of the command to execute. Running 'cmdstr <tag>' instantly locates and triggers it.")]
    tag: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Capture a command execution context (invoked automatically by shell hook)
    #[command(
        about = "Capture a command (called automatically by shell hook)",
        long_about = "Automated internal command called by shell environment startup hooks to capture \
                      metadata such as raw command text, shell exit status, command duration in milliseconds, \
                      the active directory at runtime, and current session/terminal ID.",
        after_help = "MANUAL USAGE:\n  \
           $ cmdstr capture \"npm run dev\" 0 450 \"/home/user/project\" \"session-12345\""
    )]
    Capture {
        /// Raw executed command text string
        command: String,
        /// Exit status returned by executed command process
        exit_code: i32,
        /// Command runtime execution duration in milliseconds
        duration_ms: i64,
        /// Current working directory where the command was executed
        cwd: String,
        /// Unique terminal terminal/session ID
        session_id: String,
    },

    /// Manually record a command snippet into the store
    Add(cli::add::AddArgs),

    /// Search, filter, and list recorded commands
    #[command(
        alias = "list",
        about = "Search and list commands",
        long_about = "Scan, search, and list command histories using fuzzy queries, tags, runtime filters, \
                      failed statuses, or bookmarks. Supports both tabular text output and pretty JSON formatting."
    )]
    Search(cli::query::QueryArgs),

    /// Associate custom tags with a command
    Tag(cli::tag::TagArgs),

    /// Annotate a command with explanation notes and bookmarks
    #[command(alias = "note")]
    Annotate(cli::annotate::AnnotateArgs),

    /// View usage frequencies and capture history analytics
    Stats(cli::stats::StatsArgs),

    /// Automatically configure your shell environment startup profiles
    Install(cli::install::InstallArgs),

    /// Export history database items to JSON or CSV format
    Export(cli::export::ExportArgs),

    /// Execute a stored command using its associated tag alias
    #[command(
        about = "Look up a tag and run the associated command",
        long_about = "Find a stored command by tagging alias and run it. Prompts for verification \
                      before execution for safety.",
        after_help = "EXAMPLES:\n  \
           $ cmdstr run db-migration\n  \
           $ cmdstr run build-release"
    )]
    Run {
        /// Tag name/alias of the command to execute
        tag: String,
    },

    /// Launch the premium interactive terminal user interface (TUI)
    #[command(
        alias = "dashboard",
        about = "Launch the interactive TUI",
        long_about = "Opens the full-screen terminal UI dashboard. Features dual-pane navigation (Tab), \
                      clipboard copy support (c), interactive command search (/), tagging controls (t), \
                      annotation/note editing (n), sudo toggle elevation (S), and a full user guide (?)."
    )]
    Tui,
}

fn print_custom_error(err: clap::Error) {
    let args: Vec<String> = std::env::args().collect();
    
    // Print a clean, beautifully formatted error block
    eprintln!("\x1b[1;31mError:\x1b[0m {}", err.to_string().trim());
    
    // Extract the entered subcommand from terminal arguments
    let subcommand = args.get(1).map(|s| s.as_str());
    
    eprintln!("\n\x1b[1;36m💡 How to fix it:\x1b[0m");
    match subcommand {
        Some("capture") => {
            eprintln!("  The capture command requires exactly 5 positional arguments.");
            eprintln!("  \x1b[1mExpected Command Structure:\x1b[0m");
            eprintln!("    cmdstr capture <COMMAND> <EXIT_CODE> <DURATION_MS> <CWD> <SESSION_ID>");
            eprintln!("\n  \x1b[1mQuick Copy-Paste Example:\x1b[0m");
            eprintln!("    $ cmdstr capture \"git status\" 0 45 \"/home/user\" \"session-9876\"");
        }
        Some("add") => {
            eprintln!("  The add command manually records a command and accepts optional tags or notes.");
            eprintln!("  \x1b[1mExpected Command Structure:\x1b[0m");
            eprintln!("    cmdstr add <COMMAND> [-t <TAGS>] [-n <NOTE>] [--exit-code <CODE>] [--duration <MS>]");
            eprintln!("\n  \x1b[1mQuick Copy-Paste Example:\x1b[0m");
            eprintln!("    $ cmdstr add \"cargo build --release\" --tag release --note \"Build release binary\"");
        }
        Some("search") | Some("list") => {
            eprintln!("  The search command searches your command history.");
            eprintln!("  \x1b[1mExpected Command Structure:\x1b[0m");
            eprintln!("    cmdstr search [QUERY] [-t <TAG>] [--last <HOURS>] [--failed] [--freq] [--bookmarks] [--json] [-l <LIMIT>]");
            eprintln!("\n  \x1b[1mQuick Copy-Paste Example:\x1b[0m");
            eprintln!("    $ cmdstr search \"ffmpeg\" --failed --last 24");
        }
        Some("tag") => {
            eprintln!("  The tag command associates tags with a stored command.");
            eprintln!("  \x1b[1mExpected Command Structure:\x1b[0m");
            eprintln!("    cmdstr tag <COMMAND_ID_OR_TEXT> <TAGS>");
            eprintln!("\n  \x1b[1mQuick Copy-Paste Example:\x1b[0m");
            eprintln!("    $ cmdstr tag \"git commit\" git,vcs");
        }
        Some("annotate") | Some("note") => {
            eprintln!("  The annotate command attaches notes and bookmark flags to a command.");
            eprintln!("  \x1b[1mExpected Command Structure:\x1b[0m");
            eprintln!("    cmdstr annotate <COMMAND_ID> <NOTE> [--bookmark]");
            eprintln!("\n  \x1b[1mQuick Copy-Paste Example:\x1b[0m");
            eprintln!("    $ cmdstr annotate 01H6W4A5 \"Starts postgres database\" --bookmark");
        }
        Some("stats") => {
            eprintln!("  The stats command displays analytical logs of your command history.");
            eprintln!("  \x1b[1mExpected Command Structure:\x1b[0m");
            eprintln!("    cmdstr stats [--json]");
            eprintln!("\n  \x1b[1mQuick Copy-Paste Example:\x1b[0m");
            eprintln!("    $ cmdstr stats --json");
        }
        Some("install") => {
            eprintln!("  The install command configures automatic tracking shell hooks.");
            eprintln!("  \x1b[1mExpected Command Structure:\x1b[0m");
            eprintln!("    cmdstr install [-s <SHELL>] [--bin-path <PATH>]");
            eprintln!("\n  \x1b[1mQuick Copy-Paste Example:\x1b[0m");
            eprintln!("    $ cmdstr install --shell zsh");
        }
        Some("export") => {
            eprintln!("  The export command serializes command logs to CSV or JSON files.");
            eprintln!("  \x1b[1mExpected Command Structure:\x1b[0m");
            eprintln!("    cmdstr export [-f <json|csv>] [-o <FILE>]");
            eprintln!("\n  \x1b[1mQuick Copy-Paste Example:\x1b[0m");
            eprintln!("    $ cmdstr export --format csv --output history.csv");
        }
        Some("run") => {
            eprintln!("  The run command executes a command matching the specified tag alias.");
            eprintln!("  \x1b[1mExpected Command Structure:\x1b[0m");
            eprintln!("    cmdstr run <TAG>");
            eprintln!("\n  \x1b[1mQuick Copy-Paste Example:\x1b[0m");
            eprintln!("    $ cmdstr run webserver");
        }
        Some("tui") | Some("dashboard") => {
            eprintln!("  The tui command launches the premium interactive dashboard.");
            eprintln!("  \x1b[1mExpected Command Structure:\x1b[0m");
            eprintln!("    cmdstr tui");
            eprintln!("\n  \x1b[1mQuick Copy-Paste Example:\x1b[0m");
            eprintln!("    $ cmdstr tui");
        }
        _ => {
            eprintln!("  Unrecognized command or incorrect structure.");
            eprintln!("  To see all available commands, run:  \x1b[1mcmdstr --help\x1b[0m");
            eprintln!("  To launch the interactive TUI, run:  \x1b[1mcmdstr tui\x1b[0m");
        }
    }
    eprintln!();
}

fn main() -> anyhow::Result<()> {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(err) => {
            if !err.use_stderr() {
                // It is a built-in help or version display request
                err.exit();
            } else {
                print_custom_error(err);
                std::process::exit(2);
            }
        }
    };

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

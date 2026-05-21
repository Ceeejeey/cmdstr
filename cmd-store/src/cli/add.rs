use clap::Args;

#[derive(Args)]
#[command(
    about = "Manually record a command snippet into the store",
    long_about = "Injects a command entry manually into the SQL history database. Useful for registering \
                  existing complex commands, scripts, or alias configurations you want to store and tag, \
                  without executing them immediately.",
    after_help = "💡 EXAMPLES & WORKFLOWS:\n\n  \
       1. Store a basic command snippet:\n     \
          $ cmdstr add \"tar -czvf backup.tar.gz /var/www\"\n\n  \
       2. Store with tags and an explanation note:\n     \
          $ cmdstr add \"cargo build --release\" --tag \"rust,release\" --note \"Compiles binary under release target\"\n\n  \
       3. Store failed command to test analytics:\n     \
          $ cmdstr add \"systemctl start nginx\" --exit-code 5 --duration 120"
)]
pub struct AddArgs {
    /// The command text string to record
    #[arg(help = "The literal command line instruction text string to store")]
    pub command: String,

    /// Tags to attach (comma-separated list)
    #[arg(short, long, help = "Optional comma-separated list of tags to associate with this command")]
    pub tag: Option<String>,

    /// Explanation note or description
    #[arg(short, long, help = "A short annotation descriptive note explaining what the command does")]
    pub note: Option<String>,

    /// Simulated process exit code
    #[arg(long, default_value = "0", help = "The exit status code to simulate for the command execution")]
    pub exit_code: i32,

    /// Simulated run duration in milliseconds
    #[arg(long, default_value = "0", help = "The simulated duration in milliseconds for the command runtime")]
    pub duration: i64,
}

pub fn execute(args: &AddArgs) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let id = crate::capture::capture_command(
        &args.command,
        args.exit_code,
        args.duration,
        &cwd,
        "manual",
    )?;

    if let Some(tags) = &args.tag {
        crate::cli::tag::apply_tags(&id, tags.split(',').map(|t| t.trim()))?;
    }

    if let Some(note) = &args.note {
        crate::cli::annotate::set_note(&id, note, false)?;
    }

    println!("Stored: {id}");
    Ok(())
}

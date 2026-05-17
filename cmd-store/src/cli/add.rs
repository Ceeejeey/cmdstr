use clap::Args;

#[derive(Args)]
pub struct AddArgs {
    /// The command to store
    pub command: String,

    /// Tags to attach (comma-separated)
    #[arg(short, long)]
    pub tag: Option<String>,

    /// A note about this command
    #[arg(short, long)]
    pub note: Option<String>,

    /// Exit code
    #[arg(long, default_value = "0")]
    pub exit_code: i32,

    /// Duration in milliseconds
    #[arg(long, default_value = "0")]
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

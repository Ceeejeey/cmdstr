use anyhow::{Context, Result};
use clap::Args;
use std::fs;
use std::path::PathBuf;

#[derive(Args)]
pub struct InstallArgs {
    /// Shell type (auto-detect by default)
    #[arg(short, long)]
    pub shell: Option<String>,

    /// Custom binary path
    #[arg(long)]
    pub bin_path: Option<String>,
}

pub fn execute(args: &InstallArgs) -> Result<()> {
    let bin_path = args
        .bin_path
        .clone()
        .unwrap_or_else(|| "cmdstr".to_string());

    let shell = args
        .shell
        .clone()
        .unwrap_or_else(detect_shell);

    let hook = match shell.as_str() {
        "bash" => generate_bash_hook(&bin_path),
        "zsh" => generate_zsh_hook(&bin_path),
        "fish" => generate_fish_hook(&bin_path),
        _ => anyhow::bail!("unsupported shell: {shell} (supported: bash, zsh, fish)"),
    };

    let rc_path = rc_path_for_shell(&shell)?;
    let existing = fs::read_to_string(&rc_path).unwrap_or_default();

    if existing.contains("cmdstr hook") {
        println!("Hook already installed in {}", rc_path.display());
        return Ok(());
    }

    fs::write(&rc_path, format!("{existing}\n{hook}\n"))
        .context("failed to write to rc file")?;

    println!(
        "✅ Installed cmdstr hook in {}\nRestart your shell or run: source {}",
        rc_path.display(),
        rc_path.display()
    );
    Ok(())
}

fn detect_shell() -> String {
    std::env::var("SHELL")
        .unwrap_or_default()
        .split('/')
        .last()
        .unwrap_or("bash")
        .to_string()
}

fn rc_path_for_shell(shell: &str) -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not find home directory")?;
    match shell {
        "bash" => Ok(home.join(".bashrc")),
        "zsh" => Ok(home.join(".zshrc")),
        "fish" => Ok(home.join(".config/fish/config.fish")),
        _ => anyhow::bail!("unsupported shell"),
    }
}

fn generate_bash_hook(bin: &str) -> String {
    format!(
        r##"
# cmdstr: smart command storage
__cmdstr_session_id="$(uuidgen 2>/dev/null || echo $$-$(date +%s))"

cmdstr_preexec() {{
    __cmdstr_cmd="$1"
    __cmdstr_start="$(date +%s%N)"
}}

cmdstr_precmd() {{
    local exit_code=$?
    if [ -n "$__cmdstr_cmd" ]; then
        local end="$(date +%s%N)"
        local duration=$(( (end - __cmdstr_start) / 1000000 ))
        {bin} capture "$__cmdstr_cmd" "$exit_code" "$duration" "$PWD" "$__cmdstr_session_id" 2>/dev/null || true
    fi
    __cmdstr_cmd=""
}}

preexec_functions+=(cmdstr_preexec)
precmd_functions+=(cmdstr_precmd)
"##,
    )
}

fn generate_zsh_hook(bin: &str) -> String {
    generate_bash_hook(bin)
}

fn generate_fish_hook(bin: &str) -> String {
    format!(
        r##"
# cmdstr: smart command storage
set -g __cmdstr_session_id (uuidgen 2>/dev/null; or echo $fish_pid-(date +%s))

function cmdstr_preexec --on-event fish_preexec
    set -g __cmdstr_cmd $argv[1]
    set -g __cmdstr_start (date +%s%N)
end

function cmdstr_precmd --on-event fish_postexec
    set -l exit_code $status
    if set -q __cmdstr_cmd
        set -l end (date +%s%N)
        set -l duration (math "floor(($end - $__cmdstr_start) / 1000000)" 2>/dev/null; or echo 0)
        {bin} capture "$__cmdstr_cmd" "$exit_code" "$duration" "$PWD" "$__cmdstr_session_id" 2>/dev/null; or true
    end
    set -e __cmdstr_cmd
end
"##,
    )
}

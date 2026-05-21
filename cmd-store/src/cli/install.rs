use anyhow::{Context, Result};
use clap::Args;
use std::fs;
use std::path::PathBuf;

#[derive(Args)]
#[command(
    about = "Safely register and install shell capture hooks",
    long_about = "Automatically configures shell startup profiles to load the background execution tracker hook. \
                  Supported shells include Bash, Zsh, and Fish. It detects your current active shell \
                  environment and writes/appends configuration to ~/.bashrc, ~/.zshrc, or ~/.config/fish/config.fish.",
    after_help = "💡 EXAMPLES & CONFIGS:\n\n  \
       1. Install hook with automatic shell detection (recommended):\n     \
          $ cmdstr install\n\n  \
       2. Explicitly install hook for Zsh shell:\n     \
          $ cmdstr install --shell zsh\n\n  \
       3. Specify custom absolute bin path for hook target:\n     \
          $ cmdstr install --bin-path \"/usr/local/bin/cmdstr\""
)]
pub struct InstallArgs {
    /// Target shell profile type (auto-detect if omitted)
    #[arg(short, long, help = "Configure hooks explicitly for zsh, bash, or fish shell")]
    pub shell: Option<String>,

    /// Custom target binary path
    #[arg(long, help = "Custom absolute binary/executable file path target for the hook to invoke")]
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

    if existing.contains("cmdstr hook") || existing.contains("__cmdstr_session_id") {
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
        .next_back()
        .filter(|s| !s.is_empty())
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
# ── cmdstr hook ──────────────────────────────────────────────
# Auto-capture commands. Works in vanilla bash (no bash-preexec needed).
__cmdstr_session_id="$(uuidgen 2>/dev/null || echo $$-$(date +%s))"
__cmdstr_cmd=""
__cmdstr_start=""

__cmdstr_preexec() {{
    [ -n "$__cmdstr_cmd" ] && return
    local cmd="$BASH_COMMAND"
    case "$cmd" in
        cmdstr\ capture*|__cmdstr_*|_cmdstr_*) return ;;
    esac
    local trimmed="${{cmd#"${{cmd%%[![:space:]]*}}"}}"
    [ -z "$trimmed" ] && return
    __cmdstr_cmd="$cmd"
    __cmdstr_start="$(date +%s%N)"
}}

__cmdstr_precmd() {{
    local exit_code=$?
    if [ -n "$__cmdstr_cmd" ]; then
        local end="$(date +%s%N)"
        local duration=$(( (end - __cmdstr_start) / 1000000 ))
        {bin} capture "$__cmdstr_cmd" "$exit_code" "$duration" "$PWD" "$__cmdstr_session_id" 2>/dev/null || true
    fi
    __cmdstr_cmd=""
}}

trap '__cmdstr_preexec' DEBUG
if [[ "$PROMPT_COMMAND" != *"__cmdstr_precmd"* ]]; then
    PROMPT_COMMAND="__cmdstr_precmd${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}"
fi
# ── end cmdstr hook ──────────────────────────────────────────
"##,
    )
}

fn generate_zsh_hook(bin: &str) -> String {
    format!(
        r##"
# ── cmdstr hook ──────────────────────────────────────────────
# Auto-capture commands using native zsh hooks.
__cmdstr_session_id="$(uuidgen 2>/dev/null || echo $$-$(date +%s))"

autoload -Uz add-zsh-hook

__cmdstr_preexec() {{
    local cmd="$1"
    case "$cmd" in
        cmdstr\ capture*|__cmdstr_*|_cmdstr_*) return ;;
    esac
    local trimmed="${{cmd#"${{cmd%%[![:space:]]*}}"}}"
    [ -z "$trimmed" ] && return
    __cmdstr_cmd="$cmd"
    __cmdstr_start="$(date +%s%N)"
}}

__cmdstr_precmd() {{
    local exit_code=$?
    if [ -n "$__cmdstr_cmd" ]; then
        local end="$(date +%s%N)"
        local duration=$(( (end - __cmdstr_start) / 1000000 ))
        {bin} capture "$__cmdstr_cmd" "$exit_code" "$duration" "$PWD" "$__cmdstr_session_id" 2>/dev/null || true
    fi
    __cmdstr_cmd=""
}}

add-zsh-hook preexec __cmdstr_preexec
add-zsh-hook precmd __cmdstr_precmd
# ── end cmdstr hook ──────────────────────────────────────────
"##,
    )
}

fn generate_fish_hook(bin: &str) -> String {
    format!(
        r##"
# ── cmdstr hook ──────────────────────────────────────────────
# Auto-capture commands using fish events.
set -g __cmdstr_session_id (uuidgen 2>/dev/null; or echo $fish_pid-(date +%s))

function __cmdstr_preexec --on-event fish_preexec
    set -l cmd $argv[1]
    string match -q 'cmdstr capture*' -- $cmd; and return
    string match -q '__cmdstr_*' -- $cmd; and return
    set -l trimmed (string trim -- $cmd)
    test -z "$trimmed"; and return
    set -g __cmdstr_cmd $cmd
    set -g __cmdstr_start (date +%s%N)
end

function __cmdstr_precmd --on-event fish_postexec
    set -l exit_code $status
    if set -q __cmdstr_cmd
        set -l end (date +%s%N)
        set -l duration (math "floor(($end - $__cmdstr_start) / 1000000)" 2>/dev/null; or echo 0)
        {bin} capture "$__cmdstr_cmd" "$exit_code" "$duration" "$PWD" "$__cmdstr_session_id" 2>/dev/null; or true
    end
    set -e __cmdstr_cmd
end
# ── end cmdstr hook ──────────────────────────────────────────
"##,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_generate_bash_hook_contains_bin() {
        let hook = generate_bash_hook("/usr/local/bin/cmdstr");
        assert!(hook.contains("/usr/local/bin/cmdstr capture"));
    }

    #[test]
    fn test_generate_bash_hook_contains_debug_trap() {
        let hook = generate_bash_hook("cmdstr");
        assert!(hook.contains("trap '__cmdstr_preexec' DEBUG"));
        assert!(hook.contains("PROMPT_COMMAND"));
    }

    #[test]
    fn test_generate_bash_hook_session_id() {
        let hook = generate_bash_hook("cmdstr");
        assert!(hook.contains("__cmdstr_session_id"));
    }

    #[test]
    fn test_generate_bash_hook_self_filter() {
        let hook = generate_bash_hook("cmdstr");
        assert!(hook.contains("cmdstr\\ capture*"));
    }

    #[test]
    fn test_generate_zsh_hook_contains_bin() {
        let hook = generate_zsh_hook("/usr/local/bin/cmdstr");
        assert!(hook.contains("/usr/local/bin/cmdstr capture"));
    }

    #[test]
    fn test_generate_zsh_hook_uses_native_hooks() {
        let hook = generate_zsh_hook("cmdstr");
        assert!(hook.contains("autoload -Uz add-zsh-hook"));
        assert!(hook.contains("add-zsh-hook preexec __cmdstr_preexec"));
        assert!(hook.contains("add-zsh-hook precmd __cmdstr_precmd"));
    }

    #[test]
    fn test_generate_zsh_hook_differs_from_bash() {
        // Zsh should use add-zsh-hook, not DEBUG trap
        let zsh = generate_zsh_hook("cmdstr");
        let bash = generate_bash_hook("cmdstr");
        assert!(zsh.contains("add-zsh-hook"));
        assert!(!zsh.contains("trap '__cmdstr_preexec' DEBUG"));
        assert!(bash.contains("trap '__cmdstr_preexec' DEBUG"));
    }

    #[test]
    fn test_generate_fish_hook_contains_bin() {
        let hook = generate_fish_hook("/opt/cmdstr");
        assert!(hook.contains("/opt/cmdstr capture"));
    }

    #[test]
    fn test_generate_fish_hook_contains_events() {
        let hook = generate_fish_hook("cmdstr");
        assert!(hook.contains("--on-event fish_preexec"));
        assert!(hook.contains("--on-event fish_postexec"));
    }

    #[test]
    fn test_generate_fish_hook_self_filter() {
        let hook = generate_fish_hook("cmdstr");
        assert!(hook.contains("cmdstr capture"));
        assert!(hook.contains("__cmdstr_"));
    }

    #[test]
    fn test_detect_shell_from_env() {
        let _g = ENV_LOCK.lock().unwrap();
        let old = std::env::var("SHELL").ok();
        std::env::set_var("SHELL", "/usr/bin/zsh");
        assert_eq!(detect_shell(), "zsh");
        restore_env("SHELL", old);
    }

    #[test]
    fn test_detect_shell_fallback() {
        let _g = ENV_LOCK.lock().unwrap();
        let old = std::env::var("SHELL").ok();
        std::env::remove_var("SHELL");
        assert_eq!(detect_shell(), "bash");
        restore_env("SHELL", old);
    }

    #[test]
    fn test_detect_shell_takes_basename() {
        let _g = ENV_LOCK.lock().unwrap();
        let old = std::env::var("SHELL").ok();
        std::env::set_var("SHELL", "/opt/local/bin/fish");
        assert_eq!(detect_shell(), "fish");
        restore_env("SHELL", old);
    }

    #[test]
    fn test_rc_path_for_bash() {
        let home = dirs::home_dir().expect("home dir");
        assert_eq!(rc_path_for_shell("bash").unwrap(), home.join(".bashrc"));
    }

    #[test]
    fn test_rc_path_for_zsh() {
        let home = dirs::home_dir().expect("home dir");
        assert_eq!(rc_path_for_shell("zsh").unwrap(), home.join(".zshrc"));
    }

    #[test]
    fn test_rc_path_for_fish() {
        let home = dirs::home_dir().expect("home dir");
        assert_eq!(
            rc_path_for_shell("fish").unwrap(),
            home.join(".config/fish/config.fish")
        );
    }

    #[test]
    fn test_rc_path_unsupported_shell() {
        assert!(rc_path_for_shell("tcsh").is_err());
    }

    #[test]
    fn test_hooks_contain_cmdstr_comment() {
        let bash = generate_bash_hook("cmdstr");
        let zsh = generate_zsh_hook("cmdstr");
        let fish = generate_fish_hook("cmdstr");
        assert!(bash.contains("# ── cmdstr hook"));
        assert!(zsh.contains("# ── cmdstr hook"));
        assert!(fish.contains("# ── cmdstr hook"));
    }

    fn restore_env(name: &str, old: Option<String>) {
        match old {
            Some(v) => std::env::set_var(name, v),
            None => std::env::remove_var(name),
        }
    }
}

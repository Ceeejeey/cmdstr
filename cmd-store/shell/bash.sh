# cmdstr: smart command storage — Bash hook
# Source this in your .bashrc, or run: cmdstr install
# Works in vanilla bash (no bash-preexec required)

__cmdstr_session_id="$(uuidgen 2>/dev/null || echo $$-$(date +%s))"
__cmdstr_cmd=""
__cmdstr_start=""

# Capture the command BEFORE it runs via DEBUG trap
__cmdstr_preexec() {
    # Only capture if we haven't already captured for this prompt cycle
    [ -n "$__cmdstr_cmd" ] && return

    local cmd="$BASH_COMMAND"

    # Don't capture cmdstr's own commands or internal shell operations
    case "$cmd" in
        cmdstr\ capture*|__cmdstr_*|_cmdstr_*) return ;;
    esac

    # Don't capture empty or whitespace-only commands
    local trimmed="${cmd#"${cmd%%[![:space:]]*}"}"
    [ -z "$trimmed" ] && return

    __cmdstr_cmd="$cmd"
    __cmdstr_start="$(date +%s%N)"
}

# Process the result AFTER the command completes via PROMPT_COMMAND
__cmdstr_precmd() {
    local exit_code=$?
    if [ -n "$__cmdstr_cmd" ]; then
        local end="$(date +%s%N)"
        local duration=$(( (end - __cmdstr_start) / 1000000 ))
        cmdstr capture "$__cmdstr_cmd" "$exit_code" "$duration" "$PWD" \
            "$__cmdstr_session_id" 2>/dev/null || true
    fi
    __cmdstr_cmd=""
}

# Install the hooks
trap '__cmdstr_preexec' DEBUG

# Append to PROMPT_COMMAND without clobbering existing entries
if [[ "$PROMPT_COMMAND" != *"__cmdstr_precmd"* ]]; then
    PROMPT_COMMAND="__cmdstr_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi

# cmdstr: smart command storage — Bash hook
# Source this in your .bashrc, or run: cmdstr install

__cmdstr_session_id="$(uuidgen 2>/dev/null || echo $$-$(date +%s))"

cmdstr_preexec() {
    __cmdstr_cmd="$1"
    __cmdstr_start="$(date +%s%N)"
}

cmdstr_precmd() {
    local exit_code=$?
    if [ -n "$__cmdstr_cmd" ]; then
        local end="$(date +%s%N)"
        local duration=$(( (end - __cmdstr_start) / 1000000 ))
        cmdstr capture "$__cmdstr_cmd" "$exit_code" "$duration" "$PWD" \
            "$__cmdstr_session_id" 2>/dev/null || true
    fi
    __cmdstr_cmd=""
}

preexec_functions+=(cmdstr_preexec)
precmd_functions+=(cmdstr_precmd)

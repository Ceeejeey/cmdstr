# cmdstr: smart command storage — Fish hook
# Source this in your config.fish, or run: cmdstr install

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
        cmdstr capture "$__cmdstr_cmd" "$exit_code" "$duration" "$PWD" \
            "$__cmdstr_session_id" 2>/dev/null; or true
    end
    set -e __cmdstr_cmd
end

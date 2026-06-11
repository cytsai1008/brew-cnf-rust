# brew-cnf

Fast drop-in replacement for `brew which-formula --explain`, written in Rust.

## What this does

When you type an unknown command in the shell, `handler.sh` (Homebrew's command-not-found hook) calls `brew which-formula --explain <cmd>`. That invocation takes 600–700ms due to Ruby VM startup. This binary does the same lookup in ~5ms.

## Project structure

```
src/main.rs      — entire implementation (single file)
Cargo.toml       — deps: dirs = "6" (for home dir resolution)
PLAN.md          — discovery notes and design decisions
```

## Build

```sh
cargo build           # debug
cargo build --release # optimized
```

## Key design decisions

**No subprocess for path resolution.** The path to `executables.txt` is derived from OS conventions directly in Rust (`~/Library/Caches/Homebrew` on macOS, `$XDG_CACHE_HOME/Homebrew` on Linux) rather than calling `brew --cache`. `$HOMEBREW_CELLAR` env var overrides the Cellar path.

**No auto-update.** The binary never triggers `brew update`. If the database is stale (older than `$HOMEBREW_API_AUTO_UPDATE_SECS`, default 450s), it warns to stderr and continues. `--update` flag opts in to triggering `brew update --auto-update`. `HOMEBREW_NO_CNF_WARN=1` or `--no-warn` silences the warning.

**Output is identical to Homebrew's.** Same stdout strings, same exit codes (0 on match, 1 on no match or missing DB). The warning goes to stderr only so `handler.sh`'s `$()` capture is never contaminated.

**Whole-word matching.** Exe list is space-delimited; match uses `split_whitespace().any(|e| e == cmd)` — same semantics as Homebrew's `[[ " ${cmds_text} " == *" ${executable} "* ]]`.

## Integration with handler.sh

```sh
# In your .zshrc / .bashrc, after sourcing handler.sh:
homebrew_command_not_found_handle() {
  local cmd="$1"
  local txt
  if command -v brew-cnf &>/dev/null; then
    txt="$(brew-cnf "${cmd}" 2>&1 >/dev/tty || brew-cnf "${cmd}" 2>/dev/null)"
  else
    txt="$(brew which-formula --explain "${cmd}" 2>/dev/null)"
  fi
  # ... rest of handler
}
```

Or simpler — just replace the one line in a copy of `handler.sh`.

## Testing

```sh
./target/debug/brew-cnf ffmpeg          # multi-match
./target/debug/brew-cnf git             # single match
./target/debug/brew-cnf nonexistent     # no match, exit 1
./target/debug/brew-cnf --update ffmpeg # trigger brew update if stale
HOMEBREW_NO_CNF_WARN=1 ./target/debug/brew-cnf ffmpeg  # suppress warning
```

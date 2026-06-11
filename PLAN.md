# brew-cnf-rust

A fast drop-in replacement for `brew which-formula --explain`, written in Rust.

## Discovery

### Current mechanism

When you type an unknown command, the shell fires a built-in callback:
- zsh: `command_not_found_handler`
- bash: `command_not_found_handle`

Homebrew's `handler.sh` (sourced from `.zshrc`/`.bashrc`) registers itself into that slot. On every failed command it runs:

```sh
txt="$(brew which-formula --explain "${cmd}" 2>/dev/null)"
```

### Why it's slow

`brew` is a Ruby program. Every invocation pays ~300ms of fixed overhead before any real work:
1. Ruby VM startup
2. Loading hundreds of Homebrew `.rb` library files
3. Argument parsing and subcommand dispatch

Measured on an Apple Silicon Mac: **600–700ms** per invocation.

### What the actual work is

After all that startup, the real logic (`executables.sh`) does:

1. Read `~/Library/Caches/Homebrew/api/internal/executables.txt` (~330KB, ~6953 lines)
2. Scan line-by-line with a bash `while read` loop for whole-word match of the command name
3. Filter out already-installed formulae by checking `$HOMEBREW_CELLAR/<formula>/`
4. Print an install suggestion

The file format is:
```
formula(version):exe1 exe2 exe3
git:git git-cvsserver git-receive-pack ...
ffmpeg:ffmpeg ffprobe
```

The actual search takes **~1ms**. The overhead is ~600ms.

### Key paths

| Resource | Path |
|---|---|
| `executables.txt` | `~/Library/Caches/Homebrew/api/internal/executables.txt` (macOS) |
| `executables.txt` | `~/.cache/Homebrew/api/internal/executables.txt` (Linux) |
| Cellar | `$(brew --prefix)/Cellar/` (typically `/opt/homebrew/Cellar` on Apple Silicon) |
| handler script | `$(brew --repository)/Library/Homebrew/command-not-found/handler.sh` |

The cache path is not hardcoded in Homebrew — it derives from:
- macOS: `$HOME/Library/Caches/Homebrew`
- Linux: `${XDG_CACHE_HOME:-$HOME/.cache}/Homebrew`

The Cellar path derives from `brew --prefix`, typically:
- Apple Silicon: `/opt/homebrew/Cellar`
- Intel Mac: `/usr/local/Cellar`
- Linux: `/home/linuxbrew/.linuxbrew/Cellar`

---

## Plan

### Goal

Build a Rust binary `brew-cnf` that replicates the output of `brew which-formula --explain <cmd>` in under 10ms, with no Ruby or Homebrew process involvement.

### What to replicate

Given a command name (e.g. `ffmpeg`), the binary must:

1. Locate `executables.txt` without spawning any subprocess
2. Parse the file and find all formulae that provide the command
3. Filter out formulae already installed (present in Cellar)
4. Print one of:
   - `The program '<cmd>' is currently not installed. You can install it by typing:\n  brew install <formula>` (single match)
   - `The program '<cmd>' can be found in the following formulae:\n  * <formula1>\n  * <formula2>\nTry: brew install <selected formula>` (multiple matches)
   - Exit code 1 with no output if no match found or all matches are installed

Exit code must be `1` when nothing useful is printed (so `handler.sh` falls through to the default "command not found" message).

### What NOT to do

- Do not replace `handler.sh` — just replace the subprocess it calls
- Do not update or download `executables.txt` — leave that to `brew update`
- Do not handle `--no-install-from-api` or other brew flags

### Path resolution (no subprocess)

```rust
fn executables_txt_path() -> PathBuf {
    // macOS
    #[cfg(target_os = "macos")]
    let cache = dirs::home_dir().unwrap().join("Library/Caches/Homebrew");
    // Linux
    #[cfg(not(target_os = "macos"))]
    let cache = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::home_dir().unwrap().join(".cache"))
        .join("Homebrew");

    cache.join("api/internal/executables.txt")
}

fn cellar_path() -> PathBuf {
    // Apple Silicon default; fall back via HOMEBREW_CELLAR env var
    std::env::var("HOMEBREW_CELLAR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            #[cfg(target_os = "macos")]
            { PathBuf::from("/opt/homebrew/Cellar") }
            #[cfg(not(target_os = "macos"))]
            { PathBuf::from("/home/linuxbrew/.linuxbrew/Cellar") }
        })
}
```

### Staleness handling

The official Homebrew handler triggers `brew update --auto-update` when `executables.txt` is older than 450 seconds (`$HOMEBREW_API_AUTO_UPDATE_SECS`). This binary takes a lighter approach:

| Condition | Default behavior |
|---|---|
| File fresh (< 450s) | Search normally, no noise |
| File stale (≥ 450s) | Search normally, print warning to stderr |
| File missing | Exit 1 silently (fall through to shell's default message) |
| `--update` flag passed | Run `brew update --auto-update` if stale, then search |

**Staleness threshold** defaults to 450s, matching Homebrew. Override with `$HOMEBREW_API_AUTO_UPDATE_SECS`.

**Disable the stale warning** by setting `HOMEBREW_NO_CNF_WARN=1`.

**Flags:**
- `--update` — trigger `brew update --auto-update` when file is stale (disabled by default)
- `--no-warn` — suppress the stale warning (same effect as `HOMEBREW_NO_CNF_WARN=1`)

The warning is printed to **stderr** so it never contaminates the stdout text that `handler.sh` captures and echoes to the user.

### Implementation steps

1. **Scaffold** — `cargo init`, add `dirs` crate for home dir resolution
2. **Parse args** — one positional argument (the command name); `--update` and `--no-warn` flags
3. **Locate files** — implement `executables_txt_path()` and `cellar_path()` as above
4. **Staleness check** — stat the file mtime, compare against threshold; warn to stderr if stale and warn not suppressed; if `--update` and stale, spawn `brew update --auto-update`
5. **Search** — read file with `BufReader`, split each line on first `:`, check if the exe-list contains the command as a whole word
6. **Filter** — for each matching formula, check `cellar_path().join(formula).is_dir()`
7. **Output** — print the same text Homebrew prints; exit 0 on match, exit 1 on no match

### Usage after build

In `handler.sh`, change one line:

```sh
# before
txt="$(brew which-formula --explain "${cmd}" 2>/dev/null)"

# after (point to the built binary)
txt="$(brew-cnf "${cmd}" 2>/dev/null)"
```

Or wrap it with a fallback:

```sh
if command -v brew-cnf &>/dev/null; then
  txt="$(brew-cnf "${cmd}" 2>/dev/null)"
else
  txt="$(brew which-formula --explain "${cmd}" 2>/dev/null)"
fi
```

### Expected outcome

| | Current (`brew which-formula`) | Target (`brew-cnf`) |
|---|---|---|
| Startup cost | ~300ms (Ruby VM) | ~0ms |
| File parse | ~15ms (bash loop) | <1ms (Rust BufReader) |
| Total | ~600–700ms | <10ms |

# brew-cnf

A fast replacement for Homebrew's `brew command-not-found`, written in Rust.

When you type an unknown command in your shell, Homebrew's command-not-found handler suggests which formula to install. The built-in handler invokes the full `brew` Ruby process for every typo — costing ~700ms. This binary does the same lookup in ~5ms.

| | `brew which-formula` | `brew-cnf` |
|---|---|---|
| Startup | ~300ms (Ruby VM) | ~0ms |
| Lookup | ~15ms (bash loop) | <1ms |
| **Total** | **~700ms** | **~5ms** |

## Installation

```sh
brew tap cytsai1008/tap
brew trust cytsai1008/tap
brew install brew-cnf
```

## Usage

```sh
brew-cnf <command>
```

Same output as `brew which-formula --explain`:

```
$ brew-cnf ffmpeg
The program 'ffmpeg' can be found in the following formulae:
  * ffmpeg
  * ffmpeg@7
Try: brew install <selected formula>

$ brew-cnf git
The program 'git' is currently not installed. You can install it by typing:
  brew install git
```

Exits `0` when a formula is found, `1` when not found (so the shell falls through to its default "command not found" message).

## Flags

| Flag | Description |
|---|---|
| `--init` | Print the shell hook for `eval` in `.zshrc` / `.bashrc` |
| `--update` | Refresh the executables database, or run `brew update --auto-update` if the database is missing/stale before searching |
| `--no-warn` | Suppress the stale database warning |
| `--help` | Show usage |

When used with `--init`, `--update` and `--no-warn` are preserved in the generated shell hook:

```sh
eval "$(brew-cnf --init --no-warn)"
eval "$(brew-cnf --init --update)"
```

`brew-cnf --update` without a command refreshes Homebrew's executables database and exits. It exits `0` only when `brew update --auto-update` succeeds and the database exists afterward.

## Environment variables

| Variable | Description |
|---|---|
| `HOMEBREW_NO_CNF_WARN=1` | Suppress `brew-cnf` warnings (same as `--no-warn`) |
| `HOMEBREW_API_AUTO_UPDATE_SECS` | Staleness threshold in seconds (default: 604800 = 7 days) |
| `HOMEBREW_CELLAR` | Override the Cellar path |
| `HOMEBREW_BREW_FILE` | Override the `brew` binary path (used with `--update`) |

## Shell integration

Add one line to your `.zshrc` or `.bashrc`:

```sh
eval "$(brew-cnf --init)"
```

`brew-cnf --init` prints:

```sh
command_not_found_handler() { echo "zsh: command not found: $1" >&2; echo >&2; brew-cnf "$1"; return 127; }
command_not_found_handle() { echo "bash: $1: command not found" >&2; echo >&2; brew-cnf "$1"; return 127; }
```

`eval` installs both into your shell (`command_not_found_handler` for zsh, `command_not_found_handle` for bash — each shell ignores the other's name). The handlers return 127 (the conventional exit code for "command not found") and always print the shell's standard error message, since the shell suppresses its own output once a handler is defined. When appropriate for an interactive shell, `brew-cnf` also adds Homebrew formula suggestions. The hook definition stays in the binary, so it updates automatically when you upgrade `brew-cnf`.

## How it works

Homebrew maintains a database of every formula and its installed executables at:

- macOS: `~/Library/Caches/Homebrew/api/internal/executables.txt`
- Linux: `~/.cache/Homebrew/api/internal/executables.txt`

The file looks like:

```
ffmpeg:ffmpeg ffprobe
git:git git-cvsserver git-receive-pack ...
```

`brew-cnf` reads this file directly with a Rust `BufReader`, matches whole words, filters out already-installed formulae by checking the Cellar, and prints the result — no Ruby, no subprocess.

## Requirements

- Rust 1.70+
- Homebrew with `brew update` run at least once (to populate `executables.txt`)

# brew-cnf

A fast replacement for Homebrew's `brew which-formula --explain`, written in Rust.

When you type an unknown command in your shell, Homebrew's command-not-found handler suggests which formula to install. The built-in handler invokes the full `brew` Ruby process for every typo — costing ~700ms. This binary does the same lookup in ~5ms.

| | `brew which-formula` | `brew-cnf` |
|---|---|---|
| Startup | ~300ms (Ruby VM) | ~0ms |
| Lookup | ~15ms (bash loop) | <1ms |
| **Total** | **~700ms** | **~5ms** |

## Installation

```sh
brew tap cytsai1008/tap
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
| `--update` | Run `brew update --auto-update` if the database is stale before searching |
| `--no-warn` | Suppress the stale database warning |
| `--help` | Show usage |

## Environment variables

| Variable | Description |
|---|---|
| `HOMEBREW_NO_CNF_WARN=1` | Suppress the stale database warning (same as `--no-warn`) |
| `HOMEBREW_API_AUTO_UPDATE_SECS` | Staleness threshold in seconds (default: 450) |
| `HOMEBREW_CELLAR` | Override the Cellar path |
| `HOMEBREW_BREW_FILE` | Override the `brew` binary path (used with `--update`) |

## Hook integration

Find your Homebrew `handler.sh`:

```sh
brew command-not-found-init
```

In your `.zshrc` or `.bashrc`, source `handler.sh` then override the inner function to use `brew-cnf` with a fallback:

```sh
source "$(brew --repository)/Library/Homebrew/command-not-found/handler.sh"

homebrew_command_not_found_handle() {
  local cmd="$1"
  local txt
  if command -v brew-cnf &>/dev/null; then
    txt="$(brew-cnf "${cmd}" 2>/dev/null)"
  else
    txt="$(brew which-formula --explain "${cmd}" 2>/dev/null)"
  fi

  if [[ -z "${txt}" ]]; then
    [[ -n "${ZSH_VERSION}" ]] && echo "zsh: command not found: ${cmd}" >&2
    [[ -n "${BASH_VERSION}" ]] && echo "${cmd}: command not found" >&2
  else
    echo "${txt}"
  fi
  return 127
}
```

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

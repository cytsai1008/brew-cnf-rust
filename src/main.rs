use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::SystemTime;

fn executables_txt_path() -> PathBuf {
    if let Ok(p) = std::env::var("HOMEBREW_CACHE") {
        return PathBuf::from(p).join("api/internal/executables.txt");
    }

    #[cfg(target_os = "macos")]
    let cache = dirs::home_dir()
        .expect("cannot determine home directory")
        .join("Library/Caches/Homebrew");

    #[cfg(not(target_os = "macos"))]
    let cache = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .expect("cannot determine home directory")
                .join(".cache")
        })
        .join("Homebrew");

    cache.join("api/internal/executables.txt")
}

fn cellar_path() -> PathBuf {
    if let Ok(p) = std::env::var("HOMEBREW_CELLAR") {
        return PathBuf::from(p);
    }
    if let Ok(p) = std::env::var("HOMEBREW_PREFIX") {
        let path = PathBuf::from(p).join("Cellar");
        if path.exists() {
            return path;
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Apple Silicon default; Intel fallback
        let arm = PathBuf::from("/opt/homebrew/Cellar");
        if arm.exists() {
            return arm;
        }
        PathBuf::from("/usr/local/Cellar")
    }

    #[cfg(not(target_os = "macos"))]
    {
        let system = PathBuf::from("/home/linuxbrew/.linuxbrew/Cellar");
        if system.exists() {
            return system;
        }
        dirs::home_dir()
            .expect("cannot determine home directory")
            .join(".linuxbrew/Cellar")
    }
}

fn staleness_threshold_secs() -> u64 {
    std::env::var("HOMEBREW_API_AUTO_UPDATE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(604_800) // 7 days
}

fn file_age_secs(path: &Path) -> Option<u64> {
    let mtime = path.metadata().ok()?.modified().ok()?;
    SystemTime::now()
        .duration_since(mtime)
        .ok()
        .map(|d| d.as_secs())
}

fn is_installed(cellar: &Path, formula: &str) -> bool {
    cellar.join(formula).is_dir()
}

fn search_content(content: &str, cmd: &str, cellar: &Path) -> Vec<(String, bool)> {
    // Pre-build padded needle: " cmd " matches whole words in the space-delimited exe list.
    // Prepend/append a space to the exe line when checking so we handle start/end of string too.
    let needle = format!(" {cmd} ");

    let mut matches = vec![];

    for line in content.lines() {
        let Some(colon) = line.find(':') else {
            continue;
        };
        let formula_raw = &line[..colon];
        let exes = &line[colon + 1..];

        // strip optional version suffix: "git(2.44)" → "git"
        let formula = match formula_raw.find('(') {
            Some(i) => &formula_raw[..i],
            None => formula_raw,
        };

        // Whole-word match. needle is " cmd "; also cover boundary cases and the
        // single-entry case (exes == "cmd" with no surrounding spaces).
        let found = exes == cmd
            || exes.contains(needle.as_str())
            || exes.starts_with(&needle[1..])                 // cmd at start: "cmd ..."
            || exes.ends_with(&needle[..needle.len() - 1]); // cmd at end:   "... cmd"
        if found {
            matches.push((formula.to_string(), is_installed(cellar, formula)));
        }
    }

    matches
}

fn print_usage() {
    eprintln!("Usage: brew-cnf [--explain] [--update] [--no-warn] <command>...");
    eprintln!("       brew-cnf --update   refresh Homebrew's executables database");
    eprintln!(
        "       brew-cnf --init [--update] [--no-warn]   print shell hook (eval in .zshrc/.bashrc)"
    );
    eprintln!("       --explain           suggest install command (default: show ✔/✘ status)");
    eprintln!("       HOMEBREW_NO_CNF_WARN=1  suppress stale-database warning");
    eprintln!(
        "       HOMEBREW_API_AUTO_UPDATE_SECS=N  staleness threshold (default: 604800 = 7 days)"
    );
}

fn print_init(flag_update: bool, flag_no_warn: bool) {
    let mut lookup_args = vec!["--explain"];
    if flag_update {
        lookup_args.push("--update");
    }
    if flag_no_warn {
        lookup_args.push("--no-warn");
    }
    let lookup_args = format!(" {}", lookup_args.join(" "));

    // zsh uses command_not_found_handler; bash uses command_not_found_handle.
    // Defining both is harmless — each shell ignores the other's name.
    // CI skip: suppress lookup when inside a pipe or Midnight Commander (MC_SID),
    // unless HOMEBREW_COMMAND_NOT_FOUND_CI is set (matches official Homebrew handler).
    println!(
        "command_not_found_handler() {{\n\
        \x20 echo \"zsh: command not found: $1\" >&2\n\
        \x20 if [ -z \"${{HOMEBREW_COMMAND_NOT_FOUND_CI}}\" ] && {{ [ -n \"${{MC_SID}}\" ] || [ ! -t 1 ]; }}; then\n\
        \x20   return 127\n\
        \x20 fi\n\
        \x20 echo >&2\n\
        \x20 brew-cnf{} -- \"$1\"\n\
        \x20 return 127\n\
        }}",
        lookup_args
    );
    println!(
        "command_not_found_handle() {{\n\
        \x20 echo \"bash: $1: command not found\" >&2\n\
        \x20 if [ -z \"${{HOMEBREW_COMMAND_NOT_FOUND_CI}}\" ] && {{ [ -n \"${{MC_SID}}\" ] || [ ! -t 1 ]; }}; then\n\
        \x20   return 127\n\
        \x20 fi\n\
        \x20 echo >&2\n\
        \x20 brew-cnf{} -- \"$1\"\n\
        \x20 return 127\n\
        }}",
        lookup_args
    );
}

fn run_brew_update(no_warn: bool) -> bool {
    let brew = std::env::var("HOMEBREW_BREW_FILE").unwrap_or_else(|_| "brew".into());
    let ok = process::Command::new(&brew)
        .args(["update", "--auto-update"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok && !no_warn {
        eprintln!("brew-cnf: warning: `brew update --auto-update` failed");
    }
    ok
}

fn is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

// Mirrors Homebrew's tty_colour_enabled: color when HOMEBREW_COLOR is set,
// or when on a TTY and HOMEBREW_NO_COLOR is unset.
fn colour_enabled() -> bool {
    if std::env::var_os("HOMEBREW_COLOR").is_some() {
        return true;
    }
    if std::env::var_os("HOMEBREW_NO_COLOR").is_some() {
        return false;
    }
    is_tty()
}

// Mirrors Homebrew's pretty_installed / pretty_uninstalled in utils.sh.
fn print_formula_status(formula: &str, installed: bool, tty: bool, color: bool, no_emoji: bool) {
    if !tty && !color {
        println!("{formula}");
        return;
    }

    if no_emoji {
        let label = if installed { "installed" } else { "uninstalled" };
        if color {
            let code = if installed { 32 } else { 31 };
            println!("\x1b[{code}m\x1b[1m{formula} ({label})\x1b[0m");
        } else {
            println!("{formula} ({label})");
        }
    } else if color {
        let (code, marker) = if installed { (32, '✔') } else { (31, '✘') };
        println!("\x1b[1m{formula} \x1b[{code}m{marker}\x1b[0m");
    } else {
        let marker = if installed { '✔' } else { '✘' };
        println!("{formula} {marker}");
    }
}

fn print_matches(cmd: &str, formulae: &[String]) {
    if formulae.len() == 1 {
        println!("The program '{cmd}' is currently not installed. You can install it by typing:");
        println!("  brew install {}", formulae[0]);
    } else {
        println!("The program '{cmd}' can be found in the following formulae:");
        for f in formulae {
            println!("  * {f}");
        }
        println!("Try: brew install <selected formula>");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ── search_content ──────────────────────────────────────────────────────

    fn fake_cellar() -> PathBuf {
        PathBuf::from("/nonexistent/cellar")
    }

    const DB: &str = "\
foo(1.0.0):foo2 foo3\n\
bar(1.2.3):\n\
baz(10.4):baz\n\
qux(4.5.6):QUX\n\
quux:quux\n\
multi:aaa bbb ccc\n\
edge:first last\n";

    #[test]
    fn search_no_match() {
        assert!(search_content(DB, "nothere", &fake_cellar()).is_empty());
    }

    #[test]
    fn search_empty_exe_list() {
        // "bar" has no executables — command "bar" should not match
        assert!(search_content(DB, "bar", &fake_cellar()).is_empty());
    }

    #[test]
    fn search_single_match() {
        let m = search_content(DB, "baz", &fake_cellar());
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].0, "baz");
    }

    #[test]
    fn search_multi_formula_match() {
        // "foo2" appears in foo's exe list
        let m = search_content(DB, "foo2", &fake_cellar());
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].0, "foo");
    }

    #[test]
    fn search_version_suffix_stripped() {
        // Formula name is "foo(1.0.0)" → should be returned as "foo"
        let m = search_content(DB, "foo3", &fake_cellar());
        assert_eq!(m[0].0, "foo");
    }

    #[test]
    fn search_no_version_suffix() {
        // "quux" has no version suffix
        let m = search_content(DB, "quux", &fake_cellar());
        assert_eq!(m[0].0, "quux");
    }

    #[test]
    fn search_no_partial_match() {
        // "baz" is in the db, but "ba" should not match
        assert!(search_content(DB, "ba", &fake_cellar()).is_empty());
        assert!(search_content(DB, "baz2", &fake_cellar()).is_empty());
    }

    #[test]
    fn search_middle_of_list() {
        let m = search_content(DB, "bbb", &fake_cellar());
        assert_eq!(m[0].0, "multi");
    }

    #[test]
    fn search_first_in_list() {
        let m = search_content(DB, "first", &fake_cellar());
        assert_eq!(m[0].0, "edge");
    }

    #[test]
    fn search_last_in_list() {
        let m = search_content(DB, "last", &fake_cellar());
        assert_eq!(m[0].0, "edge");
    }

    #[test]
    fn search_case_sensitive() {
        // "QUX" is in the db, lowercase "qux" should not match
        assert!(search_content(DB, "qux", &fake_cellar()).is_empty());
        let m = search_content(DB, "QUX", &fake_cellar());
        assert_eq!(m[0].0, "qux");
    }

    #[test]
    fn search_installed_flag() {
        // cellar does not exist → all formulas report not installed
        let m = search_content(DB, "baz", &fake_cellar());
        assert!(!m[0].1, "expected not-installed");
    }

    // ── staleness_threshold_secs ─────────────────────────────────────────────

    #[test]
    fn staleness_default() {
        // SAFETY: single-threaded test context; no concurrent env reads for this var
        unsafe { std::env::remove_var("HOMEBREW_API_AUTO_UPDATE_SECS") };
        assert_eq!(staleness_threshold_secs(), 604_800);
    }

    #[test]
    fn staleness_env_override() {
        unsafe {
            std::env::set_var("HOMEBREW_API_AUTO_UPDATE_SECS", "3600");
        }
        assert_eq!(staleness_threshold_secs(), 3600);
        unsafe {
            std::env::remove_var("HOMEBREW_API_AUTO_UPDATE_SECS");
        }
    }

    #[test]
    fn staleness_invalid_env_falls_back_to_default() {
        unsafe {
            std::env::set_var("HOMEBREW_API_AUTO_UPDATE_SECS", "notanumber");
        }
        assert_eq!(staleness_threshold_secs(), 604_800);
        unsafe {
            std::env::remove_var("HOMEBREW_API_AUTO_UPDATE_SECS");
        }
    }

    // ── print_formula_status (output capture) ────────────────────────────────

    fn capture_print_formula_status(
        formula: &str,
        installed: bool,
        tty: bool,
        color: bool,
        no_emoji: bool,
    ) -> String {
        use std::io::Write;
        let mut buf = Vec::new();
        // Redirect by building the string manually mirroring the function logic.
        if !tty && !color {
            writeln!(buf, "{formula}").unwrap();
        } else if no_emoji {
            let label = if installed { "installed" } else { "uninstalled" };
            if color {
                let code = if installed { 32 } else { 31 };
                writeln!(buf, "\x1b[{code}m\x1b[1m{formula} ({label})\x1b[0m").unwrap();
            } else {
                writeln!(buf, "{formula} ({label})").unwrap();
            }
        } else if color {
            let (code, marker) = if installed { (32, '✔') } else { (31, '✘') };
            writeln!(buf, "\x1b[1m{formula} \x1b[{code}m{marker}\x1b[0m").unwrap();
        } else {
            let marker = if installed { '✔' } else { '✘' };
            writeln!(buf, "{formula} {marker}").unwrap();
        }
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn formula_status_no_tty_no_color_plain() {
        let out = capture_print_formula_status("mypkg", false, false, false, false);
        assert_eq!(out, "mypkg\n");
    }

    #[test]
    fn formula_status_tty_no_color_installed() {
        let out = capture_print_formula_status("mypkg", true, true, false, false);
        assert_eq!(out, "mypkg ✔\n");
    }

    #[test]
    fn formula_status_tty_no_color_uninstalled() {
        let out = capture_print_formula_status("mypkg", false, true, false, false);
        assert_eq!(out, "mypkg ✘\n");
    }

    #[test]
    fn formula_status_no_emoji_no_color_installed() {
        let out = capture_print_formula_status("mypkg", true, true, false, true);
        assert_eq!(out, "mypkg (installed)\n");
    }

    #[test]
    fn formula_status_no_emoji_color_uninstalled() {
        let out = capture_print_formula_status("mypkg", false, true, true, true);
        assert_eq!(out, "\x1b[31m\x1b[1mmypkg (uninstalled)\x1b[0m\n");
    }

    #[test]
    fn formula_status_color_installed() {
        let out = capture_print_formula_status("mypkg", true, true, true, false);
        assert_eq!(out, "\x1b[1mmypkg \x1b[32m✔\x1b[0m\n");
    }

    #[test]
    fn formula_status_color_uninstalled() {
        let out = capture_print_formula_status("mypkg", false, true, true, false);
        assert_eq!(out, "\x1b[1mmypkg \x1b[31m✘\x1b[0m\n");
    }
}

fn main() {
    let mut cmd_args: Vec<String> = Vec::new();
    let mut flag_update = false;
    let mut flag_no_warn = false;
    let mut flag_init = false;
    let mut flag_explain = false;
    let mut end_of_opts = false;
    let mut saw_bare_dash = false;

    let raw_args: Vec<String> = std::env::args().skip(1).collect();

    for arg in &raw_args {
        if end_of_opts {
            cmd_args.push(arg.clone());
            continue;
        }
        match arg.as_str() {
            "--update" => flag_update = true,
            "--no-warn" => flag_no_warn = true,
            "--init" => flag_init = true,
            "--explain" => flag_explain = true,
            "-h" | "--help" => {
                print_usage();
                process::exit(0);
            }
            // `--` ends option parsing; everything after is a lookup target.
            "--" => {
                end_of_opts = true;
                saw_bare_dash = true;
            }
            // bare `-` is a shell idiom (stdin), not a command to look up
            "-" => saw_bare_dash = true,
            _ if arg.starts_with('-') => {
                eprintln!("brew-cnf: unknown flag: {arg}");
                print_usage();
                process::exit(1);
            }
            _ => {
                cmd_args.push(arg.clone());
            }
        }
    }

    if flag_init {
        print_init(flag_update, flag_no_warn);
        process::exit(0);
    }

    let no_warn = flag_no_warn || std::env::var("HOMEBREW_NO_CNF_WARN").is_ok_and(|v| v == "1");
    let db_path = executables_txt_path();

    if cmd_args.is_empty() {
        if flag_update {
            let ok = run_brew_update(no_warn);
            if ok && db_path.exists() {
                process::exit(0);
            }
            if ok && !no_warn {
                eprintln!(
                    "brew-cnf: warning: executables database not found at {}",
                    db_path.display()
                );
            }
            process::exit(1);
        }
        // Bare `-` or standalone `--` with no real command: silently exit.
        if saw_bare_dash {
            process::exit(0);
        }
        if raw_args.is_empty() {
            print_usage();
            process::exit(1);
        }
        print_usage();
        process::exit(1);
    }

    if !db_path.exists() {
        if flag_update {
            run_brew_update(no_warn);
        }
    }

    if !db_path.exists() {
        if !no_warn {
            eprintln!(
                "brew-cnf: warning: executables database not found at {}",
                db_path.display()
            );
        }
        process::exit(1);
    }

    let threshold = staleness_threshold_secs();

    if let Some(age) = file_age_secs(&db_path) {
        if age >= threshold {
            if flag_update {
                run_brew_update(no_warn);
            } else if !no_warn {
                let age_mins = age / 60;
                eprintln!(
                    "brew-cnf: warning: executables database is {age_mins}m old (threshold: {}m). \
                     Run `brew update` or use --update to refresh.",
                    threshold / 60
                );
            }
        }
    }

    let cellar = cellar_path();
    let tty = is_tty();
    let color = colour_enabled();
    let no_emoji = std::env::var_os("HOMEBREW_NO_EMOJI").is_some();

    let db_content = match fs::read_to_string(&db_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("brew-cnf: error reading database: {e}");
            process::exit(1);
        }
    };

    let mut exit_code = 0i32;
    for cmd in &cmd_args {
        let matches = search_content(&db_content, cmd, &cellar);
        if matches.is_empty() {
            exit_code = 1;
            continue;
        }
        if flag_explain {
            let uninstalled: Vec<String> = matches
                .iter()
                .filter(|(_, installed)| !installed)
                .map(|(f, _)| f.clone())
                .collect();
            if uninstalled.is_empty() {
                exit_code = 1;
                continue;
            }
            print_matches(cmd, &uninstalled);
        } else {
            for (formula, installed) in &matches {
                print_formula_status(formula, *installed, tty, color, no_emoji);
            }
        }
    }
    process::exit(exit_code);
}

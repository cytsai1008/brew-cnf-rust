use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::SystemTime;

fn executables_txt_path() -> PathBuf {
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
    PathBuf::from("/home/linuxbrew/.linuxbrew/Cellar")
}

fn staleness_threshold_secs() -> u64 {
    std::env::var("HOMEBREW_API_AUTO_UPDATE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(604_800) // 7 days
}

fn file_age_secs(path: &Path) -> Option<u64> {
    let mtime = path.metadata().ok()?.modified().ok()?;
    SystemTime::now().duration_since(mtime).ok().map(|d| d.as_secs())
}

fn is_installed(cellar: &Path, formula: &str) -> bool {
    cellar.join(formula).is_dir()
}

fn search(db_path: &Path, cmd: &str, cellar: &Path) -> Vec<String> {
    let content = match fs::read_to_string(db_path) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    // Pre-build padded needle: " cmd " matches whole words in the space-delimited exe list.
    // Prepend/append a space to the exe line when checking so we handle start/end of string too.
    let needle = format!(" {cmd} ");

    let mut matches = vec![];

    for line in content.lines() {
        let Some(colon) = line.find(':') else { continue };
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
            || exes.ends_with(&needle[..needle.len() - 1]);   // cmd at end:   "... cmd"
        if found && !is_installed(cellar, formula) {
            matches.push(formula.to_string());
        }
    }

    matches
}

fn print_usage() {
    eprintln!("Usage: brew-cnf [--update] [--no-warn] <command>");
    eprintln!("       HOMEBREW_NO_CNF_WARN=1  suppress stale-database warning");
    eprintln!("       HOMEBREW_API_AUTO_UPDATE_SECS=N  staleness threshold (default: 604800 = 7 days)");
}

fn main() {
    let mut cmd_arg: Option<String> = None;
    let mut flag_update = false;
    let mut flag_no_warn = false;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--update" => flag_update = true,
            "--no-warn" => flag_no_warn = true,
            "-h" | "--help" => {
                print_usage();
                process::exit(0);
            }
            _ if arg.starts_with('-') => {
                eprintln!("brew-cnf: unknown flag: {arg}");
                print_usage();
                process::exit(1);
            }
            _ => {
                if cmd_arg.is_some() {
                    eprintln!("brew-cnf: too many arguments");
                    print_usage();
                    process::exit(1);
                }
                cmd_arg = Some(arg);
            }
        }
    }

    let cmd = match cmd_arg {
        Some(c) => c,
        None => {
            print_usage();
            process::exit(1);
        }
    };

    let db_path = executables_txt_path();

    if !db_path.exists() {
        process::exit(1);
    }

    let threshold = staleness_threshold_secs();
    let no_warn = flag_no_warn || std::env::var("HOMEBREW_NO_CNF_WARN").is_ok_and(|v| v == "1");

    if let Some(age) = file_age_secs(&db_path) {
        if age >= threshold {
            if flag_update {
                let brew = std::env::var("HOMEBREW_BREW_FILE").unwrap_or_else(|_| "brew".into());
                let ok = process::Command::new(&brew)
                    .args(["update", "--auto-update"])
                    .stdout(process::Stdio::null())
                    .stderr(process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if !ok {
                    eprintln!("brew-cnf: warning: `brew update --auto-update` failed");
                }
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
    let formulae = search(&db_path, &cmd, &cellar);

    if formulae.is_empty() {
        process::exit(1);
    }

    if formulae.len() == 1 {
        println!("The program '{cmd}' is currently not installed. You can install it by typing:");
        println!("  brew install {}", formulae[0]);
    } else {
        println!("The program '{cmd}' can be found in the following formulae:");
        for f in &formulae {
            println!("  * {f}");
        }
        println!("Try: brew install <selected formula>");
    }
}

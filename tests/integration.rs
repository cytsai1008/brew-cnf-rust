/// Integration tests for brew-cnf.
///
/// Each test creates a temporary executables.txt (and optionally a fake Cellar)
/// and exercises the binary via `std::process::Command`, checking stdout, stderr,
/// and exit status — mirroring the approach in Homebrew's which-formula_spec.rb.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Path to the compiled test binary.
fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_brew-cnf"))
}

/// Set up a scratch directory with `executables.txt` at the path the binary
/// expects when `HOMEBREW_CACHE` is set.  Returns the temp dir root.
fn make_db(dir: &Path, content: &str) {
    let db_dir = dir.join("api/internal");
    fs::create_dir_all(&db_dir).unwrap();
    fs::write(db_dir.join("executables.txt"), content).unwrap();
}

/// Create a fake Cellar entry so `is_installed` returns true.
fn install_formula(cellar: &Path, formula: &str) {
    fs::create_dir_all(cellar.join(format!("{formula}/1.0.0"))).unwrap();
}

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new(db_content: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        make_db(dir.path(), db_content);
        Self { dir }
    }

    fn cache_dir(&self) -> &Path {
        self.dir.path()
    }

    fn cellar_dir(&self) -> PathBuf {
        self.dir.path().join("Cellar")
    }

    fn install(&self, formula: &str) {
        install_formula(&self.cellar_dir(), formula);
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(bin())
            .args(args)
            .env("HOMEBREW_CACHE", self.cache_dir())
            .env("HOMEBREW_CELLAR", self.cellar_dir())
            .env("HOMEBREW_NO_COLOR", "1")
            .env("HOMEBREW_NO_EMOJI", "1")
            .output()
            .expect("failed to run brew-cnf")
    }

    fn run_env(&self, args: &[&str], extra_env: &[(&str, &str)]) -> Output {
        let mut cmd = Command::new(bin());
        cmd.args(args)
            .env("HOMEBREW_CACHE", self.cache_dir())
            .env("HOMEBREW_CELLAR", self.cellar_dir())
            .env("HOMEBREW_NO_COLOR", "1")
            .env("HOMEBREW_NO_EMOJI", "1");
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        cmd.output().expect("failed to run brew-cnf")
    }
}

fn stdout(o: &Output) -> &str {
    std::str::from_utf8(&o.stdout).unwrap().trim_end_matches('\n')
}

fn stderr(o: &Output) -> &str {
    std::str::from_utf8(&o.stderr).unwrap().trim_end_matches('\n')
}

// ── DB content shared across tests ───────────────────────────────────────────

const DB: &str = "\
foo(1.0.0):foo2 foo3\n\
bar(1.2.3):\n\
baz(10.4):baz\n\
qux(4.5.6):QUX\n\
quux:quux\n\
multi:aaa bbb ccc\n";

// ── --explain mode ────────────────────────────────────────────────────────────

#[test]
fn explain_single_uninstalled_formula() {
    let fx = Fixture::new(DB);
    let out = fx.run(&["--explain", "baz"]);
    assert!(out.status.success(), "exit code should be 0");
    assert_eq!(
        stdout(&out),
        "The program 'baz' is currently not installed. You can install it by typing:\n  brew install baz"
    );
    assert!(stderr(&out).is_empty());
}

#[test]
fn explain_multi_formula_match() {
    // Both "aaa" and "bbb" come from "multi" — single-formula result
    let fx = Fixture::new(DB);
    let out = fx.run(&["--explain", "aaa"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("brew install multi"));
}

#[test]
fn explain_installed_formula_exits_1() {
    let fx = Fixture::new(DB);
    fx.install("baz");
    let out = fx.run(&["--explain", "baz"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).is_empty(), "should produce no output when already installed");
}

#[test]
fn explain_no_match_exits_1() {
    let fx = Fixture::new(DB);
    let out = fx.run(&["--explain", "nothere"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).is_empty());
}

#[test]
fn explain_empty_exe_list_exits_1() {
    let fx = Fixture::new(DB);
    // "bar" has an empty exe list — should not match
    let out = fx.run(&["--explain", "bar"]);
    assert_eq!(out.status.code(), Some(1));
}

// ── status mode (default, no --explain) ──────────────────────────────────────

#[test]
fn status_uninstalled_shows_marker() {
    let fx = Fixture::new(DB);
    // Force color so the (uninstalled) label is rendered (non-TTY run would suppress it otherwise)
    let out = fx.run_env(&["baz"], &[("HOMEBREW_COLOR", "1"), ("HOMEBREW_NO_EMOJI", "1")]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("baz"), "formula name should appear");
    assert!(stdout(&out).contains("uninstalled"));
}

#[test]
fn status_installed_shows_installed() {
    let fx = Fixture::new(DB);
    fx.install("baz");
    let out = fx.run_env(&["baz"], &[("HOMEBREW_COLOR", "1"), ("HOMEBREW_NO_EMOJI", "1")]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("installed"));
    assert!(!stdout(&out).contains("uninstalled"));
}

#[test]
fn status_multiple_commands_mixed_exit() {
    // "baz" matches, "nothere" doesn't → exit 1 (any miss → 1)
    let fx = Fixture::new(DB);
    let out = fx.run(&["baz", "nothere"]);
    assert_eq!(out.status.code(), Some(1));
    // "baz" line should still be printed
    assert!(stdout(&out).contains("baz"));
}

#[test]
fn status_all_matched_exit_0() {
    let fx = Fixture::new(DB);
    let out = fx.run(&["baz", "quux"]);
    assert!(out.status.success());
}

// ── missing database ──────────────────────────────────────────────────────────

#[test]
fn missing_db_exits_1_with_warning() {
    let dir = tempfile::tempdir().unwrap();
    // Do NOT create executables.txt
    let out = Command::new(bin())
        .args(["git"])
        .env("HOMEBREW_CACHE", dir.path())
        .env("HOMEBREW_CELLAR", dir.path().join("Cellar"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = std::str::from_utf8(&out.stderr).unwrap();
    assert!(err.contains("warning"), "should warn about missing database");
}

#[test]
fn missing_db_no_warn_suppresses_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .args(["--no-warn", "git"])
        .env("HOMEBREW_CACHE", dir.path())
        .env("HOMEBREW_CELLAR", dir.path().join("Cellar"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        std::str::from_utf8(&out.stderr).unwrap().is_empty(),
        "--no-warn should suppress the missing-db warning"
    );
}

#[test]
fn missing_db_env_no_warn_suppresses_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .args(["git"])
        .env("HOMEBREW_CACHE", dir.path())
        .env("HOMEBREW_CELLAR", dir.path().join("Cellar"))
        .env("HOMEBREW_NO_CNF_WARN", "1")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(std::str::from_utf8(&out.stderr).unwrap().is_empty());
}

// ── version-suffix stripping ──────────────────────────────────────────────────

#[test]
fn version_suffix_stripped_in_explain_output() {
    let fx = Fixture::new(DB);
    // foo(1.0.0) should appear as "foo" in output, not "foo(1.0.0)"
    let out = fx.run(&["--explain", "foo2"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("brew install foo"), "got: {text}");
    assert!(!text.contains("foo("), "version suffix must be stripped");
}

// ── case sensitivity ──────────────────────────────────────────────────────────

#[test]
fn case_sensitive_no_match() {
    let fx = Fixture::new(DB);
    // "qux" is lowercase but DB has "QUX" as the executable
    let out = fx.run(&["--explain", "qux"]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn case_sensitive_exact_match() {
    let fx = Fixture::new(DB);
    let out = fx.run(&["--explain", "QUX"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("brew install qux"));
}

// ── no-args / --help ──────────────────────────────────────────────────────────

#[test]
fn no_args_exits_1_with_usage() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .env("HOMEBREW_CACHE", dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = std::str::from_utf8(&out.stderr).unwrap();
    assert!(err.contains("Usage"));
}

#[test]
fn help_flag_exits_0_with_usage() {
    let out = Command::new(bin()).args(["--help"]).output().unwrap();
    assert!(out.status.success());
    let err = std::str::from_utf8(&out.stderr).unwrap();
    assert!(err.contains("Usage"));
}

#[test]
fn unknown_flag_exits_1() {
    let out = Command::new(bin()).args(["--bogus"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
}

// ── --init ────────────────────────────────────────────────────────────────────

#[test]
fn init_prints_shell_hooks() {
    let out = Command::new(bin()).args(["--init"]).output().unwrap();
    assert!(out.status.success());
    let text = std::str::from_utf8(&out.stdout).unwrap();
    assert!(text.contains("command_not_found_handler"), "zsh hook missing");
    assert!(text.contains("command_not_found_handle"), "bash hook missing");
    assert!(text.contains("brew-cnf --explain"));
}

#[test]
fn init_with_update_includes_flag() {
    let out = Command::new(bin()).args(["--init", "--update"]).output().unwrap();
    assert!(out.status.success());
    let text = std::str::from_utf8(&out.stdout).unwrap();
    assert!(text.contains("--update"));
}

#[test]
fn init_with_no_warn_includes_flag() {
    let out = Command::new(bin()).args(["--init", "--no-warn"]).output().unwrap();
    assert!(out.status.success());
    let text = std::str::from_utf8(&out.stdout).unwrap();
    assert!(text.contains("--no-warn"));
}

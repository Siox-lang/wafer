//! Opt-in real sioxc/native acceptance; never substitute a fake compiler.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "wafer_native_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn cli(root: &Path, args: &[&str]) -> Output {
    let compiler = std::env::var_os("WAFER_TEST_SIOXC")
        .expect("set WAFER_TEST_SIOXC to the actual compiler with explicit modules/test planning");
    let std = std::env::var_os("WAFER_TEST_STD")
        .expect("set WAFER_TEST_STD to its matching std directory");
    Command::new(env!("CARGO_BIN_EXE_wafer"))
        .current_dir(root)
        .arg("--sioxc")
        .arg(compiler)
        .arg("--std")
        .arg(std)
        .args(args)
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn project(root: &Path) {
    write(
        root,
        "dep/Die.toml",
        "[die]\nname = \"dep\"\nversion = \"0.1.0\"\n",
    );
    write(root, "dep/src/lib.siox", "module dep; pub fn answer() -> integer { return 42; } #[test] entity DoNotRun {} impl DoNotRun { process { assert!(false, \"dependency test must not run\"); } }");
    write(
        root,
        "app/Die.toml",
        "[die]\nname = \"app\"\nversion = \"0.1.0\"\n[dependencies]\ndep = { path = \"../dep\" }\n",
    );
    write(root, "app/src/lib.siox", "module app; use dep::answer; pub entity Top { value: integer out } impl Top { value = answer(); } #[test] entity Unit {} impl Unit { process { assert!(answer() == 42, \"unit\"); } }");
    write(root, "app/tests/smoke.siox", "module app::smoke; use app::Top; #[test] entity Smoke {} impl Smoke { let dut: Top = {}; process { await 1ns; assert!(dut.value == 42, \"actual native simulation\"); } }");
}

#[test]
#[ignore = "requires an actual sioxc with module inputs/test planning and matching std"]
fn real_check_object_build_native_tests_filter_listing_and_vcd_fst() {
    let dir = Directory::new();
    project(&dir.0);
    let app = dir.0.join("app");
    assert_success(&cli(&app, &["check"]));
    assert_success(&cli(&app, &["build", "--top", "Top"]));
    let object = app.join("target/wafer/release/app.o");
    assert!(std::fs::metadata(object).unwrap().len() > 0);
    let list = cli(&app, &["test", "--list"]);
    assert_success(&list);
    let names = String::from_utf8(list.stdout).unwrap();
    assert!(names.contains("app::Unit"));
    assert!(names.contains("app::smoke::Smoke"));
    assert!(!names.contains("DoNotRun"));
    let filtered = cli(&app, &["test", "app::smoke::Smoke", "--wave", "both"]);
    assert_success(&filtered);
    let stdout = String::from_utf8(filtered.stdout).unwrap();
    assert!(stdout.contains("app::smoke::Smoke ... ok"));
    assert!(!stdout.contains("app::Unit ..."));
    assert!(!stdout.contains("DoNotRun"));
    let waves: Vec<_> = std::fs::read_dir(app.join("target/wafer/release/tests/waves"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    let vcd = waves
        .iter()
        .find(|path| path.extension().is_some_and(|ext| ext == "vcd"))
        .unwrap();
    let fst = waves
        .iter()
        .find(|path| path.extension().is_some_and(|ext| ext == "fst"))
        .unwrap();
    assert!(std::fs::read_to_string(vcd)
        .unwrap()
        .contains("$enddefinitions"));
    assert!(std::fs::metadata(fst).unwrap().len() > 0);
    let all = cli(&app, &["test"]);
    assert_success(&all);
    let stdout = String::from_utf8(all.stdout).unwrap();
    assert_eq!(stdout.matches("app::Unit ... ok").count(), 1);
    assert_eq!(stdout.matches("app::smoke::Smoke ... ok").count(), 1);
    assert!(!stdout.contains("DoNotRun"));
}

#[test]
#[ignore = "requires an actual sioxc with module inputs/test planning and matching std"]
fn real_compiler_and_native_failures_are_not_hidden() {
    let dir = Directory::new();
    project(&dir.0);
    let app = dir.0.join("app");
    write(&app, "tests/failing.siox", "module app::failing; #[test] entity Failing {} impl Failing { process { assert!(false, \"failure sentinel\"); } }");
    let failure = cli(&app, &["test", "Failing"]);
    assert!(!failure.status.success());
    assert!(String::from_utf8_lossy(&failure.stdout).contains("failure sentinel"));
    write(
        &app,
        "src/lib.siox",
        "module app; pub struct Broken { value: Missing }",
    );
    let compiler = cli(&app, &["check"]);
    assert!(!compiler.status.success());
    let stderr = String::from_utf8_lossy(&compiler.stderr);
    assert!(stderr.contains("Missing"));
    assert!(stderr.contains("src/lib.siox"));
}

#[test]
#[ignore = "requires an actual sioxc with module inputs/test planning and matching std"]
fn a_source_only_library_can_check_and_test_successfully_with_no_native_tests() {
    let dir = Directory::new();
    write(
        &dir.0,
        "Die.toml",
        "[die]\nname = \"plain\"\nversion = \"0.1.0\"\n",
    );
    write(
        &dir.0,
        "src/lib.siox",
        "module plain; pub fn identity(value: integer) -> integer { return value; }",
    );
    assert_success(&cli(&dir.0, &["check"]));
    let tests = cli(&dir.0, &["test"]);
    assert_success(&tests);
    assert!(String::from_utf8_lossy(&tests.stderr).contains("Finished 0 test targets"));
}

#[test]
#[ignore = "requires an actual sioxc with module inputs/test planning and matching std"]
fn a_freshly_scaffolded_die_builds_and_runs_without_manual_source_edits() {
    let dir = Directory::new();
    assert_success(&cli(&dir.0, &["new", "fresh-die"]));
    let root = dir.0.join("fresh-die");
    assert_success(&cli(&root, &["check"]));
    assert_success(&cli(&root, &["build"]));
    let output = cli(&root, &["test", "--wave", "vcd"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("fresh_die::Smoke ... ok"));
}

#[test]
#[ignore = "requires an actual sioxc with module inputs/test planning and matching std"]
fn real_native_execution_uses_a_git_die_offline_at_its_locked_commit() {
    let dir = Directory::new();
    project(&dir.0);
    let remote = dir.0.join("dep");
    for args in [
        vec!["init", "-qb", "main"],
        vec!["add", "."],
        vec!["commit", "-qm", "fixture"],
    ] {
        let output = Command::new("git")
            .current_dir(&remote)
            .args([
                "-c",
                "user.name=Wafer Test",
                "-c",
                "user.email=wafer@example.invalid",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let app = dir.0.join("app");
    write(
        &app,
        "Die.toml",
        &format!(
            "[die]\nname = \"app\"\nversion = \"0.1.0\"\n[dependencies]\ndep = {{ git = {:?} }}\n",
            remote.display().to_string()
        ),
    );
    assert_success(&cli(&app, &["fetch"]));
    std::fs::rename(remote, dir.0.join("unavailable-remote")).unwrap();
    assert_success(&cli(&app, &["check", "--offline", "--locked"]));
    assert_success(&cli(
        &app,
        &["build", "--top", "Top", "--offline", "--locked"],
    ));
    let test = cli(&app, &["test", "--offline", "--locked"]);
    assert_success(&test);
    assert!(String::from_utf8_lossy(&test.stdout).contains("app::smoke::Smoke ... ok"));
    assert!(!String::from_utf8_lossy(&test.stdout).contains("DoNotRun"));
}

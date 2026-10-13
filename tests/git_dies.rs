//! Exercise actual Git repositories; no hosted registry or external network.

use serde_json::Value;
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
            "wafer_git_{}_{}",
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

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
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
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn die(root: &Path, name: &str, dependencies: &str) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.siox"),
        format!("module {name}; pub const VALUE: integer = 1;"),
    )
    .unwrap();
    std::fs::write(
        root.join("Die.toml"),
        format!("[die]\nname = {name:?}\nversion = \"0.1.0\"\n[dependencies]\n{dependencies}\n"),
    )
    .unwrap();
}

fn commit(root: &Path) -> String {
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "fixture"]);
    git(root, &["rev-parse", "HEAD"])
}

fn cli(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wafer"))
        .current_dir(root)
        .args(arguments)
        .output()
        .unwrap()
}

fn metadata(root: &Path, arguments: &[&str]) -> Value {
    let output = cli(root, arguments);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn package<'a>(json: &'a Value, name: &str) -> &'a Value {
    json["dies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|die| die["name"] == name)
        .unwrap()
}

#[test]
fn git_refs_are_pinned_until_update_and_old_checkouts_survive() {
    let dir = Directory::new();
    let remote = dir.0.join("remote");
    die(&remote, "dep", "");
    git(&remote, &["init", "-qb", "main"]);
    let first = commit(&remote);
    let app = dir.0.join("app");
    die(
        &app,
        "app",
        &format!(
            "dep = {{ git = {:?}, version = \"^0.1\" }}",
            remote.display().to_string()
        ),
    );
    let json = metadata(&app, &["metadata"]);
    let initial = package(&json, "dep");
    assert_eq!(initial["source"]["revision"], first);
    let original_entry = PathBuf::from(initial["entry"].as_str().unwrap());
    let lock = std::fs::read(app.join("Wafer.lock")).unwrap();
    std::fs::write(
        remote.join("src/lib.siox"),
        "module dep; pub const VALUE: integer = 2;",
    )
    .unwrap();
    let second = commit(&remote);
    assert_ne!(first, second);
    assert_eq!(
        package(&metadata(&app, &["metadata", "--locked"]), "dep")["source"]["revision"],
        first
    );
    assert_eq!(std::fs::read(app.join("Wafer.lock")).unwrap(), lock);
    let update = cli(&app, &["update"]);
    assert!(
        update.status.success(),
        "{}",
        String::from_utf8_lossy(&update.stderr)
    );
    let json = metadata(&app, &["metadata", "--offline", "--locked"]);
    assert_eq!(package(&json, "dep")["source"]["revision"], second);
    assert!(std::fs::read_to_string(&original_entry)
        .unwrap()
        .contains("= 1"));
    assert!(
        std::fs::read_to_string(json["modules"]["dep"].as_str().unwrap())
            .unwrap()
            .contains("= 2")
    );
}

#[test]
fn cached_pins_work_with_an_unavailable_remote_and_update_preserves_the_lock_on_failure() {
    let dir = Directory::new();
    let remote = dir.0.join("remote");
    die(&remote, "dep", "");
    git(&remote, &["init", "-qb", "main"]);
    commit(&remote);
    let app = dir.0.join("app");
    die(
        &app,
        "app",
        &format!("dep = {{ git = {:?} }}", remote.display().to_string()),
    );
    metadata(&app, &["metadata"]);
    let lock = std::fs::read(app.join("Wafer.lock")).unwrap();
    std::fs::rename(&remote, dir.0.join("unavailable-remote")).unwrap();
    metadata(&app, &["metadata", "--offline", "--locked"]);
    metadata(&app, &["metadata", "--locked"]);
    assert!(!cli(&app, &["update"]).status.success());
    assert_eq!(std::fs::read(app.join("Wafer.lock")).unwrap(), lock);
}

#[test]
fn git_subdirectory_dies_keep_sibling_path_dependencies_in_the_same_commit() {
    let dir = Directory::new();
    let remote = dir.0.join("remote");
    die(&remote.join("a"), "a", "b = { path = \"../b\" }");
    die(&remote.join("b"), "b", "");
    git(&remote, &["init", "-qb", "main"]);
    let revision = commit(&remote);
    let app = dir.0.join("app");
    die(
        &app,
        "app",
        &format!(
            "a = {{ git = {:?}, subdir = \"a\", rev = {revision:?} }}",
            remote.display().to_string()
        ),
    );
    let json = metadata(&app, &["metadata"]);
    for name in ["a", "b"] {
        assert_eq!(package(&json, name)["source"]["revision"], revision);
    }
    assert_eq!(package(&json, "b")["source"]["subdir"], "b");
    metadata(&app, &["metadata", "--offline", "--locked"]);
}

#[test]
fn offline_missing_cache_dirty_checkouts_and_invalid_lock_versions_are_rejected() {
    let dir = Directory::new();
    let remote = dir.0.join("remote");
    die(&remote, "dep", "");
    git(&remote, &["init", "-qb", "main"]);
    commit(&remote);
    let app = dir.0.join("app");
    die(
        &app,
        "app",
        &format!("dep = {{ git = {:?} }}", remote.display().to_string()),
    );
    let missing = cli(&app, &["fetch", "--offline"]);
    assert!(!missing.status.success());
    assert!(!app.join("Wafer.lock").exists());
    let json = metadata(&app, &["metadata"]);
    let entry = PathBuf::from(package(&json, "dep")["entry"].as_str().unwrap());
    std::fs::write(&entry, "keep my edits").unwrap();
    let dirty = cli(&app, &["fetch", "--offline", "--locked"]);
    assert!(!dirty.status.success());
    assert!(String::from_utf8_lossy(&dirty.stderr).contains("edits were preserved"));
    assert_eq!(std::fs::read_to_string(entry).unwrap(), "keep my edits");
    std::fs::write(app.join("Wafer.lock"), "version = 999\ndie = []\n").unwrap();
    let version = cli(&app, &["metadata"]);
    assert!(!version.status.success());
    assert!(String::from_utf8_lossy(&version.stderr).contains("unsupported Wafer.lock"));
}

#[test]
fn unsafe_sources_and_ambiguous_specs_fail_without_running_a_git_helper() {
    let dir = Directory::new();
    let app = dir.0.join("app");
    for spec in [
        "{ git = \"ext::sh -c ignored\" }",
        "{ git = \"https://user:secret@example.invalid/repo\" }",
        "{ git = \"ssh://user:secret@example.invalid/repo\" }",
        "{ git = \"file:///absent\", path = \"../other\" }",
        "{ path = \"../other\", rev = \"main\" }",
        "{ git = \"file:///absent\", rev = \"--help\" }",
        "{ git = \"file:///absent\", subdir = \"../escape\" }",
    ] {
        die(&app, "app", &format!("dep = {spec}"));
        assert!(!cli(&app, &["fetch"]).status.success(), "{spec}");
        assert!(!app.join(".wafer/git").exists());
    }
}

#[test]
fn locked_selector_changes_and_offline_update_never_rewrite_the_lock() {
    let dir = Directory::new();
    let remote = dir.0.join("remote");
    die(&remote, "dep", "");
    git(&remote, &["init", "-qb", "main"]);
    commit(&remote);
    git(&remote, &["tag", "v1"]);
    let app = dir.0.join("app");
    let url = remote.display().to_string();
    die(&app, "app", &format!("dep = {{ git = {url:?} }}"));
    metadata(&app, &["metadata"]);
    let lock = std::fs::read(app.join("Wafer.lock")).unwrap();
    die(
        &app,
        "app",
        &format!("dep = {{ git = {url:?}, rev = \"v1\" }}"),
    );
    assert!(!cli(&app, &["metadata", "--locked"]).status.success());
    assert!(!cli(&app, &["update", "--offline"]).status.success());
    assert!(!cli(&app, &["update", "--locked"]).status.success());
    assert_eq!(std::fs::read(app.join("Wafer.lock")).unwrap(), lock);
    metadata(&app, &["metadata"]);
    metadata(&app, &["metadata", "--offline", "--locked"]);
}

#[test]
fn git_path_dependencies_cannot_import_host_dies_outside_the_checkout() {
    let dir = Directory::new();
    let host = dir.0.join("host");
    die(&host, "host", "");
    let remote = dir.0.join("remote");
    die(
        &remote,
        "dep",
        &format!("host = {{ path = {:?} }}", host.display().to_string()),
    );
    git(&remote, &["init", "-qb", "main"]);
    commit(&remote);
    let app = dir.0.join("app");
    die(
        &app,
        "app",
        &format!("dep = {{ git = {:?} }}", remote.display().to_string()),
    );
    let output = cli(&app, &["metadata"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("escapes its locked checkout"));
    assert!(!app.join("Wafer.lock").exists());
}

#[test]
fn ignored_untracked_source_files_cannot_change_a_locked_git_die() {
    let dir = Directory::new();
    let remote = dir.0.join("remote");
    die(&remote, "dep", "");
    std::fs::write(remote.join(".gitignore"), "secret/\n").unwrap();
    git(&remote, &["init", "-qb", "main"]);
    commit(&remote);
    let app = dir.0.join("app");
    let url = remote.display().to_string();
    die(&app, "app", &format!("dep = {{ git = {url:?} }}"));
    let json = metadata(&app, &["metadata"]);
    let entry = PathBuf::from(package(&json, "dep")["entry"].as_str().unwrap());
    let checkout = entry.parent().unwrap().parent().unwrap();
    die(&checkout.join("secret"), "dep", "");
    die(
        &app,
        "app",
        &format!("dep = {{ git = {url:?}, subdir = \"secret\" }}"),
    );
    let output = cli(&app, &["metadata"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not tracked in its locked commit"));
}

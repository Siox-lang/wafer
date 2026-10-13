use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

use wafer::{
    manifest,
    project::{self, Project},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "wafer_packages_{}_{}",
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

fn die(root: &Path, name: &str, version: &str, dependencies: &str) -> PathBuf {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.siox"),
        format!("module {};", name.replace('-', "_")),
    )
    .unwrap();
    let path = root.join("Die.toml");
    std::fs::write(
        &path,
        format!("[die]\nname = {name:?}\nversion = {version:?}\n[dependencies]\n{dependencies}"),
    )
    .unwrap();
    path
}

fn cli(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wafer"))
        .current_dir(root)
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn names_use_siox_identifiers_and_reserve_core_std() {
    assert_eq!(
        manifest::namespace("spi-controller").unwrap(),
        "spi_controller"
    );
    for name in [
        "", "3bad", "std", "core", "entity", "a/b", "bad name", "a::b",
    ] {
        assert!(manifest::namespace(name).is_err(), "{name}");
    }
}

#[test]
fn diamond_dependencies_are_ordered_and_shared_once() {
    let dir = Directory::new();
    die(&dir.0.join("shared"), "shared", "1.2.0", "");
    die(
        &dir.0.join("left"),
        "left",
        "0.1.0",
        "shared = { path = \"../shared\", version = \"^1\" }",
    );
    die(
        &dir.0.join("right"),
        "right",
        "0.1.0",
        "shared = { path = \"../shared\", version = \"1.2\" }",
    );
    let root = die(
        &dir.0.join("app"),
        "app",
        "0.1.0",
        "left = { path = \"../left\" }\nright = { path = \"../right\" }",
    );
    let project = Project::load(&root).unwrap();
    assert_eq!(
        project
            .dies
            .iter()
            .map(|die| die.name.as_str())
            .collect::<Vec<_>>(),
        ["shared", "left", "right", "app"]
    );
    assert_eq!(project.modules.len(), 4);
    assert_eq!(project.root, "app");
}

#[test]
fn cycles_version_mismatches_aliases_and_duplicate_versions_fail() {
    let dir = Directory::new();
    let app = dir.0.join("app");
    let other = dir.0.join("other");
    let root = die(&app, "app", "0.1.0", "other = { path = \"../other\" }");
    die(&other, "other", "0.1.0", "app = { path = \"../app\" }");
    assert!(Project::load(&root)
        .unwrap_err()
        .to_string()
        .contains("dependency cycle"));
    die(&other, "other", "2.0.0", "");
    die(
        &app,
        "app",
        "0.1.0",
        "other = { path = \"../other\", version = \"^1\" }",
    );
    assert!(Project::load(&root)
        .unwrap_err()
        .to_string()
        .contains("does not satisfy"));
    die(&app, "app", "0.1.0", "alias = { path = \"../other\" }");
    assert!(Project::load(&root)
        .unwrap_err()
        .to_string()
        .contains("renaming"));
    die(&other, "app", "2.0.0", "");
    die(&app, "app", "0.1.0", "app = { path = \"../other\" }");
    assert!(Project::load(&root)
        .unwrap_err()
        .to_string()
        .contains("multiple sources/versions"));
}

#[test]
fn source_paths_and_module_names_cannot_escape_the_die() {
    let dir = Directory::new();
    let root = die(&dir.0.join("app"), "app", "0.1.0", "");
    std::fs::write(dir.0.join("external.siox"), "module external;").unwrap();
    let base = "[die]\nname = \"app\"\nversion = \"0.1.0\"\n";
    for module in ["other::helper", "app::entity", "app::"] {
        std::fs::write(
            &root,
            format!("{base}[modules]\n{module:?} = \"src/lib.siox\"\n"),
        )
        .unwrap();
        assert!(Project::load(&root).is_err(), "{module}");
    }
    std::fs::write(&root, format!("{base}[lib]\npath = \"../external.siox\"\n")).unwrap();
    assert!(Project::load(&root)
        .unwrap_err()
        .to_string()
        .contains("inside"));
    std::fs::write(&root, format!("{base}misspelled = 42\n")).unwrap();
    assert!(Project::load(&root).is_err());
}

#[test]
fn lockfiles_are_stable_and_locked_mode_never_rewrites() {
    let dir = Directory::new();
    let path = die(&dir.0, "app", "0.1.0", "");
    let lock = dir.0.join("Wafer.lock");
    let project = Project::load(&path).unwrap();
    assert!(project.write_lock(&lock, true).is_err());
    assert!(!lock.exists());
    project.write_lock(&lock, false).unwrap();
    let original = std::fs::read(&lock).unwrap();
    project.write_lock(&lock, true).unwrap();
    assert_eq!(std::fs::read(&lock).unwrap(), original);
    die(&dir.0, "app", "0.2.0", "");
    let changed = Project::load(&path).unwrap();
    assert!(changed.write_lock(&lock, true).is_err());
    assert_eq!(std::fs::read(&lock).unwrap(), original);
    changed.write_lock(&lock, false).unwrap();
    assert_ne!(std::fs::read(&lock).unwrap(), original);
}

#[test]
fn file_lock_releases_on_drop_and_discovery_walks_upwards() {
    let dir = Directory::new();
    let path = die(&dir.0, "app", "0.1.0", "");
    std::fs::create_dir_all(dir.0.join("src/nested")).unwrap();
    assert_eq!(
        project::discover(&dir.0.join("src/nested")).unwrap(),
        path.canonicalize().unwrap()
    );
    let lock = project::project_lock(&dir.0).unwrap();
    assert!(project::project_lock(&dir.0).is_err());
    drop(lock);
    assert!(project::project_lock(&dir.0).is_ok());
}

#[test]
fn cli_scaffolds_metadata_json_and_refuses_overwrites() {
    let dir = Directory::new();
    let new = cli(&dir.0, &["new", "spi-controller"]);
    assert!(
        new.status.success(),
        "{}",
        String::from_utf8_lossy(&new.stderr)
    );
    assert!(new.stdout.is_empty());
    let root = dir.0.join("spi-controller");
    assert!(std::fs::read_to_string(root.join("src/lib.siox"))
        .unwrap()
        .contains("module spi_controller;"));
    let metadata = cli(&root, &["metadata"]);
    assert!(
        metadata.status.success(),
        "{}",
        String::from_utf8_lossy(&metadata.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    assert_eq!(json["root"], "spi-controller");
    assert!(json["modules"]["spi_controller"]
        .as_str()
        .unwrap()
        .ends_with("src/lib.siox"));
    assert!(cli(&root, &["metadata", "--locked"]).status.success());
    assert!(!cli(&dir.0, &["new", "spi-controller"]).status.success());
    let original = std::fs::read(root.join("src/lib.siox")).unwrap();
    assert!(!cli(&root, &["init", "."]).status.success());
    assert_eq!(std::fs::read(root.join("src/lib.siox")).unwrap(), original);
}

#[test]
fn init_preserves_existing_files_and_gitignore() {
    let dir = Directory::new();
    std::fs::write(dir.0.join(".gitignore"), "keep me\n").unwrap();
    std::fs::write(dir.0.join("notes.txt"), "keep notes").unwrap();
    let output = cli(&dir.0, &["init", ".", "--name", "existing"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(dir.0.join(".gitignore")).unwrap(),
        "keep me\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.0.join("notes.txt")).unwrap(),
        "keep notes"
    );
}

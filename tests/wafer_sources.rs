//! Exercise the existing Cargo path/Git source walker and source fingerprint.

use cargo::{
    core::{Package, Shell, SourceId},
    sources::{path::PathSource, source::Source},
    util::GlobalContext,
};
use cargo_test_support::{cargo_test, git};
use std::{collections::BTreeSet, path::Path};

fn write(root: &Path, path: &str, value: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, value).unwrap();
}

fn context(root: &Path, wafer: bool) -> GlobalContext {
    let mut context = if wafer {
        GlobalContext::new_wafer(Shell::new(), root.into(), root.join("home"))
    } else {
        GlobalContext::new(Shell::new(), root.into(), root.join("home"))
    };
    // No user configuration is needed for source enumeration.
    context.set_values(Default::default()).unwrap();
    context.set_env(Default::default());
    context
}

fn package(root: &Path, context: &GlobalContext) -> Package {
    PathSource::new(root, SourceId::for_path(root).unwrap(), context)
        .root_package()
        .unwrap()
}

fn files(package: &Package, context: &GlobalContext) -> BTreeSet<String> {
    PathSource::preload_with(package.clone(), context)
        .list_files(package)
        .unwrap()
        .into_iter()
        .map(|entry| {
            entry
                .strip_prefix(package.root())
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/")
        })
        .collect()
}

#[test]
fn filesystem_walk_stops_at_active_package_boundaries_not_other_language_manifests() {
    for wafer in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let context = context(root, wafer);
        let manifest = context.manifest_filename();
        let lock = context.lockfile_filename();
        let extension = if wafer { "siox" } else { "rs" };
        let foreign = if wafer { "Cargo.toml" } else { "Wafer.toml" };
        write(root, manifest, "[package]\nname='root'\nversion='1.0.0'\n");
        write(root, lock, "lock contents");
        write(root, &format!("src/lib.{extension}"), "source");
        write(root, &format!("child/{manifest}"), "nested package");
        write(root, &format!("child/src/lib.{extension}"), "nested source");
        write(root, &format!("foreign/{foreign}"), "ordinary data");
        write(root, "foreign/data.txt", "ordinary data");
        write(root, "target/generated.txt", "build output");
        write(root, ".hidden.txt", "untracked hidden file");
        let package = package(root, &context);
        assert_eq!(
            files(&package, &context),
            [
                manifest.to_owned(),
                lock.to_owned(),
                format!("src/lib.{extension}"),
                format!("foreign/{foreign}"),
                "foreign/data.txt".into(),
            ]
            .into()
        );
    }
}

#[test]
fn include_and_exclude_rules_keep_the_active_root_manifest_and_lockfile() {
    for wafer in [false, true] {
        for selection in ["include=['src/**']", "exclude=['**']"] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            let context = context(root, wafer);
            let manifest = context.manifest_filename();
            let lock = context.lockfile_filename();
            let source = if wafer { "src/lib.siox" } else { "src/lib.rs" };
            write(
                root,
                manifest,
                &format!("[package]\nname='root'\nversion='1.0.0'\n{selection}\n"),
            );
            write(root, source, "source");
            write(root, lock, "lock contents");
            write(root, "data.txt", "excluded");
            let package = package(root, &context);
            let mut expected: BTreeSet<_> = [manifest.to_owned(), lock.to_owned()].into();
            if selection.starts_with("include") {
                expected.insert(source.to_owned());
            }
            assert_eq!(files(&package, &context), expected);
        }
    }
}

#[cargo_test]
fn tracked_active_manifests_anchor_git_ignore_and_nested_package_selection() {
    for wafer in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        let context = context(&root, wafer);
        let manifest = context.manifest_filename();
        let lock = context.lockfile_filename();
        let source = if wafer { "src/lib.siox" } else { "src/lib.rs" };
        let repository = git::repo(&root)
            .file(manifest, "[package]\nname='root'\nversion='1.0.0'\n")
            .file(source, "source")
            .file(".gitignore", "scratch.txt\ntarget/\n")
            .file(".tracked-hidden", "tracked data")
            .file(&format!("child/{manifest}"), "nested package")
            .file("child/data.txt", "nested data")
            .build();
        write(&root, lock, "untracked lock");
        write(&root, "scratch.txt", "ignored");
        write(&root, "target/generated.txt", "ignored output");
        write(&root, "new.txt", "untracked source data");
        let package = package(&root, &context);
        let mut expected: BTreeSet<_> = [
            manifest.to_owned(),
            source.to_owned(),
            ".gitignore".into(),
            ".tracked-hidden".into(),
            "new.txt".into(),
        ]
        .into();
        assert_eq!(files(&package, &context), expected);
        let native = git2::Repository::open(repository.root()).unwrap();
        let mut index = native.index().unwrap();
        index.add_path(Path::new(lock)).unwrap();
        index.write().unwrap();
        expected.insert(lock.to_owned());
        assert_eq!(files(&package, &context), expected);
    }
}

#[cargo_test]
fn source_freshness_tracks_selected_files_without_nested_or_generated_noise() {
    for wafer in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        let context = context(&root, wafer);
        let manifest = context.manifest_filename();
        let lock = context.lockfile_filename();
        let input = if wafer { "src/lib.siox" } else { "src/lib.rs" };
        let _repository = git::repo(&root)
            .file(manifest, "[package]\nname='root'\nversion='1.0.0'\n")
            .file(input, "source")
            .file(".gitignore", "scratch.txt\n")
            .file(&format!("child/{manifest}"), "nested package")
            .file("child/data.txt", "nested data")
            .build();
        let package = package(&root, &context);
        let source = PathSource::preload_with(package.clone(), &context);
        for file in source.list_files(&package).unwrap() {
            filetime::set_file_mtime(file.as_ref(), filetime::FileTime::from_unix_time(1000, 0))
                .unwrap();
        }
        let original = source.fingerprint(&package).unwrap();
        for path in [
            lock,
            "scratch.txt",
            "child/data.txt",
            "target/generated.txt",
        ] {
            write(&root, path, "must not affect this package");
        }
        assert_eq!(source.fingerprint(&package).unwrap(), original);
        filetime::set_file_mtime(
            root.join(input),
            filetime::FileTime::from_unix_time(2000, 0),
        )
        .unwrap();
        let changed = source.fingerprint(&package).unwrap();
        assert_ne!(changed, original);
        write(&root, "new.txt", "new untracked source data");
        filetime::set_file_mtime(
            root.join("new.txt"),
            filetime::FileTime::from_unix_time(3000, 0),
        )
        .unwrap();
        assert_ne!(source.fingerprint(&package).unwrap(), changed);
    }
}

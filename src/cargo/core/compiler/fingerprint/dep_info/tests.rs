//! Actual Sioxc outputs feed Cargo's existing dependency encoding and rebasing.

use super::{DepInfoFormat, parse_dep_info, parse_sioxc_dep_info, translate_dep_info};
use cargo_util::ProcessBuilder;
use std::{collections::HashMap, ffi::OsString, path::Path, process::Command, sync::Arc};

fn tool(name: &str) -> OsString {
    // Native acceptance needs the real compiler and its actual std checkout.
    #[allow(clippy::disallowed_methods)]
    std::env::var_os(name).unwrap_or_else(|| panic!("set {name}"))
}

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn compile(source: &Path, native: &Path, depfile: &Path, test: bool) {
    let mut command = Command::new(tool("WAFER_TEST_SIOXC"));
    command
        .arg(source)
        .arg("--out")
        .arg(native)
        .arg("--dep-info")
        .arg(depfile)
        .arg("--std")
        .arg(tool("WAFER_TEST_STD"));
    if test {
        command.arg("--test");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(std::fs::metadata(native).unwrap().len() > 0);
}

fn translate(input: &Path, output: &Path, package: &Path, build: &Path) -> crate::CargoResult<()> {
    translate_dep_info(
        input,
        output,
        package,
        package,
        build,
        &ProcessBuilder::new(tool("WAFER_TEST_SIOXC")),
        true,
        &Arc::new(HashMap::new()),
        DepInfoFormat::Sioxc,
    )
}

fn program() -> &'static str {
    "module unit; entity Dut { value: integer out } impl Dut { value = 7; }\n"
}

#[cfg(unix)]
#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_STD"]
fn actual_native_sidecars_decode_literal_source_paths_without_fake_core_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for name in [
        "plain.siox",
        "space #cash$colon:slash\\.siox",
        "glob[ab]*?percent%pipe|semi;.siox",
        "quoted\\;semi.siox",
        "back\\ space (paren).siox",
        "<core>/attrs.siox",
    ] {
        let source = root.join(name);
        write(&source, program());
        let native = root.join("native.o");
        let depfile = root.join("native.d");
        compile(&source, &native, &depfile, false);
        let info = parse_sioxc_dep_info(&depfile).unwrap();
        assert!(info.files.contains_key(&source), "{name}");
        assert!(info.files.len() > 1, "std files were not tracked");
        assert!(info.files.keys().all(|path| path.is_file()));
        assert!(info.files.values().all(Option::is_none));
        assert!(info.env.is_empty());
        let encoded = root.join("native.inputs");
        translate(&depfile, &encoded, root, root).unwrap();
        let decoded = parse_dep_info(root, root, &encoded).unwrap().unwrap();
        assert_eq!(info.files, decoded.files);
    }
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_STD"]
fn encoded_real_inputs_rebase_package_and_build_roots_and_preserve_old_data_on_error() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("package");
    let build = dir.path().join("build");
    std::fs::create_dir_all(&build).unwrap();
    for (name, source) in [
        ("package", package.join("src/input.siox")),
        ("generated", build.join("generated.siox")),
    ] {
        write(&source, program());
        let native = build.join(format!("{name}.o"));
        let depfile = build.join(format!("{name}.d"));
        compile(&source, &native, &depfile, false);
        let encoded = build.join(format!("{name}.inputs"));
        translate(&depfile, &encoded, &package, &build).unwrap();
        let before = std::fs::read(&encoded).unwrap();
        std::fs::write(&depfile, "truncated dependency record").unwrap();
        assert!(translate(&depfile, &encoded, &package, &build).is_err());
        assert_eq!(std::fs::read(&encoded).unwrap(), before);
    }
    let moved_package = dir.path().join("moved-package");
    let moved_build = dir.path().join("moved-build");
    std::fs::rename(&package, &moved_package).unwrap();
    std::fs::rename(&build, &moved_build).unwrap();
    for (name, expected) in [
        ("package", moved_package.join("src/input.siox")),
        ("generated", moved_build.join("generated.siox")),
    ] {
        let info = parse_dep_info(
            &moved_package,
            &moved_build,
            &moved_build.join(format!("{name}.inputs")),
        )
        .unwrap()
        .unwrap();
        assert!(info.files.contains_key(&expected));
        assert!(info.files.keys().all(|path| path.is_file()));
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_STD"]
fn encoded_inputs_keep_symlink_aliases_and_parent_traversal_observable() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let build = root.join("target");
    std::fs::create_dir(&build).unwrap();
    let a = root.join("a.siox");
    let b = root.join("b.siox");
    write(&a, program());
    write(&b, &program().replace("7", "9"));
    let alias = root.join("input.siox");
    std::os::unix::fs::symlink(&a, &alias).unwrap();
    let depfile = build.join("alias.d");
    compile(&alias, &build.join("alias.o"), &depfile, false);
    let encoded = build.join("alias.inputs");
    translate(&depfile, &encoded, root, &build).unwrap();
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(&b, &alias).unwrap();
    let info = parse_dep_info(root, &build, &encoded).unwrap().unwrap();
    assert!(info.files.contains_key(&alias));
    assert!(!info.files.contains_key(&a));
    assert_eq!(
        std::fs::read_to_string(&alias).unwrap(),
        std::fs::read_to_string(&b).unwrap()
    );

    let external = dir.path().join("external");
    std::fs::create_dir_all(external.join("child")).unwrap();
    write(&external.join("input.siox"), program());
    std::os::unix::fs::symlink(external.join("child"), root.join("link")).unwrap();
    let traversal = root.join("link/../input.siox");
    let depfile = build.join("parent.d");
    compile(&traversal, &build.join("parent.o"), &depfile, false);
    let encoded = build.join("parent.inputs");
    translate(&depfile, &encoded, root, &build).unwrap();
    let info = parse_dep_info(root, &build, &encoded).unwrap().unwrap();
    assert!(info.files.contains_key(&traversal));
    assert_eq!(std::fs::read_to_string(&traversal).unwrap(), program());
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_STD"]
fn actual_native_test_binary_keeps_usable_dependency_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("smoke.siox");
    write(
        &source,
        "module smoke; #[test] entity Smoke {} impl Smoke { process { assert!(1 + 1 == 2); } }\n",
    );
    let native = dir.path().join("smoke.test");
    let depfile = dir.path().join("smoke.d");
    compile(&source, &native, &depfile, true);
    let info = parse_sioxc_dep_info(&depfile).unwrap();
    assert!(info.files.contains_key(&source));
    let ran = Command::new(&native)
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        ran.status.success(),
        "{}{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
}

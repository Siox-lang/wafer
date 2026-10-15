//! Both compiler providers exercise Cargo's actual resolve/feature pipeline.

use cargo_test_support::{cargo_test, git};
use std::{
    ffi::OsString,
    path::Path,
    process::{Command, Output},
};

const WAFER: &str = env!("CARGO_BIN_EXE_wafer");
const CARGO: &str = env!("CARGO_BIN_EXE_cargo");

fn write(root: &Path, path: &str, value: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, value).unwrap();
}

fn tool(name: &str) -> OsString {
    // Acceptance uses actual compilers selected explicitly by the guarded job.
    #[allow(clippy::disallowed_methods)]
    std::env::var_os(name).unwrap_or_else(|| panic!("set {name} to the actual compiler"))
}

fn command(binary: &str, root: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(binary);
    command
        .current_dir(root)
        .env_clear()
        .env("HOME", root.join("home"))
        .env("CARGO_HOME", root.join("home/.cargo"))
        .env("WAFER_HOME", root.join("home/.wafer"));
    if binary == WAFER {
        command
            .env("SIOXC", tool("WAFER_TEST_SIOXC"))
            .env("RUSTC", "must-not-run-rustc");
    } else {
        command
            .env("RUSTC", tool("WAFER_TEST_RUSTC"))
            .env("SIOXC", "must-not-run-sioxc");
    }
    command.args(args);
    command
}

fn metadata(output: Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[cargo_test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_RUSTC"]
fn full_metadata_uses_the_same_graph_and_feature_selection_as_cargo() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for manifest in ["Cargo.toml", "Wafer.toml"] {
        write(
            root,
            manifest,
            &format!(
                "[workspace]\nmembers=[]\nexclude=['dep','host_dep','other_dep']\n[package]\nname='controller'\nversion='1.0.0'\nresolver='2'\n[features]\ndefault=['extra']\nextra=['dep/selected']\n[dependencies]\ndep={{path='dep'}}\n[target.'cfg(target_os = {:?})'.dependencies]\nhost_dep={{path='host_dep'}}\n[target.'cfg(target_os = \"not-this-os\")'.dependencies]\nother_dep={{path='other_dep'}}\n",
                std::env::consts::OS
            ),
        );
        write(
            root,
            &format!("dep/{manifest}"),
            "[package]\nname='dep'\nversion='1.0.0'\n[lib]\nname='vendor'\n[features]\nselected=[]\n",
        );
        for name in ["host_dep", "other_dep"] {
            write(
                root,
                &format!("{name}/{manifest}"),
                &format!("[package]\nname='{name}'\nversion='1.0.0'\n"),
            );
        }
    }
    for prefix in ["", "dep/", "host_dep/", "other_dep/"] {
        for ext in ["rs", "siox"] {
            write(
                root,
                &format!("{prefix}src/lib.{ext}"),
                "metadata must not compile this",
            );
        }
    }
    let wafer = metadata(
        command(WAFER, root, &["metadata", "--format-version", "1"])
            .output()
            .unwrap(),
    );
    let cargo = metadata(
        command(CARGO, root, &["metadata", "--format-version", "1"])
            .output()
            .unwrap(),
    );
    assert_eq!(wafer["resolve"], cargo["resolve"]);
    assert_eq!(wafer["workspace_members"].as_array().unwrap().len(), 1);
    assert_eq!(wafer["packages"].as_array().unwrap().len(), 4);
    assert!(
        wafer["packages"]
            .as_array()
            .unwrap()
            .iter()
            .all(|package| package.get("edition").is_none())
    );
    let root_id = wafer["resolve"]["root"].clone();
    let node = wafer["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == root_id)
        .unwrap();
    assert!(
        node["deps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dep| dep["name"] == "vendor")
    );

    let version = Command::new(tool("WAFER_TEST_SIOXC"))
        .arg("-vV")
        .output()
        .unwrap();
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout).unwrap();
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap();
    for flags in [vec![], vec!["--no-default-features"]] {
        let mut args = vec![
            "metadata",
            "--format-version",
            "1",
            "--filter-platform",
            host,
        ];
        args.extend(flags.iter().copied());
        let wafer = metadata(command(WAFER, root, &args).output().unwrap());
        let cargo = metadata(command(CARGO, root, &args).output().unwrap());
        assert_eq!(wafer["resolve"], cargo["resolve"]);
        assert_eq!(
            wafer["packages"].as_array().unwrap().len(),
            3,
            "members={} resolve={}",
            wafer["workspace_members"],
            wafer["resolve"]
        );
        assert!(
            !wafer["packages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|package| package["name"] == "other_dep")
        );
        let dep_id = &wafer["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|package| package["name"] == "dep")
            .unwrap()["id"];
        let node = wafer["resolve"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| &node["id"] == dep_id)
            .unwrap();
        assert_eq!(
            node["features"]
                .as_array()
                .unwrap()
                .iter()
                .any(|feature| feature == "selected"),
            flags.is_empty()
        );
    }
}

#[cargo_test]
#[ignore = "requires actual WAFER_TEST_SIOXC"]
fn full_metadata_conservatively_reuses_git_pins_from_wafer_lock() {
    let dir = tempfile::tempdir().unwrap();
    let repo = git::repo(&dir.path().join("source"))
        .file("Wafer.toml", "[package]\nname='dep'\nversion='1.0.0'\n")
        .file("src/lib.siox", "first source")
        .build();
    let original_commit = repo.revparse_head();
    let root = dir.path().join("project");
    write(
        &root,
        "Wafer.toml",
        &format!(
            "[package]\nname='controller'\nversion='1.0.0'\n[dependencies]\ndep={{git={:?}}}\n",
            repo.url().as_str()
        ),
    );
    write(&root, "src/lib.siox", "metadata only");
    write(&root, "Cargo.lock", "unrelated Cargo data\n");
    let first = metadata(
        command(WAFER, &root, &["metadata", "--format-version", "1"])
            .output()
            .unwrap(),
    );
    assert_eq!(first["packages"].as_array().unwrap().len(), 2);
    let original_lock = std::fs::read(root.join("Wafer.lock")).unwrap();
    let native_repo = git2::Repository::open(repo.root()).unwrap();
    write(repo.root(), "src/lib.siox", "second source");
    git::add(&native_repo);
    let changed_commit = git::commit(&native_repo).to_string();
    assert_ne!(original_commit, changed_commit);
    for flags in [vec!["--locked"], vec!["--offline", "--locked"]] {
        let mut args = vec!["metadata", "--format-version", "1"];
        args.extend(flags);
        let result = metadata(command(WAFER, &root, &args).output().unwrap());
        let dep = result["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|package| package["name"] == "dep")
            .unwrap();
        assert!(dep["source"].as_str().unwrap().contains(&original_commit));
        assert!(!dep["source"].as_str().unwrap().contains(&changed_commit));
        assert_eq!(
            std::fs::read(root.join("Wafer.lock")).unwrap(),
            original_lock
        );
    }
    assert_eq!(
        std::fs::read_to_string(root.join("Cargo.lock")).unwrap(),
        "unrelated Cargo data\n"
    );
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC"]
fn no_deps_needs_no_compiler_and_cross_platform_facts_are_not_invented() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "Wafer.toml",
        "[package]\nname='controller'\nversion='1.0.0'\n",
    );
    write(root, "src/lib.siox", "metadata only");
    let result = metadata(
        command(
            WAFER,
            root,
            &["metadata", "--no-deps", "--format-version", "1"],
        )
        .env("SIOXC", "must-not-run-sioxc")
        .output()
        .unwrap(),
    );
    assert!(result["resolve"].is_null());
    let missing = command(WAFER, root, &["metadata", "--format-version", "1"])
        .env("SIOXC", "missing-sioxc")
        .output()
        .unwrap();
    assert!(!missing.status.success());
    let unsupported = command(
        WAFER,
        root,
        &[
            "metadata",
            "--format-version",
            "1",
            "--filter-platform",
            "unsupported-siox-cross",
        ],
    )
    .output()
    .unwrap();
    assert!(!unsupported.status.success());
    assert!(
        String::from_utf8_lossy(&unsupported.stderr).contains("cross-target"),
        "{}",
        String::from_utf8_lossy(&unsupported.stderr)
    );
    assert!(!root.join("Wafer.lock").exists());
}

#[cargo_test]
#[ignore = "requires actual WAFER_TEST_RUSTC and the native system linker"]
fn cargo_reference_still_builds_and_runs_through_the_shared_platform_resolver() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "Cargo.toml",
        "[package]\nname='controller'\nversion='1.0.0'\n[dependencies]\ndep={path='dep'}\n",
    );
    write(
        root,
        "dep/Cargo.toml",
        "[package]\nname='dep'\nversion='1.0.0'\n[lib]\nname='vendor'\n",
    );
    write(root, "dep/src/lib.rs", "pub fn answer() -> u8 { 7 }\n");
    write(
        root,
        "src/main.rs",
        "fn main() { assert_eq!(vendor::answer(), 7); }\n",
    );
    // The real system linker is discovered through the verification host's PATH.
    #[allow(clippy::disallowed_methods)]
    let path = std::env::var_os("PATH").expect("native build acceptance needs PATH");
    let built = command(CARGO, root, &["build", "--offline", "--timings"])
        .env("PATH", path)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr)
    );
    let timing =
        std::fs::read_to_string(root.join("target/cargo-timings/cargo-timing.html")).unwrap();
    assert!(timing.contains("Cargo Build Timings") && timing.contains("<td>rustc:</td>"));
    assert!(!timing.contains("Wafer Build Timings") && !timing.contains("<td>sioxc:</td>"));
    let ran = Command::new(root.join(format!(
        "target/debug/controller{}",
        std::env::consts::EXE_SUFFIX
    )))
    .current_dir(root)
    .output()
    .unwrap();
    assert!(
        ran.status.success(),
        "{}{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(!root.join("Wafer.lock").exists());
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_RUSTC"]
fn unit_graph_cli_uses_the_shared_context_without_compiling_source() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (manifest, extension) in [("Cargo.toml", "rs"), ("Wafer.toml", "siox")] {
        write(
            root,
            manifest,
            "[package]\nname='controller'\nversion='1.0.0'\nresolver='2'\n\
             [features]\ndefault=['extra']\nextra=['dep/selected']\n\
             [dependencies]\ndep={path='dep'}\n[profile.release]\nopt-level=2\n",
        );
        write(
            root,
            &format!("dep/{manifest}"),
            "[package]\nname='dep'\nversion='1.0.0'\n[lib]\nname='vendor'\n[features]\nselected=[]\n",
        );
        for path in ["src/lib", "src/main", "dep/src/lib"] {
            write(
                root,
                &format!("{path}.{extension}"),
                "planning must not compile this",
            );
        }
    }
    for flags in [vec![], vec!["--release", "--no-default-features", "--lib"]] {
        let mut args = vec!["build", "--unit-graph", "-Zunstable-options"];
        args.extend(flags);
        let mut graphs = Vec::new();
        for binary in [WAFER, CARGO] {
            let mut graph = metadata(
                command(binary, root, &args)
                    .env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "nightly")
                    .output()
                    .unwrap(),
            );
            assert_eq!(graph["version"], 1);
            assert!(graph["units"].as_array().unwrap().iter().any(|unit| {
                unit["dependencies"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|dep| dep["extern_crate_name"] == "vendor")
            }));
            for unit in graph["units"].as_array_mut().unwrap() {
                let target = unit["target"].as_object_mut().unwrap();
                if binary == WAFER {
                    assert!(!target.contains_key("edition") && !target.contains_key("crate_types"));
                    assert!(target["src_path"].as_str().unwrap().ends_with(".siox"));
                } else {
                    assert!(target.remove("edition").is_some());
                    assert!(target.remove("crate_types").is_some());
                    let path =
                        Path::new(target["src_path"].as_str().unwrap()).with_extension("siox");
                    target.insert("src_path".into(), path.to_str().unwrap().into());
                }
            }
            graphs.push(graph);
        }
        assert_eq!(graphs[0], graphs[1]);
    }
    let locked = std::fs::read(root.join("Wafer.lock")).unwrap();
    metadata(
        command(
            WAFER,
            root,
            &["build", "--unit-graph", "-Zunstable-options", "--locked"],
        )
        .env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "nightly")
        .output()
        .unwrap(),
    );
    assert_eq!(std::fs::read(root.join("Wafer.lock")).unwrap(), locked);
    assert!(!root.join("target/debug").exists());
    assert!(!root.join("target/release").exists());
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC"]
fn unit_graph_planning_keeps_unsupported_and_unstable_options_gated() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "Wafer.toml",
        "[package]\nname='controller'\nversion='1.0.0'\n",
    );
    write(root, "src/lib.siox", "planning only");
    for extra in ["--ignore-rust-version", "-Zbuild-std"] {
        let rejected = command(
            WAFER,
            root,
            &["build", "--unit-graph", "-Zunstable-options", extra],
        )
        .env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "nightly")
        .env("SIOXC", "must-not-run-sioxc")
        .output()
        .unwrap();
        assert!(!rejected.status.success());
        assert!(
            String::from_utf8_lossy(&rejected.stderr).contains("planning does not support"),
            "{}",
            String::from_utf8_lossy(&rejected.stderr)
        );
    }
    let stable = command(WAFER, root, &["build", "--unit-graph"])
        .env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "stable")
        .env("SIOXC", "must-not-run-sioxc")
        .output()
        .unwrap();
    assert!(!stable.status.success());
    assert!(String::from_utf8_lossy(&stable.stderr).contains("flag is unstable"));
    let cross = command(
        WAFER,
        root,
        &[
            "build",
            "--unit-graph",
            "-Zunstable-options",
            "--target",
            "unsupported-siox-cross",
        ],
    )
    .env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "nightly")
    .output()
    .unwrap();
    assert!(!cross.status.success());
    assert!(String::from_utf8_lossy(&cross.stderr).contains("cross-target"));
    assert!(!root.join("Wafer.lock").exists());
    assert!(!root.join("target").exists());
}

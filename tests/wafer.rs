//! Exercise the actual fork executables; no fake compiler or resolver.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn command(binary: &str, root: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(binary);
    command
        .current_dir(root)
        .env_clear()
        .env("HOME", root.join("home"))
        .env("CARGO_HOME", root.join("home/.cargo"))
        .env("RUSTC", "compiler-must-not-be-executed")
        .env("RUSTUP_HOME", root.join("missing-rustup"))
        .args(args);
    command
}

fn cli(binary: &str, root: &Path, args: &[&str]) -> Output {
    command(binary, root, args).output().unwrap()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

const WAFER: &str = env!("CARGO_BIN_EXE_wafer");
const CARGO: &str = env!("CARGO_BIN_EXE_cargo");

fn metadata(binary: &str, root: &Path) -> serde_json::Value {
    serde_json::from_str(&success(cli(
        binary,
        root,
        &["metadata", "--no-deps", "--format-version", "1"],
    )))
    .unwrap()
}

#[test]
fn wafer_bootstrap_brands_and_rejects_unadapted_operations_before_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    assert!(success(cli(WAFER, dir.path(), &["--version"])).starts_with("wafer "));
    assert!(success(cli(CARGO, dir.path(), &["--version"])).starts_with("cargo "));
    let help = success(cli(WAFER, dir.path(), &["--help"]));
    assert!(help.contains("Cargo-based Siox") && help.contains("metadata --no-deps"));
    assert!(!help.contains("Rust's package manager"));
    let list = success(cli(WAFER, dir.path(), &["--list"]));
    assert!(list.contains("locate-project") && !list.contains("publish"));
    for args in [
        vec!["build"],
        vec!["check"],
        vec!["test"],
        vec!["metadata"],
        vec!["fetch"],
        vec!["update"],
        vec!["tree"],
        vec!["clean"],
        vec!["new", "not-created"],
        vec!["init"],
        vec!["search", "siox"],
    ] {
        let output = cli(WAFER, dir.path(), &args);
        assert!(!output.status.success(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("adapter is not implemented"),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(!dir.path().join("target").exists());
    assert!(!dir.path().join("not-created").exists());
    assert!(!dir.path().join("Wafer.lock").exists());
}

#[test]
fn cargo_and_wafer_locate_their_own_manifests_without_fallback() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "Cargo.toml",
        "[package]\nname='rust_reference'\n",
    );
    write(
        dir.path(),
        "src/lib.rs",
        "not valid Rust; metadata must not compile",
    );
    write(dir.path(), "Wafer.toml", "[package]\nname='siox_project'\n");
    std::fs::create_dir_all(dir.path().join("nested/deep")).unwrap();
    for (binary, filename) in [(CARGO, "Cargo.toml"), (WAFER, "Wafer.toml")] {
        let output = success(cli(
            binary,
            &dir.path().join("nested/deep"),
            &["locate-project", "--message-format", "plain"],
        ));
        assert_eq!(PathBuf::from(output.trim()), dir.path().join(filename));
    }
    let wrong = cli(
        WAFER,
        dir.path(),
        &["locate-project", "--manifest-path", "Cargo.toml"],
    );
    let rust_metadata = success(cli(
        CARGO,
        dir.path(),
        &["metadata", "--no-deps", "--format-version", "1"],
    ));
    let rust_metadata: serde_json::Value = serde_json::from_str(&rust_metadata).unwrap();
    assert_eq!(
        rust_metadata["packages"][0]["targets"][0]["name"],
        "rust_reference"
    );
    assert!(!wrong.status.success());
    assert!(String::from_utf8_lossy(&wrong.stderr).contains("Wafer.toml"));
    std::fs::remove_file(dir.path().join("Wafer.toml")).unwrap();
    let missing = cli(WAFER, dir.path(), &["locate-project"]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("could not find `Wafer.toml`"));
}

#[test]
fn wafer_metadata_reuses_cargo_with_real_library_names_and_explicit_siox_paths() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "Wafer.toml",
        "[package]\nname='controller'\nversion='0.1.0'\n[lib]\nname='vendor'\npath='src/lib.siox'\n[[test]]\nname='smoke'\npath='tests/smoke.siox'\n",
    );
    write(
        dir.path(),
        "src/lib.siox",
        "not valid Siox; metadata must not analyze it",
    );
    write(dir.path(), "tests/smoke.siox", "also not valid Siox");
    let output = success(cli(
        WAFER,
        dir.path(),
        &["metadata", "--no-deps", "--format-version", "1"],
    ));
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(json["resolve"].is_null());
    assert_eq!(json["packages"][0]["name"], "controller");
    assert!(json["packages"][0].get("edition").is_none());
    assert!(json["packages"][0].get("rust_version").is_none());
    let targets = json["packages"][0]["targets"].as_array().unwrap();
    assert!(targets.iter().any(|target| target["name"] == "vendor"
        && target["src_path"] == dir.path().join("src/lib.siox").to_str().unwrap()));
    assert!(targets.iter().any(|target| target["name"] == "smoke"));
    assert!(
        targets
            .iter()
            .all(|target| target.get("edition").is_none() && target.get("crate_types").is_none())
    );
    assert!(!output.contains("vendor::lib"));
    assert!(!dir.path().join("target").exists());
    assert!(!dir.path().join("Wafer.lock").exists());
    write(
        dir.path(),
        "Wafer.toml",
        "[package]\nname='controller'\n[modules]\nvendor='src/lib.siox'\n",
    );
    let output = cli(
        WAFER,
        dir.path(),
        &["metadata", "--no-deps", "--format-version", "1"],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not support a [modules] table"));
}

#[test]
fn wafer_workspaces_and_inherited_package_metadata_use_wafer_manifests() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "Wafer.toml",
        "[workspace]\nmembers=['dies/*']\n[workspace.package]\nversion='1.2.3'\nlicense='MIT'\n",
    );
    write(
        dir.path(),
        "dies/controller/Wafer.toml",
        "[package]\nname='controller'\nversion.workspace=true\nlicense.workspace=true\nworkspace='../..'\n[lib]\nname='vendor'\npath='src/lib.siox'\n",
    );
    write(
        dir.path(),
        "dies/controller/src/lib.siox",
        "not valid Siox; workspace discovery is compiler independent",
    );
    let output = success(cli(
        WAFER,
        dir.path(),
        &["metadata", "--no-deps", "--format-version", "1"],
    ));
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(json["packages"].as_array().unwrap().len(), 1);
    assert_eq!(json["packages"][0]["version"], "1.2.3");
    assert_eq!(json["packages"][0]["license"], "MIT");
    let location = success(cli(
        WAFER,
        &dir.path().join("dies/controller"),
        &["locate-project", "--workspace", "--message-format", "plain"],
    ));
    assert_eq!(
        PathBuf::from(location.trim()),
        dir.path().join("Wafer.toml")
    );
}

#[test]
fn target_discovery_uses_each_languages_extension_and_preserves_cargo_layout() {
    let dir = tempfile::tempdir().unwrap();
    for manifest in ["Cargo.toml", "Wafer.toml"] {
        write(
            dir.path(),
            manifest,
            "[package]\nname='controller'\n[lib]\nname='vendor'\n",
        );
    }
    let sources = [
        ("lib", "vendor", "src/lib"),
        ("bin", "controller", "src/main"),
        ("bin", "tool", "src/bin/tool"),
        ("bin", "nested_tool", "src/bin/nested_tool/main"),
        ("example", "demo", "examples/demo"),
        ("example", "nested_demo", "examples/nested_demo/main"),
        ("test", "smoke", "tests/smoke"),
        ("test", "nested_smoke", "tests/nested_smoke/main"),
        ("bench", "perf", "benches/perf"),
        ("bench", "nested_perf", "benches/nested_perf/main"),
    ];
    for ext in ["rs", "siox"] {
        for (_, _, path) in sources {
            write(
                dir.path(),
                &format!("{path}.{ext}"),
                "metadata must not compile this",
            );
        }
        for path in [
            "tests/.hidden",
            "tests/support/helper",
            "tests/rust_only/main",
        ] {
            if path != "tests/rust_only/main" || ext == "rs" {
                write(
                    dir.path(),
                    &format!("{path}.{ext}"),
                    "not an inferred Siox test",
                );
            }
        }
    }
    write(dir.path(), "build.rs", "not a Siox build script");

    for (binary, ext) in [(CARGO, "rs"), (WAFER, "siox")] {
        let json = metadata(binary, dir.path());
        let targets = json["packages"][0]["targets"].as_array().unwrap();
        for (kind, name, path) in sources {
            assert!(
                targets.iter().any(|target| {
                    target["name"] == name
                        && target["kind"] == serde_json::json!([kind])
                        && target["src_path"]
                            == dir.path().join(format!("{path}.{ext}")).to_str().unwrap()
                }),
                "{binary}: {kind} {name}: {targets:?}"
            );
        }
        assert_eq!(
            targets.len(),
            sources.len() + if ext == "rs" { 2 } else { 0 }
        );
        if binary == WAFER {
            assert!(
                targets
                    .iter()
                    .all(|target| target["src_path"].as_str().unwrap().ends_with(".siox"))
            );
        }
    }
}

#[test]
fn wafer_discovery_merges_explicit_targets_and_honors_auto_discovery_switches() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "Wafer.toml",
        "[package]\nname='controller'\nbuild=false\n[lib]\nname='vendor'\n[[test]]\nname='named'\npath='checks/custom.siox'\n",
    );
    for path in [
        "src/lib.siox",
        "src/main.siox",
        "tests/smoke.siox",
        "checks/custom.siox",
        "examples/demo.siox",
        "benches/perf.siox",
    ] {
        write(dir.path(), path, "metadata only");
    }
    let json = metadata(WAFER, dir.path());
    let targets = json["packages"][0]["targets"].as_array().unwrap();
    assert_eq!(targets.len(), 6);
    assert!(targets.iter().any(|target| target["name"] == "smoke"));
    assert!(targets.iter().any(|target| target["name"] == "named"));

    write(
        dir.path(),
        "Wafer.toml",
        "[package]\nname='controller'\nautolib=false\nautobins=false\nautotests=false\nautoexamples=false\nautobenches=false\n[lib]\nname='vendor'\npath='src/lib.siox'\n[[test]]\nname='named'\npath='checks/custom.siox'\n",
    );
    let json = metadata(WAFER, dir.path());
    let targets = json["packages"][0]["targets"].as_array().unwrap();
    assert_eq!(targets.len(), 2);
    assert!(
        targets
            .iter()
            .all(|target| target["name"] == "vendor" || target["name"] == "named")
    );
}

#[test]
fn wafer_discovery_diagnostics_use_siox_and_never_take_rust_legacy_fallbacks() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "src/lib.rs", "not a Siox source");
    write(dir.path(), "src/vendor.rs", "not a Siox legacy library");
    for (manifest, message) in [
        ("[package]\nname='controller'\n", "src/lib.siox"),
        (
            "[package]\nname='controller'\n[lib]\nname='vendor'\n",
            "can't find library `vendor`",
        ),
        (
            "[package]\nname='controller'\n[[bin]]\nname='missing'\n",
            "src/bin/missing.siox",
        ),
    ] {
        write(dir.path(), "Wafer.toml", manifest);
        let output = cli(
            WAFER,
            dir.path(),
            &["metadata", "--no-deps", "--format-version", "1"],
        );
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(error.contains(message), "{error}");
        assert!(
            !error.contains("src/lib.rs") && !error.contains("missing.rs"),
            "{error}"
        );
    }
    write(dir.path(), "src/lib.siox", "metadata only");
    write(dir.path(), "test/smoke.siox", "wrong directory");
    write(
        dir.path(),
        "Wafer.toml",
        "[package]\nname='controller'\n[[test]]\nname='smoke'\n",
    );
    write(
        dir.path(),
        "Cargo.toml",
        "[package]\nname='controller'\n[[test]]\nname='smoke'\n",
    );
    write(dir.path(), "test/smoke.rs", "wrong directory");
    // Cargo metadata defers invalid test-target errors and keeps usable targets.
    for binary in [WAFER, CARGO] {
        let json = metadata(binary, dir.path());
        let targets = json["packages"][0]["targets"].as_array().unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0]["kind"], serde_json::json!(["lib"]));
    }

    write(dir.path(), "tests/smoke.siox", "first candidate");
    write(dir.path(), "tests/smoke/main.siox", "second candidate");
    write(dir.path(), "tests/smoke.rs", "first candidate");
    write(dir.path(), "tests/smoke/main.rs", "second candidate");
    for binary in [WAFER, CARGO] {
        let json = metadata(binary, dir.path());
        assert_eq!(json["packages"][0]["targets"].as_array().unwrap().len(), 1);
    }
}

#[test]
fn wafer_explicit_build_scripts_fail_until_their_adapter_exists() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "src/lib.siox", "metadata only");
    for setting in ["build=true", "build='build.siox'", "metabuild=['generate']"] {
        write(
            dir.path(),
            "Wafer.toml",
            &format!("[package]\nname='controller'\n{setting}\n"),
        );
        let output = cli(
            WAFER,
            dir.path(),
            &["metadata", "--no-deps", "--format-version", "1"],
        );
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("build-script adapter is not implemented"),
            "{setting}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn wafer_home_and_project_config_are_isolated_from_cargo() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "Wafer.toml", "[package]\nname='controller'\n");
    write(root, "Cargo.toml", "[package]\nname='controller'\n");
    write(root, "src/lib.siox", "metadata only");
    write(root, "src/lib.rs", "metadata only");
    write(root, ".cargo/config.toml", "[broken Cargo config");
    write(root, "home/.cargo/config.toml", "[broken Cargo home config");
    write(
        root,
        "home/.wafer/config.toml",
        "[build]\ntarget-dir='default-output'\n",
    );
    assert_eq!(
        metadata(WAFER, root)["target_directory"],
        root.join("home/default-output").to_str().unwrap()
    );
    let cargo = cli(
        CARGO,
        root,
        &["metadata", "--no-deps", "--format-version", "1"],
    );
    assert!(!cargo.status.success());
    assert!(String::from_utf8_lossy(&cargo.stderr).contains(".cargo"));

    write(
        root,
        "relative-store/config.toml",
        "[build]\ntarget-dir='override-output'\n",
    );
    for home in [PathBuf::from("relative-store"), root.join("relative-store")] {
        let output = command(
            WAFER,
            root,
            &["metadata", "--no-deps", "--format-version", "1"],
        )
        .env("WAFER_HOME", home)
        .output()
        .unwrap();
        let json: serde_json::Value = serde_json::from_str(&success(output)).unwrap();
        assert_eq!(
            json["target_directory"],
            root.join("override-output").to_str().unwrap()
        );
    }
    let output = command(
        WAFER,
        root,
        &["metadata", "--no-deps", "--format-version", "1"],
    )
    .env("WAFER_HOME", "")
    .output()
    .unwrap();
    let json: serde_json::Value = serde_json::from_str(&success(output)).unwrap();
    assert_eq!(
        json["target_directory"],
        root.join("home/default-output").to_str().unwrap()
    );

    write(root, ".wafer/config.toml", "[broken Wafer config");
    let output = cli(
        WAFER,
        root,
        &["metadata", "--no-deps", "--format-version", "1"],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Wafer configuration"));
    assert!(String::from_utf8_lossy(&output.stderr).contains(".wafer"));
}

#[test]
fn wafer_reuses_config_precedence_aliases_and_tool_specific_environment_keys() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for manifest in ["Wafer.toml", "Cargo.toml"] {
        write(root, manifest, "[package]\nname='controller'\n");
    }
    for path in ["src/lib.siox", "src/lib.rs"] {
        write(root, path, "metadata only");
    }
    write(
        root,
        "home/.wafer/config.toml",
        "[build]\ntarget-dir='global-output'\n",
    );
    write(
        root,
        ".wafer/config.toml",
        "[build]\ntarget-dir='wafer-output'\n[alias]\nprobe='metadata --no-deps --format-version 1'\n",
    );
    write(
        root,
        ".cargo/config.toml",
        "[build]\ntarget-dir='cargo-output'\n[alias]\nprobe='metadata --no-deps --format-version 1'\n",
    );
    for (binary, prefix, expected) in [
        (WAFER, "WAFER", "wafer-output"),
        (CARGO, "CARGO", "cargo-output"),
    ] {
        let json: serde_json::Value =
            serde_json::from_str(&success(cli(binary, root, &["probe"]))).unwrap();
        assert_eq!(
            json["target_directory"],
            root.join(expected).to_str().unwrap()
        );

        let mut cmd = command(binary, root, &["probe"]);
        cmd.env("WAFER_TARGET_DIR", "wafer-env-output")
            .env("CARGO_TARGET_DIR", "cargo-env-output");
        let json: serde_json::Value =
            serde_json::from_str(&success(cmd.output().unwrap())).unwrap();
        assert_eq!(
            json["target_directory"],
            root.join(if prefix == "WAFER" {
                "wafer-env-output"
            } else {
                "cargo-env-output"
            })
            .to_str()
            .unwrap()
        );

        let mut cmd = command(binary, root, &["probe"]);
        cmd.env("WAFER_BUILD_TARGET_DIR", "wafer-typed-output")
            .env("CARGO_BUILD_TARGET_DIR", "cargo-typed-output");
        let json: serde_json::Value =
            serde_json::from_str(&success(cmd.output().unwrap())).unwrap();
        assert_eq!(
            json["target_directory"],
            root.join(if prefix == "WAFER" {
                "wafer-typed-output"
            } else {
                "cargo-typed-output"
            })
            .to_str()
            .unwrap()
        );

        let mut cmd = command(
            binary,
            root,
            &["--config", "build.target-dir='cli-output'", "probe"],
        );
        cmd.env(format!("{prefix}_BUILD_TARGET_DIR"), "typed-output");
        let json: serde_json::Value =
            serde_json::from_str(&success(cmd.output().unwrap())).unwrap();
        assert_eq!(
            json["target_directory"],
            root.join("cli-output").to_str().unwrap()
        );

        let output = command(binary, root, &["probe"])
            .env(format!("{prefix}_TARGET_DIR"), "")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(&format!("{prefix}_TARGET_DIR")));
    }

    let output = command(WAFER, root, &["probe"])
        .env("CARGO_ALIAS_PROBE", "search should-not-run")
        .env("WAFER_ALIAS_PROBE", "locate-project --message-format plain")
        .output()
        .unwrap();
    assert_eq!(
        PathBuf::from(success(output).trim()),
        root.join("Wafer.toml")
    );
    let output = command(WAFER, root, &["probe"])
        .env("WAFER_ALIAS_PROBE", "build")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("adapter is not implemented"));
}

#[test]
fn cargo_fix_environment_cannot_bypass_the_wafer_compiler_guard() {
    let dir = tempfile::tempdir().unwrap();
    let output = command(WAFER, dir.path(), &["build"])
        .env("__CARGO_FIX_PLZ", "127.0.0.1:1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Wafer's `build` adapter is not implemented")
    );
    assert!(!dir.path().join("target").exists());
}

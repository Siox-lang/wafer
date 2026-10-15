use super::*;
use crate::core::compiler::{CompilerRef, Layout, UnitInterner, UserIntent};
use crate::core::{Shell, Workspace};
use crate::ops::{CompileFilter, CompileOptions, create_sioxc_bcx};

fn tool(name: &str) -> OsString {
    // Native acceptance must use the real compiler, never a stub executable.
    #[allow(clippy::disallowed_methods)]
    std::env::var_os(name).unwrap_or_else(|| panic!("set {name}"))
}

fn fixture(root: &std::path::Path, config: &str) -> GlobalContext {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join(".wafer")).unwrap();
    std::fs::write(
        root.join("Wafer.toml"),
        "[package]\nname='smoke'\nversion='1.2.3'\nauthors=['Example']\n[lib]\nname='vendor'\n",
    )
    .unwrap();
    std::fs::write(root.join("src/lib.siox"), "planning does not parse source").unwrap();
    std::fs::write(root.join(".wafer/config.toml"), config).unwrap();
    let mut context = GlobalContext::new_wafer(Shell::new(), root.into(), root.join("home"));
    context.set_search_stop_path(root);
    context.set_env(
        [
            (
                "SIOXC".into(),
                tool("WAFER_TEST_SIOXC").into_string().unwrap(),
            ),
            ("RUSTC".into(), "must-not-run-rustc".into()),
            ("CARGO".into(), "must-not-select-cargo".into()),
            (
                "WAFER".into(),
                std::env::current_exe()
                    .unwrap()
                    .into_os_string()
                    .into_string()
                    .unwrap(),
            ),
        ]
        .into(),
    );
    context
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_STD"]
fn shared_compilation_prepares_real_sioxc_object_test_and_native_run_processes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let context = fixture(
        root,
        "[build]\nsioxcflags=['--debug']\n[env]\nWAFER_NATIVE_ENV='selected'\n",
    );
    let ws = Workspace::new(&root.join("Wafer.toml"), &context).unwrap();
    let mut options = CompileOptions::new(&context, UserIntent::Build).unwrap();
    options.filter = CompileFilter::lib_only();
    let interner = UnitInterner::new();
    let bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
    let unit = &bcx.roots[0];
    let mut compilation = Compilation::new(&bcx).unwrap();
    assert!(compilation.sysroot_target_libdir.is_empty());
    assert!(compilation.rustc_process(unit, true, true).is_err());
    assert!(compilation.rustdoc_process(unit, None).is_err());
    let mut layout = Layout::new(&ws, None, "debug").unwrap();
    layout.prepare().unwrap();
    compilation
        .root_output
        .insert(CompileKind::Host, layout.dest().into());
    compilation
        .deps_output
        .insert(CompileKind::Host, layout.deps().into());
    let source = root.join("src/lib.siox");
    for test in [false, true] {
        std::fs::write(&source, if test {
            "module smoke; #[test] entity Smoke {} impl Smoke { process { assert!(1 + 1 == 2); } }\n"
        } else {
            "module smoke; entity Smoke { value: integer out } impl Smoke { value = 7; }\n"
        }).unwrap();
        let CompilerRef::Sioxc(compiler) = bcx.target_data.compiler() else {
            panic!("selected Rust for a Siox context")
        };
        let filename = compiler.output_filename(&source, test, &context).unwrap();
        let native = layout.deps().join(filename);
        let depfile = native.with_extension("d");
        let mut command = compilation.compiler_process(unit, true, true).unwrap();
        assert_eq!(command.get_program(), &tool("WAFER_TEST_SIOXC"));
        assert_eq!(command.get_cwd(), Some(root));
        assert_eq!(
            command.get_env("WAFER"),
            Some(std::env::current_exe().unwrap().into_os_string())
        );
        assert_eq!(
            command.get_env("WAFER_MANIFEST_PATH"),
            Some(root.join("Wafer.toml").into_os_string())
        );
        assert_eq!(command.get_env("WAFER_PKG_VERSION"), Some("1.2.3".into()));
        assert_eq!(command.get_env("WAFER_PKG_AUTHORS"), Some("Example".into()));
        assert_eq!(command.get_env("WAFER_CRATE_NAME"), Some("vendor".into()));
        assert_eq!(command.get_env("WAFER_NATIVE_ENV"), Some("selected".into()));
        assert!(
            !command
                .get_envs()
                .keys()
                .any(|key| key.starts_with("CARGO_") || key == "CARGO")
        );
        command
            .arg(&source)
            .args(&unit.rustflags)
            .arg("--std")
            .arg(tool("WAFER_TEST_STD"))
            .arg("--out")
            .arg(&native)
            .arg("--dep-info")
            .arg(&depfile);
        if test {
            command.arg("--test");
        }
        command.exec().unwrap();
        assert!(std::fs::metadata(&native).unwrap().len() > 0);
        assert!(depfile.is_file());
        if test {
            let vcd = layout.dest().join("smoke.vcd");
            let mut run = compilation
                .target_process(&native, unit.kind, &unit.pkg, None)
                .unwrap();
            assert_eq!(run.get_env("WAFER_PKG_NAME"), Some("smoke".into()));
            run.arg("-o").arg(&vcd).exec().unwrap();
            assert!(vcd.is_file());
        }
    }
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC"]
fn shared_compilation_rejects_rust_primary_overrides_and_ambiguous_native_runners() {
    let dir = tempfile::tempdir().unwrap();
    for config in [
        "",
        "[target.'cfg(all())']\nrunner='unused-first'\n[target.'cfg(not(any()))']\nrunner='unused-second'\n",
    ] {
        let context = fixture(dir.path(), config);
        let ws = Workspace::new(&dir.path().join("Wafer.toml"), &context).unwrap();
        let mut options = CompileOptions::new(&context, UserIntent::Build).unwrap();
        if config.is_empty() {
            options.build_config.primary_unit_rustc = Some(ProcessBuilder::new("must-not-run"));
        }
        let interner = UnitInterner::new();
        let bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
        let error = match Compilation::new(&bcx) {
            Ok(_) => panic!("unsupported configuration accepted"),
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains(if config.is_empty() {
                "primary-unit compiler overrides"
            } else {
                "several matching instances"
            }),
            "{error}"
        );
        assert!(!dir.path().join("target/debug").exists());
    }
}

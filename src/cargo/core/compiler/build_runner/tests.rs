use super::*;
use crate::core::Shell;
use crate::core::compiler::{UnitInterner, UserIntent};
use crate::ops::{CompileFilter, CompileOptions, create_sioxc_bcx};
use crate::util::GlobalContext;
use std::ffi::OsString;

fn tool(name: &str) -> OsString {
    // Native acceptance needs the real query-capable compiler, never a fake script.
    #[allow(clippy::disallowed_methods)]
    std::env::var_os(name).unwrap_or_else(|| panic!("set {name}"))
}

fn fixture(root: &Path, profile: &str, binary: bool) -> GlobalContext {
    std::fs::create_dir_all(root.join("src")).unwrap();
    let target = if binary {
        "[[bin]]\nname='vendor'\npath='src/lib.siox'\n"
    } else {
        "[lib]\nname='vendor'\n"
    };
    std::fs::write(
        root.join("Wafer.toml"),
        format!("[package]\nname='design'\nversion='1.0.0'\nautolib=false\n{target}{profile}"),
    )
    .unwrap();
    std::fs::write(root.join("src/lib.siox"), "dry-run must not compile source").unwrap();
    let mut context = GlobalContext::new_wafer(Shell::new(), root.into(), root.join("home"));
    context.set_search_stop_path(root);
    context.set_env(
        [
            (
                "SIOXC".into(),
                tool("WAFER_TEST_SIOXC").into_string().unwrap(),
            ),
            ("RUSTC".into(), "must-not-run-rustc".into()),
        ]
        .into(),
    );
    context
}

fn with_mode(interner: &UnitInterner, unit: &Unit, mode: CompileMode) -> Unit {
    interner.intern(
        &unit.pkg,
        &unit.target,
        unit.profile.clone(),
        unit.kind,
        mode,
        unit.features.clone(),
        unit.rustflags.clone(),
        unit.rustdocflags.clone(),
        unit.links_overrides.clone(),
        unit.is_std,
        unit.dep_hash,
        unit.artifact,
        unit.artifact_target_for_features,
        unit.skip_non_compile_time_dep,
    )
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_STD"]
fn shared_runner_prepares_objects_and_tests_without_a_second_execution_pipeline() {
    for test in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let context = fixture(root, "", !test);
        let ws = crate::core::Workspace::new(&root.join("Wafer.toml"), &context).unwrap();
        let mut options = CompileOptions::new(&context, UserIntent::Build).unwrap();
        options.filter = if test {
            CompileFilter::lib_only()
        } else {
            CompileFilter::single_bin("vendor".into())
        };
        options.build_config.jobs = 1;
        let interner = UnitInterner::new();
        let mut bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
        if test {
            // Exercise a real test-mode unit; the public test command remains gated.
            let unit = with_mode(&interner, &bcx.roots[0], CompileMode::Test);
            bcx.unit_graph = [(unit.clone(), vec![])].into();
            bcx.roots = vec![unit];
        }
        let unit = &bcx.roots[0];
        let mut runner = BuildRunner::new(&bcx).unwrap();
        assert!(!runner.only_requires_rmeta(unit, unit));
        assert!(!runner.rmeta_required(unit));
        assert!(runner.lto.is_empty());
        assert!(!root.join("target").exists());
        runner.prepare_units().unwrap();
        runner.prepare().unwrap();
        assert_eq!(runner.lto[unit], Lto::OnlyObject);
        assert!(runner.is_primary_package(unit));
        assert_eq!(
            runner.build_plan_inputs().unwrap(),
            [root.join("Wafer.toml")]
        );
        let outputs = runner.outputs(unit).unwrap();
        assert!(Arc::ptr_eq(&outputs, &runner.outputs(unit).unwrap()));
        assert_eq!(runner.get_executable(unit).unwrap().is_some(), test);
        // Release the first layout lock before the full common dry-run prepares it again.
        drop(runner);
        let compilation = BuildRunner::new(&bcx).unwrap().dry_run().unwrap();
        assert!(compilation.binaries.is_empty());
        assert!(compilation.cdylibs.is_empty());
        assert_eq!(compilation.objects.len(), usize::from(!test));
        assert_eq!(compilation.tests.len(), usize::from(test));
        assert_eq!(compilation.root_crate_names, ["vendor"]);
        let planned = if test {
            &compilation.tests[0]
        } else {
            &compilation.objects[0]
        };
        assert_eq!(planned.path, outputs[0].path);
        assert!(!planned.path.exists(), "dry-run compiled source");
        let source = root.join("src/lib.siox");
        std::fs::write(&source, if test {
            "module design; #[test] entity Design {} impl Design { process { assert!(1 + 1 == 2); } }\n"
        } else {
            "module design; entity Design { value: integer out } impl Design { value = 7; }\n"
        }).unwrap();
        let mut command = compilation.compiler_process(unit, true, true).unwrap();
        command
            .arg(&source)
            .arg("--std")
            .arg(tool("WAFER_TEST_STD"))
            .arg("--out")
            .arg(&planned.path);
        if test {
            command.arg("--test");
        }
        command.exec().unwrap();
        assert!(std::fs::metadata(&planned.path).unwrap().len() > 0);
        if test {
            let vcd = root.join("runner.vcd");
            compilation
                .target_process(&planned.path, unit.kind, &unit.pkg, None)
                .unwrap()
                .arg("-o")
                .arg(&vcd)
                .exec()
                .unwrap();
            assert!(vcd.is_file());
        }
    }
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC"]
fn shared_runner_rejects_unsupported_sioxc_artifacts_and_lto_before_build_layouts() {
    for (mode, profile) in [
        (CompileMode::Check { test: false }, ""),
        (CompileMode::Doc, ""),
        (CompileMode::Build, "[profile.release]\nlto=true\n"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let context = fixture(root, profile, false);
        let ws = crate::core::Workspace::new(&root.join("Wafer.toml"), &context).unwrap();
        let mut options = CompileOptions::new(&context, UserIntent::Build).unwrap();
        options.build_config.jobs = 1;
        options.filter = CompileFilter::lib_only();
        if !profile.is_empty() {
            options.build_config.requested_profile = "release".into();
        }
        let interner = UnitInterner::new();
        let mut bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
        let unit = with_mode(&interner, &bcx.roots[0], mode);
        bcx.unit_graph = [(unit.clone(), vec![])].into();
        bcx.roots = vec![unit];
        let result = BuildRunner::new(&bcx).and_then(|runner| runner.dry_run());
        let error = match result {
            Ok(_) => panic!("unsupported compiler request accepted"),
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains(if profile.is_empty() {
                "only supports native object and test outputs"
            } else {
                "LTO/bitcode"
            }),
            "{error}"
        );
        assert!(
            !root.join("target").exists(),
            "unsupported request created a build layout"
        );
    }
}

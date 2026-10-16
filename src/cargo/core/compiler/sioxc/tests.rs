use super::*;
use crate::core::compiler::{DefaultExecutor, UnitInterner, UserIntent};
use crate::core::{Shell, Target};
use crate::ops::{CompileFilter, CompileOptions, create_sioxc_bcx};
use crate::util::GlobalContext;
use std::{
    ffi::OsString,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[test]
fn flags_forward_only_supported_semantic_options_and_preserve_value_boundaries() {
    for flags in [
        vec![
            "--std",
            "a directory",
            "--top=First",
            "-Dwarnings",
            "-A",
            "unused_signal",
            "--verbose",
        ],
        vec![
            "--std=-leading-dash",
            "--allow=warnings",
            "-g",
            "-W=unused_signal",
            "--forbid",
            "warnings",
        ],
        vec![],
    ] {
        validate_flags(
            &flags.into_iter().map(String::from).collect::<Vec<_>>(),
            CompileMode::Build,
        )
        .unwrap();
    }
    for flags in [
        vec!["--out=outside"],
        vec!["-ooutside"],
        vec!["--dep-info", "outside"],
        vec!["--emit=metadata"],
        vec!["--print=file-names"],
        vec!["--test"],
        vec!["-V"],
        vec!["--help"],
        vec!["--"],
        vec!["other.siox"],
        vec!["-Copt-level=3"],
        vec!["--unknown"],
        vec!["--std"],
        vec!["--std="],
        vec!["--std", "--out"],
        vec!["-D="],
        vec!["--allow", ""],
        vec!["--std=bad\0path"],
    ] {
        let flags = flags.into_iter().map(String::from).collect::<Vec<_>>();
        assert!(
            validate_flags(&flags, CompileMode::Build).is_err(),
            "{flags:?}"
        );
    }
    assert!(validate_flags(&["--top=First".into()], CompileMode::Test).is_err());
}

fn tool(name: &str) -> String {
    #[allow(clippy::disallowed_methods)]
    std::env::var(name).unwrap_or_else(|_| panic!("set {name}"))
}

fn fixture(root: &Path, extra: &[String]) -> GlobalContext {
    fs::create_dir_all(root.join("rtl")).unwrap();
    fs::write(root.join("Wafer.toml"), "[package]\nname='device'\nversion='1.0.0'\nautolib=false\n[[bin]]\nname='chip'\npath='rtl/chip.siox'\n").unwrap();
    fs::write(root.join("rtl/chip.siox"), "module chip; entity First { value: integer out } impl First { value = 7; } entity Second { value: integer out } impl Second { value = 9; }\n").unwrap();
    let mut context = GlobalContext::new_wafer(Shell::new(), root.into(), root.join("home"));
    context.set_search_stop_path(root);
    let mut flags = vec![
        "--std".to_owned(),
        tool("WAFER_TEST_STD"),
        "--top=First".into(),
        "-Awarnings".into(),
    ];
    flags.extend_from_slice(extra);
    context.set_env(
        [
            ("SIOXC".into(), tool("WAFER_TEST_SIOXC")),
            ("WAFER_ENCODED_SIOXCFLAGS".into(), flags.join("\u{1f}")),
            ("RUSTC".into(), "must-not-run-rustc".into()),
        ]
        .into(),
    );
    context
}

#[derive(Default)]
struct NativeExecutor {
    invocations: Mutex<Vec<Vec<OsString>>>,
    force: AtomicBool,
    replace_with_directory: AtomicBool,
}

impl Executor for NativeExecutor {
    fn exec(
        &self,
        command: &cargo_util::ProcessBuilder,
        id: crate::core::PackageId,
        target: &Target,
        mode: CompileMode,
        stdout: &mut dyn FnMut(&str) -> CargoResult<()>,
        stderr: &mut dyn FnMut(&str) -> CargoResult<()>,
    ) -> CargoResult<()> {
        assert_eq!(command.get_env("WAFER_CRATE_NAME").unwrap(), "chip");
        self.invocations
            .lock()
            .unwrap()
            .push(command.get_args().cloned().collect());
        DefaultExecutor.exec(command, id, target, mode, stdout, stderr)?;
        if self.replace_with_directory.load(Ordering::SeqCst) {
            // Fault injection after real codegen: process success alone is not an artifact.
            let mut args = command.get_args();
            let output = args
                .find(|arg| *arg == "--out")
                .and_then(|_| args.next())
                .unwrap();
            fs::remove_file(output)?;
            fs::create_dir(output)?;
        }
        Ok(())
    }
    fn force_rebuild(&self, _unit: &Unit) -> bool {
        self.force.load(Ordering::SeqCst)
    }
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_STD"]
fn shared_compile_entry_uses_real_target_flags_sidecars_and_uplift_on_dirty_and_fresh_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let context = fixture(root, &[]);
    let ws = crate::core::Workspace::new(&root.join("Wafer.toml"), &context).unwrap();
    let mut options = CompileOptions::new(&context, UserIntent::Build).unwrap();
    options.build_config.jobs = 1;
    options.build_config.export_dir = Some(root.join("export"));
    options.filter = CompileFilter::single_bin("chip".into());
    let interner = UnitInterner::new();
    let bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
    let recorded = Arc::new(NativeExecutor::default());
    let executor: Arc<dyn Executor> = recorded.clone();
    let built = BuildRunner::new(&bcx).unwrap().compile(&executor).unwrap();
    assert!(built.binaries.is_empty() && built.tests.is_empty());
    assert_eq!(built.objects.len(), 1);
    let object = &built.objects[0].path;
    let uplift = root.join("target/debug/chip.o");
    let export = root.join("export/chip.o");
    assert!(fs::metadata(object).unwrap().len() > 0);
    assert_eq!(fs::read(object).unwrap(), fs::read(&uplift).unwrap());
    assert_eq!(fs::read(object).unwrap(), fs::read(&export).unwrap());
    let invocations = recorded.invocations.lock().unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0][0], root.join("rtl/chip.siox"));
    for flag in [
        "--debug",
        "--top=First",
        "-Awarnings",
        "--emit=object",
        "--dep-info",
    ] {
        assert!(
            invocations[0].iter().any(|arg| arg == flag),
            "missing {flag}"
        );
    }
    assert!(
        invocations[0]
            .windows(2)
            .any(|args| args == [OsString::from("--std"), tool("WAFER_TEST_STD").into()])
    );
    drop(invocations);
    let raw = object.with_extension("d");
    assert!(raw.is_file());
    let mtime = paths::mtime(object).unwrap();
    fs::remove_file(&uplift).unwrap();
    fs::remove_file(&export).unwrap();
    BuildRunner::new(&bcx).unwrap().compile(&executor).unwrap();
    assert_eq!(
        recorded.invocations.lock().unwrap().len(),
        1,
        "fresh job recompiled"
    );
    assert_eq!(paths::mtime(object).unwrap(), mtime);
    assert!(
        uplift.is_file() && export.is_file(),
        "fresh job did not restore artifacts"
    );
    recorded.force.store(true, Ordering::SeqCst);
    BuildRunner::new(&bcx).unwrap().compile(&executor).unwrap();
    assert_eq!(recorded.invocations.lock().unwrap().len(), 2);
    recorded
        .replace_with_directory
        .store(true, Ordering::SeqCst);
    assert!(BuildRunner::new(&bcx).unwrap().compile(&executor).is_err());
    assert_eq!(recorded.invocations.lock().unwrap().len(), 3);
    assert!(object.is_dir());
    let mut probe = BuildRunner::new(&bcx).unwrap();
    probe.prepare_units().unwrap();
    probe.prepare().unwrap();
    assert!(
        fs::read(probe.files().fingerprint_file_path(&bcx.roots[0], ""))
            .unwrap()
            .is_empty()
    );
    drop(probe);
    drop(bcx);
    options.build_config.requested_profile = "release".into();
    let bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
    let error = BuildRunner::new(&bcx)
        .unwrap()
        .compile(&executor)
        .err()
        .unwrap();
    assert!(error.to_string().contains("opt-level=0"));
    assert_eq!(
        recorded.invocations.lock().unwrap().len(),
        3,
        "unsupported profile invoked compiler"
    );
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_STD"]
fn owned_output_flags_are_rejected_before_creating_native_build_layouts() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let outside = root.join("do-not-overwrite");
    fs::write(&outside, "sentinel").unwrap();
    let context = fixture(root, &["--out".into(), outside.to_string_lossy().into()]);
    let ws = crate::core::Workspace::new(&root.join("Wafer.toml"), &context).unwrap();
    let mut options = CompileOptions::new(&context, UserIntent::Build).unwrap();
    options.build_config.jobs = 1;
    let interner = UnitInterner::new();
    let bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
    let error = match BuildRunner::new(&bcx) {
        Ok(_) => panic!("accepted output override"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("owned by Wafer"));
    assert!(!root.join("target").exists());
    assert_eq!(fs::read_to_string(outside).unwrap(), "sentinel");
}

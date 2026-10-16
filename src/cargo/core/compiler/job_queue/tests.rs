//! Actual native compiler work uses Cargo's one queue, tokens and fingerprint jobs.

use super::*;
use crate::core::compiler::{SioxcTargetData, TimingOutput, UnitInterner, UserIntent, fingerprint};
use crate::ops::{CompileFilter, CompileOptions, create_sioxc_bcx};
use std::ffi::OsString;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

fn tool(name: &str) -> OsString {
    #[allow(clippy::disallowed_methods)]
    std::env::var_os(name).unwrap_or_else(|| panic!("set {name}"))
}

fn fixture(root: &Path) -> GlobalContext {
    for package in ["", "left/", "right/"] {
        let directory = root.join(package);
        std::fs::create_dir_all(directory.join("src")).unwrap();
        let name = if package.is_empty() {
            "controller"
        } else {
            package.trim_end_matches('/')
        };
        let dependencies = if package.is_empty() {
            "[dependencies]\nleft={path='left'}\nright={path='right'}\n"
        } else {
            ""
        };
        std::fs::write(
            directory.join("Wafer.toml"),
            format!("[package]\nname='{name}'\nversion='1.0.0'\n{dependencies}"),
        )
        .unwrap();
        std::fs::write(
            directory.join("src/lib.siox"),
            "module design; entity Design { value: integer out } impl Design { value = 7; }\n",
        )
        .unwrap();
    }
    let mut context = GlobalContext::new_wafer(Shell::new(), root.into(), root.join("home"));
    context.set_search_stop_path(root);
    context.set_env(
        [
            (
                "SIOXC".into(),
                tool("WAFER_TEST_SIOXC").into_string().unwrap(),
            ),
            ("RUSTC".into(), "must-not-run-rustc".into()),
            (
                "WAFER_ENCODED_SIOXCFLAGS".into(),
                format!("--std\u{1f}{}", tool("WAFER_TEST_STD").to_str().unwrap()),
            ),
        ]
        .into(),
    );
    context
}

fn test_root(bcx: &mut BuildContext<'_, '_, SioxcTargetData>, interner: &UnitInterner) {
    let unit = bcx.roots[0].clone();
    let test = interner.intern(
        &unit.pkg,
        &unit.target,
        unit.profile.clone(),
        unit.kind,
        CompileMode::Test,
        unit.features.clone(),
        unit.rustflags.clone(),
        unit.rustdocflags.clone(),
        unit.links_overrides.clone(),
        unit.is_std,
        unit.dep_hash,
        unit.artifact,
        unit.artifact_target_for_features,
        unit.skip_non_compile_time_dep,
    );
    let dependencies = bcx.unit_graph.remove(&unit).unwrap();
    bcx.unit_graph.insert(test.clone(), dependencies);
    bcx.roots = vec![test];
}

#[derive(Default)]
struct Activity {
    active: AtomicUsize,
    peak: AtomicUsize,
    started: Mutex<Vec<String>>,
    peers: (Mutex<usize>, Condvar),
}

fn run(
    bcx: &BuildContext<'_, '_, SioxcTargetData>,
    jobs: usize,
    force: bool,
    synchronize: bool,
) -> (CargoResult<()>, Arc<Activity>, HashMap<String, PathBuf>) {
    let mut runner = BuildRunner::new(bcx).unwrap();
    // Control the tested limit independently of the outer Cargo test jobserver.
    runner.jobserver = jobserver::Client::new(jobs).unwrap();
    runner.jobserver.acquire_raw().unwrap();
    runner.prepare_units().unwrap();
    runner.prepare().unwrap();
    assert!(
        bcx.target_data
            .compiler()
            .rust_workspace_wrapper()
            .is_none()
    );
    let activity = Arc::new(Activity::default());
    let mut queue = JobQueue::new(bcx);
    let mut outputs = HashMap::new();
    for unit in bcx.unit_graph.keys() {
        fingerprint::prepare_init(&mut runner, unit).unwrap();
        let mut job = fingerprint::prepare_target(&mut runner, unit, force).unwrap();
        let output = runner.outputs(unit).unwrap()[0].path.clone();
        outputs.insert(unit.pkg.name().to_string(), output.clone());
        if job.freshness().is_dirty() {
            let dependencies = runner
                .unit_deps(unit)
                .iter()
                .map(|dep| dep.unit.clone())
                .collect::<Vec<_>>();
            let prerequisites = dependencies
                .iter()
                .map(|dep| runner.outputs(dep).unwrap()[0].path.clone())
                .collect::<Vec<_>>();
            let executor: Arc<dyn super::super::Executor> = Arc::new(super::super::DefaultExecutor);
            let work = super::super::sioxc::prepare(&mut runner, unit, &executor)
                .unwrap()
                .then(super::super::link_targets(&mut runner, unit, false).unwrap());
            let activity = Arc::clone(&activity);
            let name = unit.pkg.name().to_string();
            job.before(Work::new(move |state| {
                // These are independent native emissions, not compiled-library imports/linking.
                anyhow::ensure!(
                    prerequisites.iter().all(|path| path.is_file()),
                    "dependent started before prerequisites finished"
                );
                activity.started.lock().unwrap().push(name);
                let active = activity.active.fetch_add(1, Ordering::SeqCst) + 1;
                activity.peak.fetch_max(active, Ordering::SeqCst);
                if synchronize && prerequisites.is_empty() {
                    let (count, ready) = &activity.peers;
                    let mut peers = count.lock().unwrap();
                    *peers += 1;
                    ready.notify_all();
                    let (_peers, timeout) = ready
                        .wait_timeout_while(peers, Duration::from_secs(5), |count| *count < 2)
                        .unwrap();
                    anyhow::ensure!(
                        !timeout.timed_out(),
                        "second native job never received a token"
                    );
                }
                let result = work.call(state);
                activity.active.fetch_sub(1, Ordering::SeqCst);
                result
            }));
        } else {
            // Fresh jobs emit directly on the main thread, without bounded-queue deadlock.
            job.before(Work::new(|state| {
                state.stderr("cached native output".into())
            }));
        }
        queue.enqueue(&runner, unit, job).unwrap();
    }
    for fingerprint in runner.fingerprints.values() {
        fingerprint.clear_memoized();
    }
    let result = queue.execute(&mut runner, &mut BuildPlan::new());
    (result, activity, outputs)
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_STD"]
fn actual_native_jobs_share_ordering_tokens_freshness_and_timing_reports() {
    for jobs in [1, 2] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let context = fixture(root);
        let ws = crate::core::Workspace::new(&root.join("Wafer.toml"), &context).unwrap();
        let mut options = CompileOptions::new(&context, UserIntent::Build).unwrap();
        options.filter = CompileFilter::lib_only();
        options.build_config.jobs = jobs as u32;
        options.build_config.timing_outputs = vec![TimingOutput::Html];
        let interner = UnitInterner::new();
        let mut bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
        test_root(&mut bcx, &interner);
        std::fs::write(root.join("src/lib.siox"),
            "module design; #[test] entity Test {} impl Test { process { assert!(1 + 1 == 2); } }\n").unwrap();
        let (result, activity, outputs) = run(&bcx, jobs, false, jobs == 2);
        result.unwrap();
        assert_eq!(activity.started.lock().unwrap().len(), 3);
        assert_eq!(activity.peak.load(Ordering::SeqCst), jobs);
        assert_eq!(activity.active.load(Ordering::SeqCst), 0);
        assert_eq!(
            activity.started.lock().unwrap().last().unwrap(),
            "controller"
        );
        let vcd = root.join("queue.vcd");
        let executor: Arc<dyn super::super::Executor> = Arc::new(super::super::DefaultExecutor);
        BuildRunner::new(&bcx)
            .unwrap()
            .compile(&executor)
            .unwrap()
            .target_process(
                &outputs["controller"],
                bcx.roots[0].kind,
                &bcx.roots[0].pkg,
                None,
            )
            .unwrap()
            .arg("-o")
            .arg(&vcd)
            .exec()
            .unwrap();
        assert!(vcd.is_file());
        let report =
            std::fs::read_to_string(root.join("target/wafer-timings/wafer-timing.html")).unwrap();
        assert!(report.contains("Wafer Build Timings"));
        let CompilerRef::Sioxc(compiler) = bcx.target_data.compiler() else {
            panic!("selected Rust")
        };
        assert!(
            report.contains("<td>sioxc:</td>")
                && report.contains(compiler.verbose_version.lines().next().unwrap())
        );
        assert!(!report.contains("<td>rustc:</td>") && !report.contains("must-not-run-rustc"));
        let mtimes = outputs
            .values()
            .map(|path| cargo_util::paths::mtime(path).unwrap())
            .collect::<Vec<_>>();
        let (result, fresh, _) = run(&bcx, jobs, false, false);
        result.unwrap();
        assert!(fresh.started.lock().unwrap().is_empty());
        assert_eq!(
            outputs
                .values()
                .map(|path| cargo_util::paths::mtime(path).unwrap())
                .collect::<Vec<_>>(),
            mtimes
        );
        std::fs::remove_file(&outputs["left"]).unwrap();
        let (result, rebuilt, _) = run(&bcx, jobs, false, false);
        result.unwrap();
        assert_eq!(*rebuilt.started.lock().unwrap(), ["left", "controller"]);
    }
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_STD"]
fn native_failure_blocks_dependents_and_rust_only_diagnostic_servers_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let context = fixture(root);
    let ws = crate::core::Workspace::new(&root.join("Wafer.toml"), &context).unwrap();
    let mut options = CompileOptions::new(&context, UserIntent::Build).unwrap();
    options.filter = CompileFilter::lib_only();
    options.build_config.jobs = 2;
    options.build_config.keep_going = true;
    let interner = UnitInterner::new();
    let bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
    {
        let mut runner = BuildRunner::new(&bcx).unwrap();
        *bcx.build_config.rustfix_diagnostic_server.borrow_mut() =
            Some(diagnostic_server::RustfixDiagnosticServer::new().unwrap());
        let error = JobQueue::new(&bcx)
            .execute(&mut runner, &mut BuildPlan::new())
            .unwrap_err();
        assert!(error.to_string().contains("Rust fix diagnostic servers"));
        assert!(
            bcx.build_config
                .rustfix_diagnostic_server
                .borrow_mut()
                .take()
                .is_some()
        );
    }
    run(&bcx, 2, false, false).0.unwrap();
    std::fs::write(root.join("left/src/lib.siox"), "invalid native source!").unwrap();
    let (result, activity, outputs) = run(&bcx, 2, true, false);
    assert!(result.is_err());
    let started = activity.started.lock().unwrap();
    assert!(started.iter().any(|name| name == "left"));
    assert!(
        started.iter().any(|name| name == "right"),
        "keep-going skipped independent work"
    );
    assert!(!started.iter().any(|name| name == "controller"));
    assert_eq!(activity.active.load(Ordering::SeqCst), 0);
    assert!(outputs["right"].is_file());
    let mut runner = BuildRunner::new(&bcx).unwrap();
    runner.prepare_units().unwrap();
    runner.prepare().unwrap();
    for unit in bcx
        .unit_graph
        .keys()
        .filter(|unit| unit.pkg.name() != "right")
    {
        assert!(
            std::fs::read(runner.files().fingerprint_file_path(unit, ""))
                .unwrap()
                .is_empty(),
            "failed or blocked native unit retained a valid fingerprint"
        );
    }
}

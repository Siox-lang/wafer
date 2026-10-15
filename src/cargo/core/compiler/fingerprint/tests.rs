//! Native outputs exercise Cargo's actual persisted fingerprint and input reader.

use super::*;
use crate::core::Shell;
use crate::core::compiler::{UnitInterner, UserIntent};
use crate::ops::{CompileFilter, CompileOptions, create_sioxc_bcx};
use std::ffi::OsString;

fn tool(name: &str) -> OsString {
    #[allow(clippy::disallowed_methods)]
    std::env::var_os(name).unwrap_or_else(|| panic!("set {name}"))
}

fn inspect<T: UnitTargetData>(bcx: &BuildContext<'_, '_, T>, unit: &Unit) -> Option<DirtyReason> {
    let mut runner = BuildRunner::new(bcx).unwrap();
    runner.prepare_units().unwrap();
    runner.prepare().unwrap();
    prepare_init(&mut runner, unit).unwrap();
    let fingerprint = calculate(&mut runner, unit).unwrap();
    compare_old_fingerprint(
        unit,
        &runner.files().fingerprint_file_path(unit, ""),
        &fingerprint,
        false,
        false,
    )
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_STD"]
fn native_outputs_and_sidecars_use_cargos_shared_persisted_fingerprints() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Wafer.toml"),
        "[package]\nname='design'\nversion='1.0.0'\n",
    )
    .unwrap();
    let source = root.join("src/lib.siox");
    std::fs::write(
        &source,
        "module design; entity Design { value: integer out } impl Design { value = 7; }\n",
    )
    .unwrap();
    let mut context = GlobalContext::new_wafer(Shell::new(), root.into(), root.join("home"));
    context.set_search_stop_path(root);
    context.set_env(
        [(
            "SIOXC".into(),
            tool("WAFER_TEST_SIOXC").into_string().unwrap(),
        )]
        .into(),
    );
    let ws = crate::core::Workspace::new(&root.join("Wafer.toml"), &context).unwrap();
    let mut options = CompileOptions::new(&context, UserIntent::Build).unwrap();
    options.build_config.jobs = 1;
    options.filter = CompileFilter::lib_only();
    let interner = UnitInterner::new();
    let bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
    let unit = &bcx.roots[0];
    assert!(inspect(&bcx, unit).is_some());
    let mut runner = BuildRunner::new(&bcx).unwrap();
    runner.prepare_units().unwrap();
    runner.prepare().unwrap();
    prepare_init(&mut runner, unit).unwrap();
    assert!(
        prepare_target(&mut runner, unit, false)
            .unwrap()
            .freshness()
            .is_dirty()
    );
    assert!(!runner.only_requires_rmeta(unit, unit));
    let output = runner.outputs(unit).unwrap()[0].path.clone();
    let raw_dep = output.with_extension("d");
    let encoded_dep = dep_info_loc(&mut runner, unit);
    let mut command = runner
        .compilation
        .compiler_process(unit, true, true)
        .unwrap();
    command
        .arg(&source)
        .arg("--std")
        .arg(tool("WAFER_TEST_STD"))
        .arg("--out")
        .arg(&output)
        .arg("--dep-info")
        .arg(&raw_dep);
    command.exec().unwrap();
    translate_dep_info(
        &raw_dep,
        &encoded_dep,
        root,
        root,
        &build_root(&runner),
        &command,
        true,
        context.env_config().unwrap(),
        DepInfoFormat::Sioxc,
    )
    .unwrap();
    // Control timestamps without sleeping or changing any shared std source.
    let anchor = FileTime::from_unix_time(1_700_000_000, 0);
    filetime::set_file_mtime(&source, anchor).unwrap();
    let inputs = parse_dep_info(root, &build_root(&runner), &encoded_dep)
        .unwrap()
        .unwrap();
    assert!(inputs.files.contains_key(&source));
    let latest_input = inputs
        .files
        .keys()
        .map(|path| paths::mtime(path).unwrap())
        .max()
        .unwrap();
    filetime::set_file_mtime(
        &encoded_dep,
        FileTime::from_unix_time(latest_input.unix_seconds() + 1, 0),
    )
    .unwrap();
    let fingerprint = calculate_normal(&mut runner, unit).unwrap();
    let location = runner.files().fingerprint_file_path(unit, "");
    write_fingerprint(&location, &fingerprint).unwrap();
    drop(runner);
    assert!(inspect(&bcx, unit).is_none());
    {
        let mut runner = BuildRunner::new(&bcx).unwrap();
        runner.prepare_units().unwrap();
        runner.prepare().unwrap();
        assert!(
            prepare_target(&mut runner, unit, false)
                .unwrap()
                .freshness()
                .is_fresh()
        );
    }
    let persisted = std::fs::read(&location).unwrap();
    filetime::set_file_mtime(
        &source,
        FileTime::from_unix_time(latest_input.unix_seconds() + 2, 0),
    )
    .unwrap();
    assert!(inspect(&bcx, unit).is_some());
    {
        let mut runner = BuildRunner::new(&bcx).unwrap();
        runner.prepare_units().unwrap();
        runner.prepare().unwrap();
        assert!(
            prepare_target(&mut runner, unit, false)
                .unwrap()
                .freshness()
                .is_dirty()
        );
        assert!(std::fs::read(&location).unwrap().is_empty());
    }
    filetime::set_file_mtime(&source, anchor).unwrap();
    assert!(
        inspect(&bcx, unit).is_some(),
        "an interrupted dirty job retained a valid fingerprint"
    );
    std::fs::write(&location, persisted).unwrap();
    assert!(inspect(&bcx, unit).is_none());
    let encoded = std::fs::read(&encoded_dep).unwrap();
    std::fs::write(&encoded_dep, "corrupt input record").unwrap();
    assert!(inspect(&bcx, unit).is_some());
    std::fs::write(&encoded_dep, &encoded).unwrap();
    filetime::set_file_mtime(
        &encoded_dep,
        FileTime::from_unix_time(latest_input.unix_seconds() + 1, 0),
    )
    .unwrap();
    assert!(inspect(&bcx, unit).is_none());
    std::fs::remove_file(&encoded_dep).unwrap();
    assert!(inspect(&bcx, unit).is_some());
    std::fs::write(&encoded_dep, encoded).unwrap();
    filetime::set_file_mtime(
        &encoded_dep,
        FileTime::from_unix_time(latest_input.unix_seconds() + 1, 0),
    )
    .unwrap();
    assert!(inspect(&bcx, unit).is_none());
    std::fs::remove_file(&output).unwrap();
    assert!(inspect(&bcx, unit).is_some());
    drop(bcx);
    drop(ws);
    context.nightly_features_allowed = true;
    context
        .configure(
            0,
            false,
            None,
            false,
            false,
            false,
            &None,
            &["checksum-freshness".into()],
            &[],
        )
        .unwrap();
    let ws = crate::core::Workspace::new(&root.join("Wafer.toml"), &context).unwrap();
    let mut options = CompileOptions::new(&context, UserIntent::Build).unwrap();
    options.build_config.jobs = 1;
    let bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
    let mut runner = BuildRunner::new(&bcx).unwrap();
    runner.prepare_units().unwrap();
    runner.prepare().unwrap();
    let error = prepare_target(&mut runner, &bcx.roots[0], false).unwrap_err();
    assert!(error.to_string().contains("checksum-freshness"));
}

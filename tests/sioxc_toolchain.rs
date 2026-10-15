use cargo::{
    core::Shell,
    util::{GlobalContext, Sioxc},
};

#[test]
#[ignore = "requires WAFER_TEST_SIOXC pointing to the actual query-capable Sioxc"]
fn query_actual_sioxc_without_compiling_or_creating_artifacts() {
    // Select an actual compiler explicitly; never replace it with a fake script.
    #[allow(clippy::disallowed_methods)]
    let compiler = std::env::var_os("WAFER_TEST_SIOXC").expect("set WAFER_TEST_SIOXC");
    let root = tempfile::tempdir().unwrap();
    let mut context =
        GlobalContext::new(Shell::new(), root.path().into(), root.path().join("home"));
    context.set_search_stop_path(root.path());
    context.set_env(Default::default());
    let compiler = Sioxc::new(compiler.into(), &context).unwrap();
    assert!(compiler.verbose_version.starts_with("sioxc "));
    assert!(!compiler.host.is_empty() && !compiler.native_target.is_empty());
    assert!(!compiler.host_cfg.is_empty());
    let input = root.path().join("missing.siox");
    assert_eq!(
        compiler.output_filename(&input, false, &context).unwrap(),
        "missing.o"
    );
    assert_eq!(
        compiler.output_filename(&input, true, &context).unwrap(),
        "missing.test"
    );
    assert!(!input.exists());
    assert!(!root.path().join("missing.o").exists());
    assert!(!root.path().join("missing.test").exists());
    assert!(Sioxc::new(env!("CARGO_BIN_EXE_cargo").into(), &context).is_err());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires WAFER_TEST_SIOXC pointing to the actual query-capable Sioxc"]
fn native_identity_tracks_executable_bytes_not_path_size_mtime_or_release() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    #[allow(clippy::disallowed_methods)]
    let actual = std::env::var_os("WAFER_TEST_SIOXC").expect("set WAFER_TEST_SIOXC");
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let first = root.join("selected/sioxc");
    std::fs::create_dir_all(first.parent().unwrap()).unwrap();
    std::fs::copy(actual, &first).unwrap();
    // ELF permits trailing bytes; both tools still execute the real compiler queries.
    std::fs::OpenOptions::new()
        .append(true)
        .open(&first)
        .unwrap()
        .write_all(b"wafer-identity-a")
        .unwrap();
    let second = root.join("other-sioxc");
    std::fs::copy(&first, &second).unwrap();
    std::fs::create_dir_all(root.join("blocked")).unwrap();
    std::fs::write(root.join("blocked/sioxc"), "not executable").unwrap();
    std::fs::set_permissions(
        root.join("blocked/sioxc"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    std::fs::create_dir_all(root.join(".wafer")).unwrap();
    std::fs::write(
        root.join(".wafer/config.toml"),
        "[env]\nPATH={value='blocked:selected',force=true}\n",
    )
    .unwrap();
    let mut context = GlobalContext::new_wafer(Shell::new(), root.into(), root.join("home"));
    context.set_search_stop_path(root);
    context.set_env([("SIOXC".into(), "sioxc".into())].into());
    let selected = context.load_sioxc().unwrap();
    assert_eq!(selected.path, first);
    let identical = Sioxc::new(second.clone(), &context).unwrap();
    assert_eq!(selected.fingerprint(), identical.fingerprint());
    let metadata = std::fs::metadata(&second).unwrap();
    {
        use std::io::{Seek, SeekFrom};
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&second)
            .unwrap();
        file.seek(SeekFrom::End(-1)).unwrap();
        file.write_all(b"b").unwrap();
    }
    filetime::set_file_mtime(
        &second,
        filetime::FileTime::from_last_modification_time(&metadata),
    )
    .unwrap();
    let changed = Sioxc::new(second.clone(), &context).unwrap();
    assert_eq!(selected.verbose_version, changed.verbose_version);
    assert_eq!(metadata.len(), std::fs::metadata(&second).unwrap().len());
    assert_eq!(
        metadata.modified().unwrap(),
        std::fs::metadata(&second).unwrap().modified().unwrap()
    );
    assert_ne!(selected.fingerprint(), changed.fingerprint());
    assert_eq!(
        selected
            .output_filename(&root.join("missing.siox"), false, &context)
            .unwrap(),
        "missing.o"
    );
    let missing = Sioxc::new(root.join("absent/sioxc"), &context).unwrap_err();
    assert!(!missing.to_string().is_empty());
}

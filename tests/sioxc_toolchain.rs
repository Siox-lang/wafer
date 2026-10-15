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

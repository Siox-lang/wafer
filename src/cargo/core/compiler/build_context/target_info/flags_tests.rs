use super::*;
use crate::core::Shell;

const HOST: &str = "x86_64-unknown-linux-gnu";

fn context(root: &Path, config: &str, environment: &[(&str, &str)], wafer: bool) -> GlobalContext {
    let directory = root.join(if wafer { ".wafer" } else { ".cargo" });
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("config.toml"), config).unwrap();
    let mut context = if wafer {
        GlobalContext::new_wafer(Shell::new(), root.into(), root.join("home"))
    } else {
        GlobalContext::new(Shell::new(), root.into(), root.join("home"))
    };
    context.set_search_stop_path(root);
    context.set_env(
        environment
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
    );
    context
}

fn collect(
    context: &GlobalContext,
    requested: &[CompileKind],
    kind: CompileKind,
    flags: Flags,
) -> CargoResult<Vec<String>> {
    let cfg = ["unix", "target_os=\"linux\""]
        .into_iter()
        .map(|value| value.parse().unwrap())
        .collect::<Vec<_>>();
    extra_args(context, requested, HOST, Some(&cfg), kind, flags)
}

#[test]
fn sioxc_flags_reuse_encoded_plain_target_cfg_and_build_precedence() {
    let root = tempfile::tempdir().unwrap();
    let config = format!(
        "[build]\nsioxcflags=['build']\nrustflags=['wrong-rust']\n\
         [target.{HOST}]\nsioxcflags=['triple']\n\
         [target.'cfg(unix)']\nsioxcflags=['unix']\n\
         [target.'cfg(target_os = \"linux\")']\nsioxcflags=['linux']\n\
         [target.'cfg(target_os = \"other\")']\nsioxcflags=['wrong-platform']\n"
    );
    for (config, extra_environment, expected) in [
        (config.as_str(), vec![], vec!["triple", "linux", "unix"]),
        (
            config.as_str(),
            vec![("SIOXCFLAGS", " --debug  --std=a ")],
            vec!["--debug", "--std=a"],
        ),
        (
            config.as_str(),
            vec![
                ("SIOXCFLAGS", "wrong-lower-priority"),
                ("WAFER_ENCODED_SIOXCFLAGS", "--std\x1fa path"),
            ],
            vec!["--std", "a path"],
        ),
        (config.as_str(), vec![("SIOXCFLAGS", "")], vec![]),
        (
            config.as_str(),
            vec![("WAFER_ENCODED_SIOXCFLAGS", "")],
            vec![],
        ),
        ("[build]\nsioxcflags=['build']\n", vec![], vec!["build"]),
        (
            "[build]\nsioxcflags=['build']\n",
            vec![("WAFER_BUILD_SIOXCFLAGS", "environment")],
            vec!["build", "environment"],
        ),
        (
            config.as_str(),
            vec![(
                "WAFER_TARGET_X86_64_UNKNOWN_LINUX_GNU_SIOXCFLAGS",
                "environment",
            )],
            vec!["triple", "environment", "linux", "unix"],
        ),
        ("", vec![], vec![]),
    ] {
        let mut environment = vec![
            ("RUSTFLAGS", "wrong-rust"),
            ("CARGO_ENCODED_RUSTFLAGS", "wrong-rust"),
            ("CARGO_ENCODED_SIOXCFLAGS", "wrong-prefix"),
            ("CARGO_BUILD_SIOXCFLAGS", "wrong-prefix"),
        ];
        environment.extend(extra_environment);
        let context = context(root.path(), config, &environment, true);
        assert_eq!(
            collect(
                &context,
                &[CompileKind::Host],
                CompileKind::Host,
                Flags::Sioxc
            )
            .unwrap(),
            expected
        );
    }
}

#[test]
fn sioxc_host_artifacts_keep_cargos_explicit_target_isolation() {
    let root = tempfile::tempdir().unwrap();
    let context = context(
        root.path(),
        "[build]\nsioxcflags=['build']\n",
        &[("SIOXCFLAGS", "environment")],
        true,
    );
    let target = CompileKind::Target(CompileTarget::new(HOST).unwrap());
    assert!(
        collect(&context, &[target], CompileKind::Host, Flags::Sioxc)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        collect(&context, &[target], target, Flags::Sioxc).unwrap(),
        ["environment"]
    );
}

#[test]
fn compiler_flag_types_are_validated_and_rust_precedence_is_unchanged() {
    let root = tempfile::tempdir().unwrap();
    for config in [
        "[build]\nsioxcflags=7\n",
        "[target.x86_64-unknown-linux-gnu]\nsioxcflags=7\n",
        "[target.'cfg(unix)']\nsioxcflags=7\n",
    ] {
        let context = context(root.path(), config, &[], true);
        assert!(
            collect(
                &context,
                &[CompileKind::Host],
                CompileKind::Host,
                Flags::Sioxc
            )
            .is_err()
        );
    }
    let context = context(
        root.path(),
        "[build]\nrustflags=['rust-build']\nsioxcflags=['wrong-siox']\n",
        &[
            ("SIOXCFLAGS", "wrong-siox"),
            ("WAFER_ENCODED_SIOXCFLAGS", "wrong-siox"),
        ],
        false,
    );
    assert_eq!(
        collect(
            &context,
            &[CompileKind::Host],
            CompileKind::Host,
            Flags::Rust
        )
        .unwrap(),
        ["rust-build"]
    );
}

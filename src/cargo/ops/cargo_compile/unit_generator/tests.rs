use super::*;
use crate::core::Shell;
use crate::core::compiler::unit_graph::UnitGraph;
use crate::core::resolver::features::CliFeatures;
use crate::ops::cargo_compile::{CompileOptions, create_bcx, create_sioxc_bcx};
use crate::util::GlobalContext;
use std::{collections::BTreeMap, path::Path};

fn write(root: &Path, path: &str, value: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, value).unwrap();
}

fn fixture(root: &Path) {
    for (manifest, extension) in [("Wafer.toml", "siox"), ("Cargo.toml", "rs")] {
        write(
            root,
            manifest,
            &format!(
                "[workspace]\nmembers=[]\nexclude=['old','new','extra','host','other']\n\
                 [package]\nname='controller'\nversion='1.0.0'\nresolver='2'\n\
                 [features]\ndefault=[]\ncli=['dep:extra']\n\
                 [dependencies]\nfast={{package='codec',path='new'}}\nslow={{package='codec',path='old'}}\nextra={{path='extra',optional=true}}\n\
                 [target.'cfg(target_os = {:?})'.dependencies]\nhost={{path='host'}}\n\
                 [target.'cfg(target_os = \"not-this-os\")'.dependencies]\nother={{path='other'}}\n\
                 [[bin]]\nname='tool'\npath='src/bin/tool.{extension}'\nrequired-features=['cli']\n\
                 [profile.release]\nopt-level=2\ndebug=true\n",
                std::env::consts::OS
            ),
        );
        write(
            root,
            &format!("src/lib.{extension}"),
            "planning must not compile this",
        );
        write(root, &format!("src/bin/tool.{extension}"), "planning only");
        for (path, name, version) in [
            ("old", "codec", "1.0.0"),
            ("new", "codec", "2.0.0"),
            ("extra", "extra", "1.0.0"),
            ("host", "host", "1.0.0"),
            ("other", "other", "1.0.0"),
        ] {
            write(
                root,
                &format!("{path}/{manifest}"),
                &format!("[package]\nname='{name}'\nversion='{version}'\n"),
            );
            write(
                root,
                &format!("{path}/src/lib.{extension}"),
                "planning only",
            );
        }
    }
}

fn context(root: &Path, wafer: bool) -> GlobalContext {
    let mut context = if wafer {
        GlobalContext::new_wafer(Shell::new(), root.into(), root.join("wafer-home"))
    } else {
        GlobalContext::new(Shell::new(), root.into(), root.join("cargo-home"))
    };
    context.set_search_stop_path(root);
    let (key, compiler, forbidden) = if wafer {
        ("SIOXC", "WAFER_TEST_SIOXC", "RUSTC")
    } else {
        ("RUSTC", "WAFER_TEST_RUSTC", "SIOXC")
    };
    // Require actual compilers explicitly; no fake output or silent passing skip.
    #[allow(clippy::disallowed_methods)]
    let compiler = std::env::var(compiler).expect("set the native acceptance compiler paths");
    context.set_env(
        [
            (key.into(), compiler),
            (forbidden.into(), "must-not-run".into()),
            (
                "CARGO_INCREMENTAL".into(),
                if wafer { "1" } else { "0" }.into(),
            ),
            (
                "WAFER_INCREMENTAL".into(),
                if wafer { "0" } else { "1" }.into(),
            ),
        ]
        .into(),
    );
    context
}

fn plan(ws: &Workspace<'_>, options: &CompileOptions) -> CargoResult<(Vec<Unit>, UnitGraph)> {
    let interner = UnitInterner::new();
    if ws.gctx().is_wafer() {
        let context = create_sioxc_bcx(ws, options, &interner)?;
        Ok((context.roots, context.unit_graph))
    } else {
        let context = create_bcx(ws, options, &interner)?;
        Ok((context.roots, context.unit_graph))
    }
}

fn identity(unit: &Unit) -> String {
    format!(
        "{}@{}:{}:{:?}:{:?}:{:?}:{:?}",
        unit.pkg.name(),
        unit.pkg.version(),
        unit.target.name(),
        unit.mode,
        unit.kind,
        unit.profile,
        unit.features,
    )
}

fn snapshot(roots: &[Unit], graph: &UnitGraph) -> (Vec<String>, BTreeMap<String, Vec<String>>) {
    let mut roots: Vec<_> = roots.iter().map(identity).collect();
    roots.sort();
    let graph = graph
        .iter()
        .map(|(unit, deps)| {
            let mut deps: Vec<_> = deps
                .iter()
                .map(|dep| format!("{} -> {}", dep.extern_crate_name, identity(&dep.unit)))
                .collect();
            deps.sort();
            (identity(unit), deps)
        })
        .collect();
    (roots, graph)
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_RUSTC"]
fn actual_compilers_share_unit_planning_versions_features_profiles_and_filters() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fixture(root);
    let wafer_context = context(root, true);
    let cargo_context = context(root, false);
    let wafer_ws = Workspace::new(&root.join("Wafer.toml"), &wafer_context).unwrap();
    let cargo_ws = Workspace::new(&root.join("Cargo.toml"), &cargo_context).unwrap();

    for (intent, profile, all_features, bin_only) in [
        (UserIntent::Build, "dev", false, false),
        (UserIntent::Build, "release", true, false),
        (UserIntent::Check { test: false }, "dev", true, true),
    ] {
        let mut snapshots = Vec::new();
        for (ws, context) in [(&wafer_ws, &wafer_context), (&cargo_ws, &cargo_context)] {
            let mut options = CompileOptions::new(context, intent).unwrap();
            options.filter = CompileFilter::Default {
                required_features_filterable: true,
            };
            options.build_config.requested_profile = profile.into();
            options.cli_features = CliFeatures::new_all(all_features);
            if bin_only {
                options.filter = CompileFilter::single_bin("tool".into());
            }
            let (roots, graph) = plan(ws, &options).unwrap();
            assert_eq!(roots.len(), if all_features && !bin_only { 2 } else { 1 });
            assert_eq!(graph.len(), if all_features { 6 } else { 4 });
            assert!(graph.keys().all(|unit| unit.kind.is_host()));
            assert!(graph.keys().all(|unit| !unit.profile.incremental));
            assert!(graph.keys().all(|unit| unit.pkg.name() != "other"));
            let codecs: Vec<_> = graph
                .keys()
                .filter(|unit| unit.pkg.name() == "codec")
                .collect();
            assert_eq!(codecs.len(), 2);
            assert_ne!(codecs[0].pkg.package_id(), codecs[1].pkg.package_id());
            assert!(
                graph
                    .values()
                    .flatten()
                    .any(|dep| dep.extern_crate_name == "fast")
            );
            assert!(
                graph
                    .values()
                    .flatten()
                    .any(|dep| dep.extern_crate_name == "slow")
            );
            if profile == "release" {
                assert!(
                    graph
                        .keys()
                        .all(|unit| unit.profile.opt_level.as_str() == "2")
                );
            }
            snapshots.push(snapshot(&roots, &graph));
        }
        assert_eq!(snapshots[0], snapshots[1]);
    }

    let mut options = CompileOptions::new(&wafer_context, UserIntent::Build).unwrap();
    options.filter = CompileFilter::single_bin("tool".into());
    let error = plan(&wafer_ws, &options).unwrap_err();
    assert!(
        error.to_string().contains("requires the features"),
        "{error:#}"
    );
    assert!(!root.join("target/debug").exists());
}

#[test]
#[ignore = "requires actual WAFER_TEST_RUSTC"]
fn rust_unit_planning_preserves_configured_flags_and_link_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fixture(root);
    let initial_context = context(root, false);
    let initial_ws = Workspace::new(&root.join("Cargo.toml"), &initial_context).unwrap();
    let host = initial_context
        .load_global_rustc(Some(&initial_ws))
        .unwrap()
        .host;
    write(
        root,
        ".cargo/config.toml",
        &format!(
            "[build]\nrustflags=['--cfg=unit_planner']\nrustdocflags=['--cfg=doc_planner']\n\
             [target.{host}.test_native]\nrustc-link-lib=['native_test']\n"
        ),
    );
    let context = context(root, false);
    let ws = Workspace::new(&root.join("Cargo.toml"), &context).unwrap();
    let mut options = CompileOptions::new(&context, UserIntent::Build).unwrap();
    options.filter = CompileFilter::lib_only();
    let interner = UnitInterner::new();
    let bcx = create_bcx(&ws, &options, &interner).unwrap();
    let data = &bcx.target_data;
    for unit in bcx.unit_graph.keys() {
        assert_eq!(unit.rustflags.as_ref(), ["--cfg=unit_planner"]);
        assert_eq!(unit.rustdocflags.as_ref(), ["--cfg=doc_planner"]);
        assert_eq!(
            unit.links_overrides["test_native"].library_links,
            ["native_test"]
        );
        let options = data.unit_options(unit.kind);
        assert!(std::rc::Rc::ptr_eq(
            &options.compiler_flags,
            &data.info(unit.kind).rustflags
        ));
        assert!(std::rc::Rc::ptr_eq(
            &options.documentation_flags,
            &data.info(unit.kind).rustdocflags
        ));
        assert!(std::rc::Rc::ptr_eq(
            &options.links_overrides,
            &data.target_config(unit.kind).links_overrides
        ));
    }
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_RUSTC"]
fn shared_context_enforces_rust_msrv_only_for_the_rust_compiler() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fixture(root);
    for manifest in ["Cargo.toml", "Wafer.toml"] {
        let contents = std::fs::read_to_string(root.join(manifest)).unwrap();
        write(
            root,
            manifest,
            &contents.replacen(
                "version='1.0.0'",
                "version='1.0.0'\nrust-version='999.0'",
                1,
            ),
        );
    }
    for wafer in [true, false] {
        let context = context(root, wafer);
        let ws = Workspace::new(&root.join(context.manifest_filename()), &context).unwrap();
        let mut options = CompileOptions::new(&context, UserIntent::Build).unwrap();
        options.filter = CompileFilter::lib_only();
        let result = plan(&ws, &options);
        if wafer {
            assert!(result.is_ok(), "{}", result.unwrap_err());
        } else {
            let error = result.unwrap_err().to_string();
            assert!(error.contains("requires rustc 999"), "{error}");
            options.honor_rust_version = Some(false);
            assert!(plan(&ws, &options).is_ok());
        }
    }
}

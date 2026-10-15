use super::*;
use crate::core::Shell;
use crate::core::compiler::unit_dependencies::build_unit_dependencies;
use crate::core::compiler::unit_graph::UnitGraph;
use crate::core::compiler::{RustcTargetData, SioxcTargetData};
use crate::core::resolver::features::{CliFeatures, ForceAllTargets};
use crate::ops::cargo_compile::{CompileOptions, rebuild_unit_graph_shared};
use crate::ops::resolve::WorkspaceResolve;
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

// Assemble the same upstream planner stages without constructing a Rust build runner.
fn plan(
    ws: &Workspace<'_>,
    options: &CompileOptions,
    target_data: &mut dyn UnitTargetData,
) -> CargoResult<(Vec<Unit>, UnitGraph)> {
    let specs = options.spec.to_package_id_specs(ws)?;
    let requested = &options.build_config.requested_kinds;
    let WorkspaceResolve {
        pkg_set,
        workspace_resolve,
        targeted_resolve: resolve,
        specs_and_features,
    } = crate::ops::resolve_ws_with_opts(
        ws,
        target_data,
        requested,
        &options.cli_features,
        &specs,
        HasDevUnits::No,
        ForceAllTargets::No,
        false,
    )?;
    let packages = pkg_set.get_many(resolve.specs_to_ids(&specs)?)?;
    let profiles = Profiles::new(ws, options.build_config.requested_profile)?;
    let explicit_host_kind = CompileKind::Target(crate::core::compiler::CompileTarget::new(
        &target_data.host_triple(),
    )?);
    let interner = UnitInterner::new();
    let mut roots = Vec::new();
    let mut graph = UnitGraph::new();
    for group in &specs_and_features {
        let generator = UnitGenerator {
            ws,
            packages: &packages,
            spec: &options.spec,
            target_data,
            filter: &options.filter,
            requested_kinds: requested,
            explicit_host_kind,
            intent: options.build_config.intent,
            resolve: &resolve,
            workspace_resolve: &workspace_resolve,
            resolved_features: &group.resolved_features,
            package_set: &pkg_set,
            profiles: &profiles,
            interner: &interner,
            has_dev_units: HasDevUnits::No,
        };
        let units = generator.generate_root_units()?;
        graph.extend(build_unit_dependencies(
            ws,
            &pkg_set,
            &resolve,
            &group.resolved_features,
            None,
            &units,
            &[],
            &HashMap::new(),
            options.build_config.intent,
            target_data,
            &profiles,
            &interner,
        )?);
        roots.extend(units);
    }
    let (roots, _, graph) = rebuild_unit_graph_shared(
        &interner,
        graph,
        &roots,
        &[],
        Some(explicit_host_kind),
        false,
    );
    Ok((roots, graph))
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
        for (ws, context, wafer) in [
            (&wafer_ws, &wafer_context, true),
            (&cargo_ws, &cargo_context, false),
        ] {
            let mut options = CompileOptions::new(context, intent).unwrap();
            options.filter = CompileFilter::Default {
                required_features_filterable: true,
            };
            options.build_config.requested_profile = profile.into();
            options.cli_features = CliFeatures::new_all(all_features);
            if bin_only {
                options.filter = CompileFilter::single_bin("tool".into());
            }
            let mut target_data: Box<dyn UnitTargetData + '_> = if wafer {
                Box::new(SioxcTargetData::new(ws, &options.build_config.requested_kinds).unwrap())
            } else {
                Box::new(RustcTargetData::new(ws, &options.build_config.requested_kinds).unwrap())
            };
            let (roots, graph) = plan(ws, &options, target_data.as_mut()).unwrap();
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
    let mut data = SioxcTargetData::new(&wafer_ws, &options.build_config.requested_kinds).unwrap();
    let error = plan(&wafer_ws, &options, &mut data).unwrap_err();
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
    let mut data = RustcTargetData::new(&ws, &options.build_config.requested_kinds).unwrap();
    let (_, graph) = plan(&ws, &options, &mut data).unwrap();
    for unit in graph.keys() {
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

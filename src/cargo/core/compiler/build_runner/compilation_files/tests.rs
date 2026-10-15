use super::*;
use crate::core::Shell;
use crate::core::compiler::{Compilation, UnitInterner, UserIntent, unit::UnitInner};
use crate::ops::{CompileFilter, CompileOptions, create_sioxc_bcx};
use crate::util::GlobalContext;
use std::{collections::HashSet, ffi::OsString};

fn tool(name: &str) -> OsString {
    // Acceptance must query and execute the actual compiler, not a fixture script.
    #[allow(clippy::disallowed_methods)]
    std::env::var_os(name).unwrap_or_else(|| panic!("set {name}"))
}

fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn fixture(root: &Path) -> GlobalContext {
    write(
        root,
        "Wafer.toml",
        "[workspace]\nmembers=[]\nexclude=['old','new']\n[package]\nname='controller'\nversion='1.0.0'\n[dependencies]\nfirst={package='codec',path='old'}\nsecond={package='codec',path='new'}\n",
    );
    for (directory, version) in [("old", "1.0.0"), ("new", "2.0.0")] {
        write(
            root,
            &format!("{directory}/Wafer.toml"),
            &format!("[package]\nname='codec'\nversion='{version}'\n"),
        );
    }
    for directory in ["", "old/", "new/"] {
        write(
            root,
            &format!("{directory}src/lib.siox"),
            "module design; entity Design { value: integer out } impl Design { value = 7; }\n",
        );
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
        ]
        .into(),
    );
    context
}

fn intern(interner: &UnitInterner, unit: UnitInner) -> Unit {
    interner.intern(
        &unit.pkg,
        &unit.target,
        unit.profile,
        unit.kind,
        unit.mode,
        unit.features,
        unit.rustflags,
        unit.rustdocflags,
        unit.links_overrides,
        unit.is_std,
        unit.dep_hash,
        unit.artifact,
        unit.artifact_target_for_features,
        unit.skip_non_compile_time_dep,
    )
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC and WAFER_TEST_STD"]
fn actual_sioxc_outputs_use_the_shared_layout_and_separate_package_versions() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let context = fixture(root);
    let ws = Workspace::new(&root.join("Wafer.toml"), &context).unwrap();
    let mut options = CompileOptions::new(&context, UserIntent::Build).unwrap();
    options.filter = CompileFilter::lib_only();
    options.build_config.export_dir = Some(root.join("export"));
    let interner = UnitInterner::new();
    let mut bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
    let mut test = (*bcx.roots[0]).clone();
    test.mode = CompileMode::Test;
    let test = intern(&interner, test);
    bcx.unit_graph.insert(test.clone(), vec![]);
    bcx.roots.push(test.clone());
    let lto = bcx
        .unit_graph
        .keys()
        .map(|unit| (unit.clone(), Lto::OnlyObject))
        .collect();
    let mut layout = Layout::new(&ws, None, "debug").unwrap();
    layout.prepare().unwrap();
    let files = CompilationFiles::new(&bcx, &lto, layout, HashMap::new());
    let mut compilation = Compilation::new(&bcx).unwrap();
    compilation
        .root_output
        .insert(CompileKind::Host, files.host_dest().into());
    compilation
        .deps_output
        .insert(CompileKind::Host, files.host_deps().into());
    let mut paths = HashSet::new();
    for unit in bcx.unit_graph.keys() {
        let outputs = files.outputs(unit, &bcx).unwrap();
        assert!(Arc::ptr_eq(&outputs, &files.outputs(unit, &bcx).unwrap()));
        assert_eq!(outputs.len(), 1);
        let output = &outputs[0];
        let CompilerRef::Sioxc(compiler) = bcx.target_data.compiler() else {
            panic!("selected Rust for Siox artifacts")
        };
        let queried = compiler
            .output_filename(
                Path::new(&format!("{}.siox", unit.target.crate_name())),
                unit.mode == CompileMode::Test,
                &context,
            )
            .unwrap();
        assert_eq!(output.path.extension(), Path::new(&queried).extension());
        assert!(
            paths.insert(output.path.clone()),
            "two units share an output"
        );
        assert_eq!(output.path.parent(), Some(files.host_deps()));
        assert!(
            output
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains(&files.metadata(unit).unit_id().to_string())
        );
        assert_eq!(output.flavor, FileFlavor::Normal);
        if unit.mode == CompileMode::Test {
            assert!(output.hardlink.is_none());
            assert!(output.export_path.is_none());
            write(
                root,
                "src/lib.siox",
                "module design; #[test] entity Design {} impl Design { process { assert!(1 + 1 == 2); } }\n",
            );
        } else if bcx.roots.contains(unit) {
            assert_eq!(
                output.hardlink.as_ref().unwrap().file_name(),
                Some(std::ffi::OsStr::new(&queried))
            );
            assert_eq!(
                output.hardlink.as_ref().unwrap().parent(),
                Some(files.host_dest())
            );
            assert_eq!(
                output.export_path.as_ref().unwrap().parent(),
                Some(root.join("export").as_path())
            );
        } else {
            assert!(output.hardlink.is_none());
            assert!(output.export_path.is_none());
        }
        let source = unit.target.src_path().path().unwrap();
        if unit.mode != CompileMode::Test {
            std::fs::write(
                source,
                "module design; entity Design { value: integer out } impl Design { value = 7; }\n",
            )
            .unwrap();
        }
        let mut command = compilation
            .compiler_process(unit, true, ws.is_member(&unit.pkg))
            .unwrap();
        command
            .arg(source)
            .arg("--std")
            .arg(tool("WAFER_TEST_STD"))
            .arg("--out")
            .arg(&output.path);
        if unit.mode == CompileMode::Test {
            command.arg("--test");
        }
        command.exec().unwrap();
        assert!(std::fs::metadata(&output.path).unwrap().len() > 0);
        if unit.mode == CompileMode::Test {
            compilation
                .target_process(&output.path, unit.kind, &unit.pkg, None)
                .unwrap()
                .arg("-o")
                .arg(root.join("layout.vcd"))
                .exec()
                .unwrap();
            assert!(root.join("layout.vcd").is_file());
        }
    }
    assert_eq!(paths.len(), 4);
    // These are independently emitted native objects, not linked/imported compiled libraries.
    assert!(!root.join("export").exists());
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC"]
fn artifact_metadata_tracks_units_and_flags_not_source_edits_or_cargo_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let context = fixture(dir.path());
    let ws = Workspace::new(&dir.path().join("Wafer.toml"), &context).unwrap();
    let options = CompileOptions::new(&context, UserIntent::Build).unwrap();
    let interner = UnitInterner::new();
    let mut bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
    let root = bcx.roots[0].clone();
    let mut variants = vec![root.clone()];
    for change in 0..3 {
        let mut unit = (*root).clone();
        match change {
            0 => unit.mode = CompileMode::Test,
            1 => unit.rustflags = vec!["--debug".to_owned()].into(),
            _ => unit.features = vec!["selected".into()],
        }
        let unit = intern(&interner, unit);
        bcx.unit_graph
            .insert(unit.clone(), bcx.unit_graph[&root].clone());
        variants.push(unit);
    }
    let lto = bcx
        .unit_graph
        .keys()
        .map(|unit| (unit.clone(), Lto::OnlyObject))
        .collect();
    let mut metas = HashMap::new();
    let hashes: HashSet<_> = variants
        .iter()
        .map(|unit| metadata_of(unit, &bcx, &lto, &mut metas).unit_id())
        .collect();
    assert_eq!(hashes.len(), variants.len());
    let before = metas[&root];
    std::fs::write(
        root.target.src_path().path().unwrap(),
        "changed source is a freshness input, not an artifact namespace",
    )
    .unwrap();
    let after = *metadata_of(&root, &bcx, &lto, &mut HashMap::new());
    assert_eq!(before.unit_id(), after.unit_id());
    assert_eq!(before.c_metadata(), after.c_metadata());
    let mut other_context = fixture(dir.path());
    other_context.set_env(
        [
            (
                "SIOXC".into(),
                tool("WAFER_TEST_SIOXC").into_string().unwrap(),
            ),
            (
                "__CARGO_DEFAULT_LIB_METADATA".into(),
                "must-not-affect-siox".into(),
            ),
        ]
        .into(),
    );
    let other_ws = Workspace::new(&dir.path().join("Wafer.toml"), &other_context).unwrap();
    let other_options = CompileOptions::new(&other_context, UserIntent::Build).unwrap();
    let other_interner = UnitInterner::new();
    let other = create_sioxc_bcx(&other_ws, &other_options, &other_interner).unwrap();
    let other_lto = other
        .unit_graph
        .keys()
        .map(|unit| (unit.clone(), Lto::OnlyObject))
        .collect();
    let other_meta = *metadata_of(&other.roots[0], &other, &other_lto, &mut HashMap::new());
    assert_eq!(before.unit_id(), other_meta.unit_id());
}

#[test]
#[ignore = "requires actual WAFER_TEST_SIOXC"]
fn unsupported_sioxc_output_modes_do_not_invent_rust_metadata_or_documentation() {
    let dir = tempfile::tempdir().unwrap();
    let context = fixture(dir.path());
    let ws = Workspace::new(&dir.path().join("Wafer.toml"), &context).unwrap();
    let options = CompileOptions::new(&context, UserIntent::Build).unwrap();
    let interner = UnitInterner::new();
    let mut bcx = create_sioxc_bcx(&ws, &options, &interner).unwrap();
    let root = bcx.roots[0].clone();
    let mut variants = Vec::new();
    for mode in [
        CompileMode::Check { test: false },
        CompileMode::Doc,
        CompileMode::Doctest,
        CompileMode::Docscrape,
        CompileMode::RunCustomBuild,
    ] {
        let mut unit = (*root).clone();
        unit.mode = mode;
        let unit = intern(&interner, unit);
        bcx.unit_graph.insert(unit.clone(), vec![]);
        bcx.roots.push(unit.clone());
        variants.push(unit);
    }
    let lto = bcx
        .unit_graph
        .keys()
        .map(|unit| (unit.clone(), Lto::OnlyObject))
        .collect();
    let layout = Layout::new(&ws, None, "debug").unwrap();
    let files = CompilationFiles::new(&bcx, &lto, layout, HashMap::new());
    for unit in variants {
        let error = files.outputs(&unit, &bcx).unwrap_err().to_string();
        assert!(
            error.contains("only supports native object and test outputs"),
            "{error}"
        );
    }
    assert!(!files.host_deps().exists());
}

#[test]
fn filename_probe_requires_a_single_compiler_owned_placeholder() {
    let file = FileType::from_compiler_filename("prefix___.object", "___").unwrap();
    assert_eq!(file.flavor, FileFlavor::Normal);
    assert!(file.crate_type.is_none());
    for invalid in ["plain.o", "___-___.o"] {
        assert!(FileType::from_compiler_filename(invalid, "___").is_err());
    }
}

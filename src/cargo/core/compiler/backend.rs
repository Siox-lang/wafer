//! Compiler-specific work inside Cargo's single planning/fingerprint/scheduling pipeline.

use super::*;

pub trait CompilerBackend<'gctx>: UnitTargetData + Sized {
    fn prepare_runner(_runner: &mut BuildRunner<'_, 'gctx, Self>) -> CargoResult<()> {
        Ok(())
    }
    fn check_documentation(_runner: &BuildRunner<'_, 'gctx, Self>) -> CargoResult<()> {
        Ok(())
    }
    fn work(
        runner: &mut BuildRunner<'_, 'gctx, Self>,
        unit: &Unit,
        exec: &Arc<dyn Executor>,
    ) -> CargoResult<Work>;
    fn prepare_custom_build(
        _runner: &mut BuildRunner<'_, 'gctx, Self>,
        _unit: &Unit,
    ) -> CargoResult<Job> {
        anyhow::bail!("selected compiler does not support Rust build scripts")
    }
    fn finish_root(_runner: &mut BuildRunner<'_, 'gctx, Self>, _unit: &Unit) -> CargoResult<()> {
        Ok(())
    }
}

impl<'gctx> CompilerBackend<'gctx> for RustcTargetData<'gctx> {
    fn prepare_runner(runner: &mut BuildRunner<'_, 'gctx>) -> CargoResult<()> {
        custom_build::build_map(runner)
    }
    fn check_documentation(runner: &BuildRunner<'_, 'gctx>) -> CargoResult<()> {
        if runner.bcx.build_config.intent.is_doc() {
            RustDocFingerprint::check_rustdoc_fingerprint(runner)?;
        }
        Ok(())
    }
    fn work(
        runner: &mut BuildRunner<'_, 'gctx>,
        unit: &Unit,
        exec: &Arc<dyn Executor>,
    ) -> CargoResult<Work> {
        if !runner.bcx.build_config.build_plan && (unit.mode.is_doc() || unit.mode.is_doc_scrape())
        {
            rustdoc(runner, unit)
        } else {
            rustc(runner, unit, exec)
        }
    }
    fn prepare_custom_build(runner: &mut BuildRunner<'_, 'gctx>, unit: &Unit) -> CargoResult<Job> {
        custom_build::prepare(runner, unit)
    }
    fn finish_root(runner: &mut BuildRunner<'_, 'gctx>, unit: &Unit) -> CargoResult<()> {
        if unit.mode.is_doc_test() {
            let mut unstable_opts = false;
            let mut args = extern_args(runner, unit, &mut unstable_opts)?;
            args.extend(lto_args(runner, unit));
            args.extend(features_args(unit));
            args.extend(check_cfg_args(unit));
            let script_metas = runner.find_build_script_metadatas(unit);
            if let Some(meta_vec) = script_metas.clone() {
                for meta in meta_vec {
                    if let Some(output) = runner.build_script_outputs.lock().unwrap().get(meta) {
                        for cfg in &output.cfgs {
                            args.push("--cfg".into());
                            args.push(cfg.into());
                        }
                        for cfg in &output.check_cfgs {
                            args.push("--check-cfg".into());
                            args.push(cfg.into());
                        }
                        for (lt, arg) in &output.linker_args {
                            if lt.applies_to(&unit.target, unit.mode) {
                                args.push("-C".into());
                                args.push(format!("link-arg={arg}").into());
                            }
                        }
                    }
                }
            }
            args.extend(unit.rustdocflags.iter().map(Into::into));
            let format = match runner.bcx.build_config.message_format {
                MessageFormat::Short => "short",
                MessageFormat::Human => "human",
                MessageFormat::Json { .. } => "json",
            };
            args.push("--error-format".into());
            args.push(format.into());
            runner.compilation.to_doc_test.push(compilation::Doctest {
                unit: unit.clone(),
                args,
                unstable_opts,
                linker: runner.compilation.target_linker(unit.kind).clone(),
                script_metas,
                env: artifact::get_env(runner, runner.unit_deps(unit))?,
            });
        }
        output_depinfo(runner, unit)
    }
}

impl<'gctx> CompilerBackend<'gctx> for SioxcTargetData {
    fn prepare_runner(runner: &mut BuildRunner<'_, 'gctx, Self>) -> CargoResult<()> {
        for unit in runner.bcx.unit_graph.keys() {
            sioxc::validate(runner, unit)?;
        }
        Ok(())
    }
    fn work(
        runner: &mut BuildRunner<'_, 'gctx, Self>,
        unit: &Unit,
        exec: &Arc<dyn Executor>,
    ) -> CargoResult<Work> {
        sioxc::prepare(runner, unit, exec)
    }
}

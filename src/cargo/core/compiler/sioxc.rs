//! Native Sioxc work, using Cargo's existing executor, input cache and artifact handling.

use super::{
    BuildRunner, CompileMode, Executor, MessageFormat, OutputOptions, SioxcTargetData, Unit, Work,
    fingerprint, on_stderr_line, on_stdout_line,
};
use crate::util::CargoResult;
use cargo_util::paths;
use cargo_util_schemas::manifest::TomlDebugInfo;
use std::{fs, sync::Arc};

/// Only forward known compiler options; Wafer owns all source/output/mode arguments.
pub(super) fn validate_flags(flags: &[String], mode: CompileMode) -> CargoResult<()> {
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        anyhow::ensure!(!flag.contains('\0'), "NUL in Sioxc flags");
        if matches!(flag.as_str(), "-g" | "--debug" | "-v" | "--verbose") {
            continue;
        }
        let (name, inline) = if flag.starts_with("--") {
            flag.split_once('=')
                .map_or((flag.as_str(), None), |(name, value)| (name, Some(value)))
        } else if flag.len() >= 2 && matches!(&flag.as_bytes()[..2], b"-A" | b"-W" | b"-D" | b"-F")
        {
            let (name, value) = flag.split_at(2);
            (
                name,
                (!value.is_empty()).then_some(value.strip_prefix('=').unwrap_or(value)),
            )
        } else {
            (flag.as_str(), None)
        };
        anyhow::ensure!(
            matches!(
                name,
                "--std"
                    | "--top"
                    | "--allow"
                    | "--warn"
                    | "--deny"
                    | "--forbid"
                    | "-A"
                    | "-W"
                    | "-D"
                    | "-F"
            ),
            "Sioxc flag {flag:?} is unsupported or owned by Wafer (source, output, dependency info and mode/query flags)"
        );
        anyhow::ensure!(
            name != "--top" || mode != CompileMode::Test,
            "Sioxc --top does not select native test entities"
        );
        let value = inline.or_else(|| flags.next().map(String::as_str));
        anyhow::ensure!(
            value.is_some_and(|value| !value.is_empty()
                && !value.contains('\0')
                && (inline.is_some() || !value.starts_with('-'))),
            "missing or invalid value for Sioxc {name}"
        );
    }
    Ok(())
}

pub(super) fn validate(
    runner: &BuildRunner<'_, '_, SioxcTargetData>,
    unit: &Unit,
) -> CargoResult<()> {
    validate_flags(&unit.rustflags, unit.mode)?;
    anyhow::ensure!(
        matches!(unit.mode, CompileMode::Build | CompileMode::Test),
        "unsupported Sioxc compilation mode"
    );
    anyhow::ensure!(
        !runner.bcx.build_config.build_plan && !runner.bcx.build_config.dry_run,
        "Sioxc native work does not implement build-plan/dry-run execution"
    );
    anyhow::ensure!(
        matches!(runner.bcx.build_config.message_format, MessageFormat::Human),
        "Sioxc native work currently supports human diagnostics only"
    );
    anyhow::ensure!(
        unit.profile.opt_level.as_str() == "0"
            && unit.profile.debuginfo.into_inner() == TomlDebugInfo::Full,
        "Sioxc currently supports opt-level=0 with full debug info; other Cargo optimization/debug profiles need compiler support"
    );
    anyhow::ensure!(
        unit.profile.codegen_backend.is_none()
            && unit.profile.codegen_units.is_none()
            && unit.profile.split_debuginfo.is_none()
            && !unit.profile.rpath
            && unit.profile.rustflags.is_empty()
            && unit.profile.trim_paths.is_none(),
        "Sioxc does not support Rust-specific backend/codegen/profile flags"
    );
    Ok(())
}

/// Prepare real native compilation; the common pipeline owns fingerprints and uplift.
pub(crate) fn prepare(
    runner: &mut BuildRunner<'_, '_, SioxcTargetData>,
    unit: &Unit,
    exec: &Arc<dyn Executor>,
) -> CargoResult<Work> {
    validate(runner, unit)?;
    let source = unit
        .target
        .src_path()
        .path()
        .ok_or_else(|| anyhow::anyhow!("Sioxc requires a source file"))?;
    let outputs = runner.outputs(unit)?;
    anyhow::ensure!(
        outputs.len() == 1,
        "Sioxc native work requires one compiler-owned artifact"
    );
    let output = outputs[0].path.clone();
    let raw = output.with_extension("d");
    let mut command = runner.compilation.compiler_process(
        unit,
        runner.is_primary_package(unit),
        runner.bcx.ws.is_member(&unit.pkg),
    )?;
    command
        .arg(source)
        .arg("--emit=object")
        .arg("--out")
        .arg(&output)
        .arg("--dep-info")
        .arg(&raw);
    if !unit
        .rustflags
        .iter()
        .any(|flag| matches!(flag.as_str(), "--debug" | "-g"))
    {
        command.arg("--debug");
    }
    command
        .args(&unit.rustflags)
        .inherit_jobserver(&runner.jobserver);
    if unit.mode == CompileMode::Test {
        command.arg("--test");
    }
    let encoded = fingerprint::dep_info_loc(runner, unit);
    let directory = runner.files().fingerprint_dir(unit);
    let package = unit.pkg.root().to_path_buf();
    let cwd = command.get_cwd().unwrap().to_path_buf();
    let build = runner.bcx.ws.build_dir().into_path_unlocked();
    let environment = Arc::clone(runner.bcx.gctx.env_config()?);
    let mut options = OutputOptions::new(runner, unit);
    let target = unit.target.clone();
    let package_id = unit.pkg.package_id();
    let manifest = unit.pkg.manifest_path().to_path_buf();
    let mode = unit.mode;
    let exec = Arc::clone(exec);
    let compile = Work::new(move |state| {
        for path in [&output, &raw] {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        state.running(&command);
        let timestamp = paths::set_invocation_time(&directory)?;
        exec.exec(
            &command,
            package_id,
            &target,
            mode,
            &mut |line| on_stdout_line(state, line, package_id, &target),
            &mut |line| on_stderr_line(state, line, package_id, &manifest, &target, &mut options),
        )?;
        let metadata = fs::metadata(&output)?;
        anyhow::ensure!(
            metadata.is_file() && metadata.len() > 0,
            "Sioxc did not produce a nonempty regular artifact"
        );
        fingerprint::translate_dep_info(
            &raw,
            &encoded,
            &cwd,
            &package,
            &build,
            &command,
            true,
            &environment,
            fingerprint::DepInfoFormat::Sioxc,
        )?;
        paths::set_file_time_no_err(&encoded, timestamp);
        Ok(())
    });
    Ok(compile)
}

#[cfg(test)]
mod tests;

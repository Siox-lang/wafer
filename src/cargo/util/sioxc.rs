//! Actual Sioxc toolchain queries; no Rust query output is fabricated.

use crate::core::compiler::apply_env_config;
use crate::util::{CargoResult, GlobalContext, hash_u64};
use anyhow::{Context as _, bail};
use cargo_util::{ProcessBuilder, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Sioxc {
    pub path: PathBuf,
    pub version: semver::Version,
    pub host: String,
    pub native_target: String,
    pub host_cfg: Vec<cargo_platform::Cfg>,
    pub verbose_version: String,
    executable_hash: [u8; 32],
}

impl Sioxc {
    pub fn new(path: PathBuf, gctx: &GlobalContext) -> CargoResult<Self> {
        let path = resolve_executable(&Self::command(&path, gctx)?)?;
        let mut command = Self::command(&path, gctx)?;
        command.arg("-vV");
        let verbose_version = output(&command).context("failed to query Sioxc version")?;
        let (version, host) = parse_version(&verbose_version)?;
        let mut command = Self::command(&path, gctx)?;
        command.arg("--print=target-triple");
        let target = output(&command).context("failed to query Sioxc native target")?;
        let native_target = single_line(&target, "native target")?.to_owned();
        if native_target.chars().any(char::is_whitespace) {
            bail!("Sioxc returned an invalid native target: {native_target:?}");
        }
        let mut command = Self::command(&path, gctx)?;
        command.arg("--print=cfg");
        let cfg = output(&command).context("failed to query Sioxc execution-host cfg")?;
        let host_cfg = cfg
            .lines()
            .map(str::parse)
            .collect::<Result<Vec<cargo_platform::Cfg>, _>>()
            .context("invalid Sioxc execution-host cfg")?;
        if host_cfg.is_empty() {
            bail!("Sioxc returned no execution-host cfg facts");
        }
        let executable_hash = Sha256::new().update_path(&path)?.finish();
        Ok(Self {
            path,
            version,
            host,
            native_target,
            host_cfg,
            verbose_version,
            executable_hash,
        })
    }

    /// Include executable bytes: embedded core/runtime and ABI features can share a version.
    pub fn fingerprint(&self) -> u64 {
        // shortcut: external tools/libraries are not included; track them before claiming full toolchain identity.
        hash_u64((
            "sioxc",
            self.executable_hash,
            &self.verbose_version,
            &self.native_target,
            &self.host_cfg,
        ))
    }

    fn command(path: &Path, gctx: &GlobalContext) -> CargoResult<ProcessBuilder> {
        let mut command = ProcessBuilder::new(path);
        command.cwd(gctx.cwd());
        apply_env_config(gctx, &mut command)?;
        Ok(command)
    }

    /// Ask the compiler for its filename without reading or compiling the input.
    pub fn output_filename(
        &self,
        input: &Path,
        test: bool,
        gctx: &GlobalContext,
    ) -> CargoResult<String> {
        let mut command = Self::command(&self.path, gctx)?;
        command.arg(input).arg("--print=file-names");
        if test {
            command.arg("--test");
        }
        let filename = output(&command).context("failed to query Sioxc artifact filename")?;
        let filename = single_line(&filename, "artifact filename")?;
        if Path::new(filename)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(filename)
        {
            bail!("Sioxc returned an invalid artifact filename: {filename:?}");
        }
        Ok(filename.to_owned())
    }
}

fn resolve_executable(command: &ProcessBuilder) -> CargoResult<PathBuf> {
    let program = Path::new(command.get_program());
    let cwd = command
        .get_cwd()
        .expect("Sioxc query sets its working directory");
    if program.components().count() > 1 || program.is_absolute() {
        return Ok(cwd.join(program));
    }
    let path = command.get_env("PATH").context("no PATH for Sioxc")?;
    for directory in std::env::split_paths(&path) {
        let candidate = cwd.join(directory).join(program);
        for candidate in std::iter::once(candidate.clone()).chain(
            (!std::env::consts::EXE_EXTENSION.is_empty())
                .then(|| candidate.with_extension(std::env::consts::EXE_EXTENSION)),
        ) {
            if !candidate.is_file() {
                continue;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if candidate.metadata()?.permissions().mode() & 0o111 == 0 {
                    continue;
                }
            }
            return Ok(candidate);
        }
    }
    bail!(
        "no executable for `{}` found in Sioxc PATH",
        program.display()
    )
}

fn output(command: &ProcessBuilder) -> CargoResult<String> {
    let output = command.exec_with_output()?;
    String::from_utf8(output.stdout).context("Sioxc query output is not UTF-8")
}

fn single_line<'a>(output: &'a str, label: &str) -> CargoResult<&'a str> {
    let mut lines = output.lines();
    let value = lines.next().filter(|line| !line.is_empty());
    match (value, lines.next()) {
        (Some(value), None) if !value.chars().any(char::is_control) => Ok(value),
        _ => bail!("Sioxc returned invalid {label}: {output:?}"),
    }
}

fn parse_version(output: &str) -> CargoResult<(semver::Version, String)> {
    let mut lines = output.lines();
    let version = lines
        .next()
        .and_then(|line| line.strip_prefix("sioxc "))
        .context("expected Sioxc's own version header, not rustc output")?;
    let version = semver::Version::parse(version).context("invalid Sioxc release version")?;
    let mut binary = None;
    let mut release = None;
    let mut host = None;
    for line in lines {
        let field = if let Some(value) = line.strip_prefix("binary: ") {
            Some((&mut binary, value))
        } else if let Some(value) = line.strip_prefix("release: ") {
            Some((&mut release, value))
        } else {
            line.strip_prefix("host: ").map(|value| (&mut host, value))
        };
        if let Some((field, value)) = field {
            if field.replace(value).is_some() {
                bail!("duplicate field in Sioxc version output: {line}");
            }
        }
    }
    if binary != Some("sioxc") || release != Some(version.to_string().as_str()) {
        bail!("inconsistent Sioxc identity in version output");
    }
    let host = host
        .filter(|host| !host.is_empty() && !host.chars().any(char::is_whitespace))
        .context("missing or invalid Sioxc build host")?;
    Ok((version, host.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parser_accepts_sioxc_and_rejects_other_or_inconsistent_tools() {
        let valid = "sioxc 0.1.0\nbinary: sioxc\nrelease: 0.1.0\nhost: x86_64-unknown-linux-gnu\nLLVM version: 23.1.1\n";
        assert_eq!(
            parse_version(valid).unwrap(),
            (
                semver::Version::new(0, 1, 0),
                "x86_64-unknown-linux-gnu".to_owned()
            )
        );
        for invalid in [
            valid.replace("sioxc 0.1.0", "rustc 1.90.0"),
            valid.replace("release: 0.1.0", "release: 0.2.0"),
            valid.replace("host: x86_64-unknown-linux-gnu", "host: "),
            format!("{valid}host: duplicate\n"),
        ] {
            assert!(parse_version(&invalid).is_err(), "{invalid}");
        }
        assert_eq!(single_line("smoke.o\n", "filename").unwrap(), "smoke.o");
        for invalid in ["", "\n", "first\nsecond\n", "name\t.o\n"] {
            assert!(single_line(invalid, "filename").is_err(), "{invalid:?}");
        }
    }
}

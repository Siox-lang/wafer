//! Project orchestration; Siox owns semantic analysis and native test discovery.

use crate::{
    manifest::{self, Manifest},
    project::{Package, Project},
    Result,
};
use clap::ValueEnum;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Wave {
    Vcd,
    Fst,
    Both,
}

pub struct Runner {
    pub compiler: PathBuf,
    pub std: PathBuf,
    pub debug: bool,
    pub cache: bool,
}

impl Runner {
    pub fn configured(
        manifest: &Manifest,
        root: &Path,
        compiler: Option<PathBuf>,
        std: Option<PathBuf>,
        debug: bool,
    ) -> Result<Self> {
        let current = std::env::current_dir()?;
        let (compiler, base) =
            if let Some(path) = compiler.or_else(|| std::env::var_os("SIOXC").map(PathBuf::from)) {
                (path, current.as_path())
            } else if let Some(path) = &manifest.toolchain.sioxc {
                (path.clone(), root)
            } else {
                (PathBuf::from("sioxc"), current.as_path())
            };
        let compiler = if compiler.components().count() > 1 || compiler.is_absolute() {
            base.join(compiler).canonicalize()?
        } else {
            compiler
        };
        let (std, base) =
            if let Some(path) = std.or_else(|| std::env::var_os("SIOX_STD").map(PathBuf::from)) {
                (path, current.as_path())
            } else {
                (
                    manifest
                        .toolchain
                        .std
                        .clone()
                        .unwrap_or_else(|| PathBuf::from("std")),
                    root,
                )
            };
        let std = base.join(std).canonicalize()?;
        let std = if std.join("std/prelude.siox").is_file() {
            std.join("std")
        } else {
            std
        };
        if !std.join("prelude.siox").is_file() {
            return Err("--std must name matching Siox core/std libraries".into());
        }
        Ok(Self {
            compiler,
            std,
            debug,
            cache: false,
        })
    }

    fn command(&self, project: &Project, entry: &Path) -> Result<Command> {
        let mut command = Command::new(&self.compiler);
        let root = root(project)?
            .manifest_path
            .parent()
            .ok_or("manifest has no parent")?;
        command
            .current_dir(root)
            .arg(entry)
            .arg("--std")
            .arg(&self.std);
        if self.debug {
            command.arg("--debug");
        }
        for (name, path) in &project.modules {
            if path != entry {
                command.arg("--module").arg(format!(
                    "{name}={}",
                    path.to_str().ok_or("compiler module paths must be UTF-8")?
                ));
            }
        }
        Ok(command)
    }

    fn compile(
        &self,
        project: &Project,
        entry: &Path,
        emit: &str,
        output: Option<&Path>,
        sources: Option<&[PathBuf]>,
        top: Option<&str>,
    ) -> Result<String> {
        let mut command = self.command(project, entry)?;
        if emit == "test" {
            command.arg("--test");
        } else {
            command.arg("--emit").arg(emit);
        }
        if let Some(output) = output {
            if std::fs::symlink_metadata(output)
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
            {
                return Err("refusing to overwrite an artifact symlink".into());
            }
            command.arg("--out").arg(output);
        }
        if let Some(sources) = sources {
            for source in sources {
                command.arg("--test-source").arg(source);
            }
        }
        if let Some(top) = top {
            command.arg("--top").arg(top);
        }
        let fingerprint = if self.cache && output.is_some() {
            let mut stable = self.command(project, entry)?;
            if emit == "test" {
                stable.arg("--test");
            } else {
                stable.arg("--emit").arg(emit);
            }
            if let Some(sources) = sources {
                for source in sources {
                    stable.arg("--test-source").arg(source);
                }
            }
            if let Some(top) = top {
                stable.arg("--top").arg(top);
            }
            let inputs = crate::cache::fingerprint(project, &self.compiler, &self.std, &stable)?;
            Some((inputs, stable))
        } else {
            None
        };
        if let (Some(output), Some((inputs, _))) = (output, fingerprint.as_ref()) {
            if let Some((stdout, stderr)) = crate::cache::lookup(output, inputs)? {
                std::io::stderr().write_all(stderr.as_bytes())?;
                eprintln!("Fresh {}", output.display());
                return Ok(stdout);
            }
        }
        let result = command.output()?;
        std::io::stderr().write_all(&result.stderr)?;
        if !result.status.success() {
            return Err(format!("sioxc failed for {} ({})", entry.display(), result.status).into());
        }
        let stdout = String::from_utf8(result.stdout)?;
        if let (Some(output), Some((inputs, stable))) = (output, fingerprint) {
            if inputs != crate::cache::fingerprint(project, &self.compiler, &self.std, &stable)? {
                return Err(
                    "declared inputs changed during compilation; retry before caching the artifact"
                        .into(),
                );
            }
            crate::cache::record(
                output,
                inputs,
                stdout.clone(),
                String::from_utf8(result.stderr)?,
            )?;
        }
        Ok(stdout)
    }

    pub fn check(&self, project: &Project) -> Result<()> {
        let mut checked = BTreeSet::new();
        for package in &project.dies {
            for source in &package.sources {
                if checked.insert(source) {
                    self.compile(project, source, "metadata", None, None, None)?;
                }
            }
        }
        for entry in test_targets(project)?.into_keys() {
            self.compile(project, &entry, "metadata", None, None, None)?;
        }
        Ok(())
    }

    pub fn build(&self, project: &Project, top: Option<&str>) -> Result<PathBuf> {
        let package = root(project)?;
        let manifest = Manifest::read(&package.manifest_path)?;
        self.check(project)?;
        let directory = artifact_directory(project, self.debug)?;
        let output = directory.join(format!("{}.o", package.name));
        self.compile(
            project,
            &package.entry,
            "object",
            Some(&output),
            None,
            top.or(manifest.lib.top.as_deref()),
        )?;
        eprintln!("Built {}", output.display());
        Ok(output)
    }

    pub fn test(
        &self,
        project: &Project,
        filter: Option<&str>,
        list: bool,
        wave: Option<Wave>,
    ) -> Result<()> {
        let mut targets = 0;
        let directory = artifact_directory(project, self.debug)?.join("tests");
        std::fs::create_dir_all(&directory)?;
        for (entry, sources) in test_targets(project)? {
            let plan = self.compile(project, &entry, "test-plan", None, Some(&sources), None)?;
            let names: Vec<_> = plan
                .lines()
                .filter(|name| filter.is_none_or(|filter| name.contains(filter)))
                .collect();
            if list {
                for name in names {
                    println!("{name}");
                }
                continue;
            }
            if names.is_empty() {
                continue;
            }
            let id = format!(
                "{:x}",
                Sha256::digest(
                    entry
                        .to_str()
                        .ok_or("test source paths must be UTF-8")?
                        .as_bytes()
                )
            );
            let filename = format!("test-{id}");
            let output = directory
                .join(&filename)
                .with_extension(std::env::consts::EXE_EXTENSION);
            self.compile(project, &entry, "test", Some(&output), Some(&sources), None)?;
            let mut command = Command::new(&output);
            command.current_dir(
                root(project)?
                    .manifest_path
                    .parent()
                    .ok_or("manifest has no parent")?,
            );
            if let Some(filter) = filter {
                command.arg(filter);
            }
            if let Some(wave) = wave {
                let waves = crate::cache::managed_directory(
                    &project.target_directory,
                    Path::new(if self.debug {
                        "debug/tests/waves"
                    } else {
                        "release/tests/waves"
                    }),
                )?;
                if matches!(wave, Wave::Vcd | Wave::Both) {
                    let output = waves.join(&filename).with_extension("vcd");
                    reject_output_symlink(&output)?;
                    command.arg("-o").arg(output);
                }
                if matches!(wave, Wave::Fst | Wave::Both) {
                    let output = waves.join(&filename).with_extension("fst");
                    reject_output_symlink(&output)?;
                    command.arg("-o").arg(output);
                }
            }
            let result = command.stdin(Stdio::null()).status()?;
            if !result.success() {
                return Err(
                    format!("test executable {} failed ({result})", output.display()).into(),
                );
            }
            targets += 1;
        }
        if !list {
            eprintln!("Finished {targets} test targets");
        }
        Ok(())
    }
}

fn artifact_directory(project: &Project, debug: bool) -> Result<PathBuf> {
    crate::cache::managed_directory(
        &project.target_directory,
        Path::new(if debug { "debug" } else { "release" }),
    )
}

pub fn clean(target: &Path) -> Result<()> {
    crate::cache::clean(target)
}

fn reject_output_symlink(path: &Path) -> Result<()> {
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(format!("refusing output symlink {}", path.display()).into());
    }
    Ok(())
}

fn root(project: &Project) -> Result<&Package> {
    project
        .dies
        .iter()
        .find(|package| package.name == project.root)
        .ok_or_else(|| "project root die is missing".into())
}

fn test_targets(project: &Project) -> Result<BTreeMap<PathBuf, Vec<PathBuf>>> {
    let package = root(project)?;
    let manifest = Manifest::read(&package.manifest_path)?;
    let root = package
        .manifest_path
        .parent()
        .ok_or("manifest has no parent")?;
    let mut targets = BTreeMap::new();
    if manifest.lib.test {
        targets.insert(package.entry.clone(), package.sources.clone());
    }
    for target in &manifest.tests {
        let path = manifest::source_path(root, &target.path)?;
        targets.insert(path.clone(), vec![path]);
    }
    let tests = root.join("tests");
    if std::fs::symlink_metadata(&tests).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err("test discovery does not follow symlinks".into());
    }
    if tests.is_dir() {
        discover_tests(root, &tests, &mut targets)?;
    }
    Ok(targets)
}

fn discover_tests(
    root: &Path,
    directory: &Path,
    targets: &mut BTreeMap<PathBuf, Vec<PathBuf>>,
) -> Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err("test discovery does not follow symlinks".into());
        }
        if kind.is_dir() {
            discover_tests(root, &entry.path(), targets)?;
        } else if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "siox")
        {
            let path = manifest::source_path(root, entry.path().strip_prefix(root)?)?;
            targets.insert(path.clone(), vec![path]);
        }
    }
    Ok(())
}

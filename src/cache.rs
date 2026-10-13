//! Explicit, content-based artifact reuse; never cache runtime test results.

use crate::{manifest::Manifest, project::Project, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
struct Stamp {
    version: u32,
    inputs: String,
    output: String,
    stdout: String,
    stderr: String,
}

pub(crate) fn executable(path: &Path) -> Result<PathBuf> {
    if path.components().count() > 1 || path.is_absolute() {
        return Ok(path.canonicalize()?);
    }
    let name = path.as_os_str();
    for directory in std::env::split_paths(&std::env::var_os("PATH").ok_or("PATH is unavailable")?)
    {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return Ok(candidate.canonicalize()?);
        }
        if cfg!(windows) {
            let candidate = candidate.with_extension("exe");
            if candidate.is_file() {
                return Ok(candidate.canonicalize()?);
            }
        }
    }
    Err(format!("cannot find executable {} on PATH", path.display()).into())
}

fn bytes(hash: &mut Sha256, data: &[u8]) {
    hash.update((data.len() as u64).to_le_bytes());
    hash.update(data);
}

fn os(hash: &mut Sha256, value: &OsStr) {
    bytes(hash, value.as_encoded_bytes());
}

pub(crate) fn file_hash(path: &Path) -> Result<String> {
    let mut hash = Sha256::new();
    let mut file = File::open(path)?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn tree(path: &Path, excluded: &BTreeSet<PathBuf>, files: &mut BTreeSet<PathBuf>) -> Result<()> {
    if excluded.contains(path) {
        return Ok(());
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "cache input {} is a symlink; use --no-cache or declare its resolved input",
            path.display()
        )
        .into());
    }
    if metadata.is_file() {
        files.insert(path.to_path_buf());
    } else if metadata.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if matches!(
                entry.file_name().to_str(),
                Some(".git" | ".wafer" | "target")
            ) {
                continue;
            }
            tree(&entry.path(), excluded, files)?;
        }
    }
    Ok(())
}

pub(crate) fn fingerprint(
    project: &Project,
    compiler: &Path,
    std: &Path,
    command: &std::process::Command,
) -> Result<String> {
    let mut hash = Sha256::new();
    bytes(&mut hash, env!("CARGO_PKG_VERSION").as_bytes());
    bytes(&mut hash, b"wafer-cache-v1");
    os(&mut hash, command.get_program());
    for argument in command.get_args() {
        os(&mut hash, argument);
    }
    if let Some(directory) = command.get_current_dir() {
        os(&mut hash, directory.as_os_str());
    }
    let mut environment = std::env::vars_os().collect::<Vec<_>>();
    environment.sort();
    for (key, value) in environment {
        os(&mut hash, &key);
        os(&mut hash, &value);
    }
    let mut files = BTreeSet::new();
    let excluded = BTreeSet::from([project.target_directory.clone()]);
    for die in &project.dies {
        let root = die.manifest_path.parent().ok_or("manifest has no parent")?;
        tree(root, &excluded, &mut files)?;
        files.extend(die.sources.iter().cloned());
        let manifest = Manifest::read(&die.manifest_path)?;
        for input in manifest.build.inputs {
            tree(&root.join(input).canonicalize()?, &excluded, &mut files)?;
        }
    }
    tree(std, &excluded, &mut files)?;
    files.insert(executable(compiler)?);
    if command.get_args().any(|argument| argument == "--test") {
        files.insert(executable(Path::new("clang"))?);
    }
    for path in files {
        os(&mut hash, path.as_os_str());
        bytes(&mut hash, file_hash(&path)?.as_bytes());
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn stamp_path(output: &Path) -> PathBuf {
    let mut path = output.as_os_str().to_os_string();
    path.push(".wafer-cache.json");
    PathBuf::from(path)
}

pub(crate) fn lookup(output: &Path, inputs: &str) -> Result<Option<(String, String)>> {
    let stamp = match std::fs::read(stamp_path(output)) {
        Ok(bytes) => match serde_json::from_slice::<Stamp>(&bytes) {
            Ok(stamp) => stamp,
            Err(_) => return Ok(None),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if stamp.version != 1 || stamp.inputs != inputs || !output.is_file() {
        return Ok(None);
    }
    if file_hash(output)? != stamp.output {
        return Ok(None);
    }
    Ok(Some((stamp.stdout, stamp.stderr)))
}

pub(crate) fn record(output: &Path, inputs: String, stdout: String, stderr: String) -> Result<()> {
    let stamp = Stamp {
        version: 1,
        inputs,
        output: file_hash(output)?,
        stdout,
        stderr,
    };
    let path = stamp_path(output);
    let temporary = path.with_extension(format!("{}.new", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec(&stamp)?)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temporary, path)?;
    Ok(())
}

pub(crate) fn managed_directory(target: &Path, relative: &Path) -> Result<PathBuf> {
    if !relative.is_relative()
        || relative
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err("invalid managed artifact path".into());
    }
    let mut path = target.to_path_buf();
    let components = std::iter::once(PathBuf::from("wafer")).chain(
        relative
            .components()
            .map(|part| PathBuf::from(part.as_os_str())),
    );
    for component in std::iter::once(None).chain(components.map(Some)) {
        if let Some(component) = component {
            path.push(component);
        }
        if std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(format!("refusing managed artifact symlink {}", path.display()).into());
        }
        std::fs::create_dir_all(&path)?;
    }
    Ok(path)
}

pub fn clean(target: &Path) -> Result<()> {
    let managed = target.join("wafer");
    for path in [target, managed.as_path()] {
        if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(format!("refusing to clean artifact symlink {}", path.display()).into());
        }
    }
    if managed.exists() {
        std::fs::remove_dir_all(&managed)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;

    #[test]
    fn fingerprints_track_extra_inputs_std_and_compiler_contents() {
        let root = std::env::temp_dir().join(format!("wafer_fingerprint_{}", std::process::id()));
        std::fs::create_dir_all(root.join("app/src")).unwrap();
        std::fs::create_dir_all(root.join("std")).unwrap();
        std::fs::write(
            root.join("app/Die.toml"),
            "[die]\nname = \"app\"\nversion = \"0.1.0\"\n[build]\ninputs = [\"../external.dat\"]\n",
        )
        .unwrap();
        std::fs::write(root.join("app/src/lib.siox"), "module app;").unwrap();
        std::fs::write(root.join("std/prelude.siox"), "first std").unwrap();
        std::fs::write(root.join("external.dat"), "first input").unwrap();
        std::fs::write(root.join("compiler"), "identity bytes only; not executed").unwrap();
        let project = Project::load(&root.join("app/Die.toml")).unwrap();
        let compiler = root.join("compiler");
        let command = std::process::Command::new(&compiler);
        let first = fingerprint(&project, &compiler, &root.join("std"), &command).unwrap();
        assert_eq!(
            first,
            fingerprint(&project, &compiler, &root.join("std"), &command).unwrap()
        );
        std::fs::write(root.join("external.dat"), "other input").unwrap();
        let external = fingerprint(&project, &compiler, &root.join("std"), &command).unwrap();
        assert_ne!(first, external);
        std::fs::write(root.join("std/prelude.siox"), "other std").unwrap();
        let std = fingerprint(&project, &compiler, &root.join("std"), &command).unwrap();
        assert_ne!(external, std);
        std::fs::write(&compiler, "other compiler bytes").unwrap();
        assert_ne!(
            std,
            fingerprint(&project, &compiler, &root.join("std"), &command).unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

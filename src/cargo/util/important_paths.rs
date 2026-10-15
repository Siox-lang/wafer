use crate::util::errors::CargoResult;
// Modified for Wafer: reuse ancestor discovery for the Siox manifest filename.
use cargo_util::paths;
use std::path::{Path, PathBuf};

/// Finds the root `Cargo.toml`.
pub fn find_root_manifest_for_wd(cwd: &Path) -> CargoResult<PathBuf> {
    find_root_manifest_named(cwd, "Cargo.toml")
}

pub fn find_root_manifest_named(cwd: &Path, name: &str) -> CargoResult<PathBuf> {
    let valid_cargo_toml_file_name = name;
    let invalid_cargo_toml_file_name = name.to_ascii_lowercase();
    let mut invalid_cargo_toml_path_exists = false;

    for current in paths::ancestors(cwd, None) {
        let manifest = current.join(valid_cargo_toml_file_name);
        if manifest.exists() {
            return Ok(manifest);
        }
        if current.join(&invalid_cargo_toml_file_name).exists() {
            invalid_cargo_toml_path_exists = true;
        }
    }

    if invalid_cargo_toml_path_exists {
        anyhow::bail!(
            "could not find `{}` in `{}` or any parent directory, but found {} please try to rename it to {}",
            valid_cargo_toml_file_name,
            cwd.display(),
            invalid_cargo_toml_file_name,
            name
        )
    } else {
        anyhow::bail!(
            "could not find `{}` in `{}` or any parent directory",
            valid_cargo_toml_file_name,
            cwd.display()
        )
    }
}

/// Returns the path to the `file` in `pwd`, if it exists.
pub fn find_project_manifest_exact(pwd: &Path, file: &str) -> CargoResult<PathBuf> {
    let manifest = pwd.join(file);

    if manifest.exists() {
        Ok(manifest)
    } else {
        anyhow::bail!("Could not find `{}` in `{}`", file, pwd.display())
    }
}

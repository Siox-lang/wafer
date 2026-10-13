use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{
    git::{self, Origin},
    manifest::{self, Manifest},
    Result,
};

pub use crate::git::GitSource;

#[derive(Debug, Serialize)]
pub struct Project {
    pub format_version: u32,
    pub root: String,
    pub dies: Vec<Package>,
    pub modules: BTreeMap<String, PathBuf>,
    pub target_directory: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct Package {
    pub name: String,
    pub version: Version,
    pub manifest_path: PathBuf,
    pub entry: PathBuf,
    pub sources: Vec<PathBuf>,
    pub dependencies: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<GitSource>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Lockfile {
    version: u32,
    die: Vec<LockedDie>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LockedDie {
    name: String,
    version: Version,
    dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<GitSource>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ResolveOptions {
    pub offline: bool,
    pub locked: bool,
    pub update: bool,
}

struct Resolution {
    cache: PathBuf,
    pins: BTreeMap<String, GitSource>,
    options: ResolveOptions,
    visiting: BTreeSet<PathBuf>,
    visited: BTreeMap<PathBuf, usize>,
}

pub fn discover(start: &Path) -> Result<PathBuf> {
    let mut current = if start.is_file() {
        start.parent().unwrap_or(start)
    } else {
        start
    }
    .canonicalize()?;
    loop {
        let candidate = current.join("Die.toml");
        if candidate.is_file() {
            return Ok(candidate);
        }
        if !current.pop() {
            return Err("cannot find Die.toml in this directory or its parents".into());
        }
    }
}

pub fn project_lock(root: &Path) -> Result<File> {
    let dir = root.join(".wafer");
    std::fs::create_dir_all(&dir)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("project.lock"))?;
    file.try_lock()
        .map_err(|error| format!("cannot lock project {}: {error}", root.display()))?;
    Ok(file)
}

impl Project {
    pub fn load(path: &Path) -> Result<Self> {
        Self::load_with_options(path, ResolveOptions::default())
    }

    pub fn load_with_options(path: &Path, options: ResolveOptions) -> Result<Self> {
        if options.locked && options.update {
            return Err("update cannot change --locked resolution".into());
        }
        if options.offline && options.update {
            return Err("update requires access to Git remotes; omit --offline".into());
        }
        let path = path.canonicalize()?;
        let lock_path = path
            .parent()
            .ok_or("manifest has no parent")?
            .join("Wafer.lock");
        let pins = if lock_path.exists() {
            let lock: Lockfile = toml::from_str(&std::fs::read_to_string(&lock_path)?)?;
            if lock.version != 1 {
                return Err("unsupported Wafer.lock format version".into());
            }
            lock.die
                .into_iter()
                .filter_map(|die| die.source.map(|source| (die.name, source)))
                .collect()
        } else {
            if options.locked {
                return Err("Wafer.lock is missing; rerun without --locked".into());
            }
            BTreeMap::new()
        };
        let root = Manifest::read(&path)?.die.name;
        let target_directory = path
            .parent()
            .ok_or("manifest has no parent")?
            .join("target");
        let mut project = Self {
            format_version: 1,
            root,
            dies: Vec::new(),
            modules: BTreeMap::new(),
            target_directory,
        };
        let mut resolution = Resolution {
            cache: path
                .parent()
                .ok_or("manifest has no parent")?
                .join(".wafer/git"),
            pins,
            options,
            visiting: BTreeSet::new(),
            visited: BTreeMap::new(),
        };
        project.visit(&path, None, &mut resolution)?;
        Ok(project)
    }

    fn visit(
        &mut self,
        path: &Path,
        origin: Option<Origin>,
        resolution: &mut Resolution,
    ) -> Result<usize> {
        let path = path.canonicalize()?;
        if resolution.visiting.contains(&path) {
            return Err(format!("dependency cycle at {}", path.display()).into());
        }
        if let Some(index) = resolution.visited.get(&path) {
            return Ok(*index);
        }
        resolution.visiting.insert(path.clone());
        if let Some(origin) = &origin {
            git::verify_source(origin, &path)?;
        }
        let manifest = Manifest::read(&path)?;
        let root = path.parent().ok_or("manifest has no parent")?;
        let mut dependencies = Vec::new();
        for (name, dependency) in &manifest.dependencies {
            let (dependency_path, dependency_origin) = if let Some(relative) = &dependency.path {
                let path = root.join(relative).join("Die.toml").canonicalize()?;
                let mut inherited = origin.clone();
                if let Some(inherited) = &mut inherited {
                    if !path.starts_with(&inherited.checkout) {
                        return Err("a Git die path dependency escapes its locked checkout".into());
                    }
                    inherited.source.subdir = path
                        .parent()
                        .ok_or("manifest has no parent")?
                        .strip_prefix(&inherited.checkout)?
                        .to_path_buf();
                }
                (path, inherited)
            } else {
                let origin = git::obtain(
                    &resolution.cache,
                    dependency,
                    resolution.pins.get(name),
                    resolution.options,
                )?;
                (
                    origin.checkout.join(&origin.source.subdir).join("Die.toml"),
                    Some(origin),
                )
            };
            let index = self.visit(&dependency_path, dependency_origin, resolution)?;
            let found = &self.dies[index];
            if &found.name != name {
                return Err(format!(
                    "dependency `{name}` declares die `{}`; package renaming is not supported yet",
                    found.name
                )
                .into());
            }
            if dependency
                .version
                .as_ref()
                .is_some_and(|requirement| !requirement.matches(&found.version))
            {
                return Err(format!(
                    "dependency `{name}` version {} does not satisfy {}",
                    found.version,
                    dependency.version.as_ref().unwrap()
                )
                .into());
            }
            dependencies.push(name.clone());
        }
        if self.dies.iter().any(|die| die.name == manifest.die.name) {
            return Err(format!(
                "multiple sources/versions for die `{}` need compiler namespace isolation",
                manifest.die.name
            )
            .into());
        }
        let entry = manifest::source_path(root, &manifest.lib.path)?;
        if let Some(origin) = &origin {
            git::verify_source(origin, &entry)?;
        }
        let namespace = manifest::namespace(&manifest.die.name)?;
        let mut sources = vec![entry.clone()];
        if self
            .modules
            .insert(namespace.clone(), entry.clone())
            .is_some()
        {
            return Err(format!("colliding die namespace `{namespace}`").into());
        }
        for (name, relative) in &manifest.modules {
            let source = manifest::source_path(root, relative)?;
            if let Some(origin) = &origin {
                git::verify_source(origin, &source)?;
            }
            sources.push(source.clone());
            if self.modules.insert(name.clone(), source).is_some() {
                return Err(format!("duplicate source module `{name}`").into());
            }
        }
        let index = self.dies.len();
        self.dies.push(Package {
            name: manifest.die.name,
            version: manifest.die.version,
            manifest_path: path.clone(),
            entry,
            sources,
            dependencies,
            source: origin.map(|origin| origin.source),
        });
        resolution.visiting.remove(&path);
        resolution.visited.insert(path, index);
        Ok(index)
    }

    pub fn write_lock(&self, path: &Path, locked: bool) -> Result<()> {
        let mut die = self
            .dies
            .iter()
            .map(|package| LockedDie {
                name: package.name.clone(),
                version: package.version.clone(),
                dependencies: package.dependencies.clone(),
                source: package.source.clone(),
            })
            .collect::<Vec<_>>();
        die.sort_by(|a, b| a.name.cmp(&b.name));
        let desired = Lockfile { version: 1, die };
        if path.exists() {
            let existing: Lockfile = toml::from_str(&std::fs::read_to_string(path)?)?;
            if existing == desired {
                return Ok(());
            }
        }
        if locked {
            return Err("Wafer.lock needs updating; rerun without --locked".into());
        }
        use std::io::Write;
        let temporary = path.with_extension(format!("lock.{}.new", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(toml::to_string_pretty(&desired)?.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)?;
        Ok(())
    }
}

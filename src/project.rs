use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{
    manifest::{self, Manifest},
    Result,
};

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
    pub dependencies: Vec<String>,
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
        let path = path.canonicalize()?;
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
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeMap::new();
        project.visit(&path, &mut visiting, &mut visited)?;
        Ok(project)
    }

    fn visit(
        &mut self,
        path: &Path,
        visiting: &mut BTreeSet<PathBuf>,
        visited: &mut BTreeMap<PathBuf, usize>,
    ) -> Result<usize> {
        let path = path.canonicalize()?;
        if visiting.contains(&path) {
            return Err(format!("dependency cycle at {}", path.display()).into());
        }
        if let Some(index) = visited.get(&path) {
            return Ok(*index);
        }
        visiting.insert(path.clone());
        let manifest = Manifest::read(&path)?;
        let root = path.parent().ok_or("manifest has no parent")?;
        let mut dependencies = Vec::new();
        for (name, dependency) in &manifest.dependencies {
            let index = self.visit(
                &root.join(&dependency.path).join("Die.toml"),
                visiting,
                visited,
            )?;
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
        let namespace = manifest::namespace(&manifest.die.name)?;
        if self
            .modules
            .insert(namespace.clone(), entry.clone())
            .is_some()
        {
            return Err(format!("colliding die namespace `{namespace}`").into());
        }
        for (name, relative) in &manifest.modules {
            if self
                .modules
                .insert(name.clone(), manifest::source_path(root, relative)?)
                .is_some()
            {
                return Err(format!("duplicate source module `{name}`").into());
            }
        }
        let index = self.dies.len();
        self.dies.push(Package {
            name: manifest.die.name,
            version: manifest.die.version,
            manifest_path: path.clone(),
            entry,
            dependencies,
        });
        visiting.remove(&path);
        visited.insert(path, index);
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

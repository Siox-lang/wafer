use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use siox::{
    diag::{DiagnosticSink, FileId},
    syntax::{lexer::Lexer, token::TokenKind},
};

use crate::Result;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub die: Die,
    #[serde(default)]
    pub lib: Library,
    #[serde(default)]
    pub modules: BTreeMap<String, PathBuf>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
    #[serde(default)]
    pub toolchain: Toolchain,
    #[serde(default, rename = "test")]
    pub tests: Vec<TestTarget>,
    #[serde(default)]
    pub build: Build,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Die {
    pub name: String,
    pub version: Version,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Library {
    pub path: PathBuf,
    #[serde(default)]
    pub top: Option<String>,
    #[serde(default = "enabled")]
    pub test: bool,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            path: "src/lib.siox".into(),
            top: None,
            test: true,
        }
    }
}

fn enabled() -> bool {
    true
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Toolchain {
    pub sioxc: Option<PathBuf>,
    pub std: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TestTarget {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
    /// Additional compile-time file/directory inputs outside the die roots.
    #[serde(default)]
    pub inputs: Vec<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subdir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<VersionReq>,
}

pub fn namespace(name: &str) -> Result<String> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!("invalid die name `{name}`").into());
    }
    let namespace = name.replace('-', "_");
    identifier(&namespace)?;
    if matches!(namespace.as_str(), "core" | "std") {
        return Err(format!("die name `{name}` uses the reserved `{namespace}` namespace").into());
    }
    Ok(namespace)
}

fn identifier(name: &str) -> Result<()> {
    let tokens = Lexer::new(FileId(0), name).tokenize(&mut DiagnosticSink::new());
    if tokens.len() != 2
        || tokens[0].kind != TokenKind::Ident
        || tokens[0].span.start != 0
        || tokens[0].span.end as usize != name.len()
    {
        return Err(format!("`{name}` is not a Siox identifier").into());
    }
    Ok(())
}

impl Manifest {
    pub fn read(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let manifest: Self = toml::from_str(&text)
            .map_err(|error| format!("invalid {}: {error}", path.display()))?;
        let prefix = namespace(&manifest.die.name)?;
        for module in manifest.modules.keys() {
            if !module.starts_with(&format!("{prefix}::")) {
                return Err(
                    format!("module `{module}` must be inside die namespace `{prefix}`").into(),
                );
            }
            for segment in module.split("::") {
                identifier(segment)?;
            }
        }
        for (name, dependency) in &manifest.dependencies {
            namespace(name)?;
            if dependency.path.is_some() == dependency.git.is_some() {
                return Err(format!("dependency `{name}` needs exactly one of path or git").into());
            }
            if dependency.path.is_some()
                && (dependency.rev.is_some() || dependency.subdir.is_some())
            {
                return Err(format!("dependency `{name}`: rev/subdir require git").into());
            }
            if let Some(url) = &dependency.git {
                crate::git::validate_url(url)?;
            }
            if dependency.rev.as_ref().is_some_and(|rev| {
                rev.is_empty() || rev.starts_with('-') || rev.chars().any(char::is_control)
            }) {
                return Err(format!("dependency `{name}` has an invalid Git revision").into());
            }
            if dependency.subdir.as_ref().is_some_and(|path| {
                !path.is_relative()
                    || path
                        .components()
                        .any(|part| matches!(part, std::path::Component::ParentDir))
            }) {
                return Err(format!("dependency `{name}` has an invalid Git subdirectory").into());
            }
        }
        Ok(manifest)
    }
}

pub fn source_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    if !relative.is_relative() {
        return Err("die source paths must be relative".into());
    }
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|error| format!("cannot locate {}: {error}", root.join(relative).display()))?;
    if !path.starts_with(root) || !path.is_file() {
        return Err(format!(
            "{} is not a source file inside {}",
            path.display(),
            root.display()
        )
        .into());
    }
    Ok(path)
}

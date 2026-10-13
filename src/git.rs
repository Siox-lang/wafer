//! Immutable checkout locations for lockfile commits, not a package server.

use crate::{manifest::Dependency, project::ResolveOptions, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitSource {
    pub url: String,
    pub revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested: Option<String>,
    #[serde(default)]
    pub subdir: PathBuf,
}

#[derive(Clone)]
pub(crate) struct Origin {
    pub source: GitSource,
    pub checkout: PathBuf,
}

pub(crate) fn validate_url(url: &str) -> Result<()> {
    let allowed = url.starts_with("https://")
        || url.starts_with("ssh://")
        || url.starts_with("file://")
        || Path::new(url).is_absolute()
        || (url.starts_with("git@") && url.contains(':'));
    if !allowed || url.chars().any(char::is_control) {
        return Err("Git sources need HTTPS, SSH, file:// or an absolute local path; helper transports are disabled".into());
    }
    if let Some(authority) = url
        .strip_prefix("https://")
        .map(|rest| rest.split('/').next().unwrap_or(""))
    {
        if authority.contains('@') {
            return Err("do not embed credentials in Git URLs; use Git credential helpers".into());
        }
    }
    if let Some(authority) = url
        .strip_prefix("ssh://")
        .map(|rest| rest.split('/').next().unwrap_or(""))
    {
        if authority
            .rsplit_once('@')
            .is_some_and(|(user, _)| user.contains(':'))
        {
            return Err("do not embed passwords in SSH URLs; use an SSH agent".into());
        }
    }
    Ok(())
}

fn run(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args([
            "-c",
            "protocol.ext.allow=never",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

pub(crate) fn verify_source(origin: &Origin, path: &Path) -> Result<()> {
    let relative = path
        .strip_prefix(&origin.checkout)?
        .to_str()
        .ok_or("non-UTF8 Git source path")?;
    run(
        &origin.checkout,
        &["ls-files", "--error-unmatch", "--", relative],
    )
    .map_err(|error| {
        format!(
            "Git die source {} is not tracked in its locked commit: {error}",
            path.display()
        )
    })?;
    Ok(())
}

pub(crate) fn obtain(
    cache: &Path,
    dependency: &Dependency,
    pinned: Option<&GitSource>,
    options: ResolveOptions,
) -> Result<Origin> {
    let url = dependency.git.as_ref().ok_or("missing Git URL")?;
    validate_url(url)?;
    let subdir = dependency.subdir.clone().unwrap_or_default();
    let pinned = pinned.filter(|pin| {
        pin.url == *url
            && pin.requested == dependency.rev
            && pin.subdir == subdir
            && !options.update
    });
    if options.locked && pinned.is_none() {
        return Err("Git dependency needs a new lockfile pin; rerun without --locked".into());
    }
    let id = format!("{:x}", Sha256::digest(url.as_bytes()));
    let cache = cache.join(id);
    std::fs::create_dir_all(&cache)?;
    let repository = cache.join("repository.git");
    let fresh_repository = !repository.exists();
    if fresh_repository {
        if options.offline {
            return Err("Git dependency is not cached; offline resolution cannot fetch it".into());
        }
        run(&cache, &["clone", "--mirror", "--", url, "repository.git"])?;
    }
    let revision = if let Some(pin) = pinned {
        pin.revision.clone()
    } else {
        let fetched = !fresh_repository && !options.offline;
        if fetched {
            if let Some(rev) = &dependency.rev {
                run(&repository, &["fetch", "--prune", "origin", rev])?;
            } else {
                run(&repository, &["fetch", "origin", "HEAD"])?;
            }
        }
        let selector = if fetched {
            "FETCH_HEAD"
        } else {
            dependency.rev.as_deref().unwrap_or("HEAD")
        };
        run(
            &repository,
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{selector}^{{commit}}"),
            ],
        )?
    };
    if !matches!(revision.len(), 40 | 64) || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("invalid Git commit in dependency lock".into());
    }
    let checkout = cache.join(&revision);
    if !checkout.exists() {
        if run(
            &repository,
            &["cat-file", "-e", &format!("{revision}^{{commit}}")],
        )
        .is_err()
        {
            if options.offline {
                return Err(
                    "locked Git commit is not cached and offline mode forbids fetching it".into(),
                );
            }
            run(&repository, &["fetch", "origin", &revision])?;
        }
        run(
            &repository,
            &[
                "worktree",
                "add",
                "--detach",
                "--",
                checkout.to_str().ok_or("non-UTF8 Git cache path")?,
                &revision,
            ],
        )?;
    }
    if run(&checkout, &["rev-parse", "HEAD"])? != revision
        || !run(
            &checkout,
            &["status", "--porcelain", "--untracked-files=all"],
        )?
        .is_empty()
    {
        return Err(format!(
            "dirty or mismatched cached Git checkout {}; edits were preserved",
            checkout.display()
        )
        .into());
    }
    let checkout = checkout.canonicalize()?;
    let package = checkout.join(&subdir).canonicalize()?;
    if !package.starts_with(&checkout) {
        return Err("Git die subdirectory escapes its checkout".into());
    }
    Ok(Origin {
        source: GitSource {
            url: url.clone(),
            revision,
            requested: dependency.rev.clone(),
            subdir,
        },
        checkout,
    })
}

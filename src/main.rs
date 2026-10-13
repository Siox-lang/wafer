use std::{
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use wafer::{
    manifest,
    project::{self, Project, ResolveOptions},
    runner::{Runner, Wave},
    Result,
};

#[derive(Parser)]
#[command(version, about = "Package and project tooling for Siox dies")]
struct Cli {
    #[arg(long, global = true)]
    manifest_path: Option<PathBuf>,
    #[arg(long, global = true)]
    locked: bool,
    #[arg(long, global = true)]
    offline: bool,
    #[arg(long, global = true)]
    sioxc: Option<PathBuf>,
    #[arg(long, global = true)]
    std: Option<PathBuf>,
    #[arg(long, global = true)]
    debug: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    New {
        directory: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    Init {
        #[arg(default_value = ".")]
        directory: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    Metadata,
    Fetch,
    Update,
    Check,
    Build {
        #[arg(long)]
        top: Option<String>,
    },
    Test {
        filter: Option<String>,
        #[arg(long)]
        list: bool,
        #[arg(long, value_enum)]
        wave: Option<Wave>,
    },
}

fn scaffold(directory: &Path, name: Option<String>, new: bool) -> Result<()> {
    if new && directory.exists() {
        return Err("destination exists; use wafer init".into());
    }
    let name = match name {
        Some(name) => name,
        None => directory
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
            .or_else(|| {
                directory
                    .canonicalize()
                    .ok()?
                    .file_name()?
                    .to_str()
                    .map(str::to_owned)
            })
            .ok_or("cannot infer die name; pass --name")?,
    };
    let namespace = manifest::namespace(&name)?;
    let manifest_path = directory.join("Die.toml");
    let source = directory.join("src/lib.siox");
    for path in [&manifest_path, &source] {
        if std::fs::symlink_metadata(path).is_ok() {
            return Err(format!("refusing to overwrite {}", path.display()).into());
        }
    }
    if std::fs::symlink_metadata(directory.join("src"))
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err("refusing to scaffold through a src symlink".into());
    }
    std::fs::create_dir_all(directory.join("src"))?;
    let manifest =
        format!("[die]\nname = {name:?}\nversion = \"0.1.0\"\n\n[lib]\npath = \"src/lib.siox\"\ntop = \"Top\"\n");
    std::fs::OpenOptions::new().write(true).create_new(true).open(&source)?
        .write_all(format!("module {namespace};\n\npub fn identity(value: integer) -> integer {{ return value; }}\n\npub entity Top {{ value: integer out }}\nimpl Top {{ value = identity(42); }}\n\n#[test]\nentity Smoke {{}}\nimpl Smoke {{\n    let dut: Top = {{}};\n    process {{ await 1ns; assert!(dut.value == 42, \"generated die smoke test\"); }}\n}}\n").as_bytes())?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&manifest_path)?
        .write_all(manifest.as_bytes())?;
    let ignore = directory.join(".gitignore");
    if std::fs::symlink_metadata(&ignore).is_err() {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(ignore)?
            .write_all(b"/target/\n/.wafer/\n")?;
    }
    eprintln!("Created die `{name}` at {}", directory.display());
    Ok(())
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::New { directory, name } => scaffold(&directory, name, true),
        Command::Init { directory, name } => scaffold(&directory, name, false),
        command => {
            let path = match cli.manifest_path {
                Some(path) => path.canonicalize()?,
                None => project::discover(&std::env::current_dir()?)?,
            };
            let root = path.parent().ok_or("manifest has no parent")?;
            let _lock = project::project_lock(root)?;
            let project = Project::load_with_options(
                &path,
                ResolveOptions {
                    offline: cli.offline,
                    locked: cli.locked,
                    update: matches!(command, Command::Update),
                },
            )?;
            project.write_lock(&root.join("Wafer.lock"), cli.locked)?;
            if matches!(
                command,
                Command::Check | Command::Build { .. } | Command::Test { .. }
            ) {
                let manifest = manifest::Manifest::read(&path)?;
                let runner = Runner::configured(&manifest, root, cli.sioxc, cli.std, cli.debug)?;
                return match command {
                    Command::Check => runner.check(&project),
                    Command::Build { top } => runner.build(&project, top.as_deref()).map(|_| ()),
                    Command::Test { filter, list, wave } => {
                        runner.test(&project, filter.as_deref(), list, wave)
                    }
                    _ => unreachable!(),
                };
            }
            match command {
                Command::Metadata => println!("{}", serde_json::to_string_pretty(&project)?),
                Command::Fetch | Command::Update => {
                    eprintln!("Resolved {} dies", project.dies.len())
                }
                _ => unreachable!(),
            }
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("wafer: {error}");
            ExitCode::FAILURE
        }
    }
}

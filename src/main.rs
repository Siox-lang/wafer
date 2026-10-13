use std::{
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use wafer::{
    manifest,
    project::{self, Project},
    Result,
};

#[derive(Parser)]
#[command(version, about = "Package and project tooling for Siox dies")]
struct Cli {
    #[arg(long, global = true)]
    manifest_path: Option<PathBuf>,
    #[arg(long, global = true)]
    locked: bool,
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
        format!("[die]\nname = {name:?}\nversion = \"0.1.0\"\n\n[lib]\npath = \"src/lib.siox\"\n");
    std::fs::OpenOptions::new().write(true).create_new(true).open(&source)?
        .write_all(format!("module {namespace};\n\npub fn identity(value: integer) -> integer {{ return value; }}\n").as_bytes())?;
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
            let project = Project::load(&path)?;
            project.write_lock(&root.join("Wafer.lock"), cli.locked)?;
            match command {
                Command::Metadata => println!("{}", serde_json::to_string_pretty(&project)?),
                Command::Fetch => eprintln!("Resolved {} local dies", project.dies.len()),
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

use cargo::ops::{self, OutputMetadataOptions};
// Modified for Wafer: do not expose Rust-only compiler metadata for Siox.

use crate::command_prelude::*;

pub fn cli() -> Command {
    subcommand("metadata")
        .about(
            "Output the resolved dependencies of a package, \
             the concrete used versions including overrides, \
             in machine-readable format",
        )
        .arg(multi_opt(
            "filter-platform",
            "TRIPLE",
            "Only include resolve dependencies matching the given target-triple",
        ))
        .arg(flag(
            "no-deps",
            "Output information only about the workspace members \
             and don't fetch dependencies",
        ))
        .arg(
            opt("format-version", "Format version")
                .value_name("VERSION")
                .value_parser(["1"]),
        )
        .arg_silent_suggestion()
        .arg_features()
        .arg_manifest_path()
        .arg_lockfile_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help metadata</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(gctx)?;

    let version = match args.get_one::<String>("format-version") {
        None => {
            gctx.shell().warn(
                "please specify `--format-version` flag explicitly \
                 to avoid compatibility problems",
            )?;
            1
        }
        Some(version) => version.parse().unwrap(),
    };

    let options = OutputMetadataOptions {
        cli_features: args.cli_features()?,
        no_deps: args.flag("no-deps"),
        filter_platforms: args._values_of("filter-platform"),
        version,
    };

    let result = ops::output_metadata(&ws, &options)?;
    if gctx.is_wafer() {
        let mut result = serde_json::to_value(&result).map_err(anyhow::Error::from)?;
        if let Some(packages) = result["packages"].as_array_mut() {
            for package in packages {
                if let Some(package) = package.as_object_mut() {
                    package.remove("edition");
                    package.remove("rust_version");
                    if let Some(targets) = package
                        .get_mut("targets")
                        .and_then(serde_json::Value::as_array_mut)
                    {
                        for target in targets {
                            if let Some(target) = target.as_object_mut() {
                                target.remove("edition");
                                target.remove("crate_types");
                            }
                        }
                    }
                }
            }
        }
        gctx.shell().print_json(&result)?;
    } else {
        gctx.shell().print_json(&result)?;
    }
    Ok(())
}

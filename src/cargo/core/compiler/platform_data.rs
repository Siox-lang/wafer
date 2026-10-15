//! Platform facts needed by Cargo's shared dependency/feature resolver.

use super::build_context::{Flags, extra_args};
use super::{BuildOutput, CompileKind, RustcTargetData};
use crate::core::{Dependency, Workspace};
use crate::util::{CargoResult, Sioxc, interning::InternedString};
use std::{collections::BTreeMap, rc::Rc};

pub trait PlatformData {
    fn host_triple(&self) -> InternedString;
    fn dep_platform_activated(&self, dep: &Dependency, kind: CompileKind) -> bool;
    fn merge_compile_kind(&mut self, kind: CompileKind) -> CargoResult<()>;
}

/// Compiler-owned options that participate in a build unit's identity.
#[derive(Clone, Default)]
pub struct UnitCompilerOptions {
    pub compiler_flags: Rc<[String]>,
    pub documentation_flags: Rc<[String]>,
    pub links_overrides: Rc<BTreeMap<String, BuildOutput>>,
}

/// The unit planner needs options, not Rust sysroots or crate output formats.
pub trait UnitTargetData: PlatformData {
    fn unit_options(&self, kind: CompileKind) -> UnitCompilerOptions;
    fn compiler(&self) -> CompilerRef<'_>;
}

/// The selected compiler's actual facts, not a Rust-compatible synthetic toolchain.
pub enum CompilerRef<'a> {
    Rust(&'a RustcTargetData<'a>),
    Sioxc(&'a Sioxc),
}

impl<'a> CompilerRef<'a> {
    pub fn rust_workspace_wrapper(&self) -> &'a Option<std::path::PathBuf> {
        match self {
            Self::Rust(data) => &data.rustc.workspace_wrapper,
            Self::Sioxc(_) => &None,
        }
    }

    pub fn cfg(&self, kind: CompileKind) -> &'a [cargo_platform::Cfg] {
        match self {
            Self::Rust(data) => data.cfg(kind),
            Self::Sioxc(compiler) => &compiler.host_cfg,
        }
    }
}

impl PlatformData for RustcTargetData<'_> {
    fn host_triple(&self) -> InternedString {
        self.rustc.host
    }
    fn dep_platform_activated(&self, dep: &Dependency, kind: CompileKind) -> bool {
        RustcTargetData::dep_platform_activated(self, dep, kind)
    }
    fn merge_compile_kind(&mut self, kind: CompileKind) -> CargoResult<()> {
        RustcTargetData::merge_compile_kind(self, kind)
    }
}

impl UnitTargetData for RustcTargetData<'_> {
    fn compiler(&self) -> CompilerRef<'_> {
        CompilerRef::Rust(self)
    }

    fn unit_options(&self, kind: CompileKind) -> UnitCompilerOptions {
        UnitCompilerOptions {
            compiler_flags: self.info(kind).rustflags.clone(),
            documentation_flags: self.info(kind).rustdocflags.clone(),
            links_overrides: self.target_config(kind).links_overrides.clone(),
        }
    }
}

/// Actual Sioxc execution-host facts; native cross-target codegen is not implied.
pub struct SioxcTargetData {
    compiler: Sioxc,
    host: InternedString,
    host_options: UnitCompilerOptions,
    target_options: UnitCompilerOptions,
}

impl SioxcTargetData {
    pub fn new(ws: &Workspace<'_>, requested: &[CompileKind]) -> CargoResult<Self> {
        let compiler = ws.gctx().load_sioxc()?;
        let host = compiler.host.as_str().into();
        let options = |kind| -> CargoResult<UnitCompilerOptions> {
            Ok(UnitCompilerOptions {
                compiler_flags: extra_args(
                    ws.gctx(),
                    requested,
                    &compiler.host,
                    Some(&compiler.host_cfg),
                    kind,
                    Flags::Sioxc,
                )?
                .into(),
                ..UnitCompilerOptions::default()
            })
        };
        let target_options = options(CompileKind::Target(super::CompileTarget::new(
            &compiler.host,
        )?))?;
        let host_options =
            if requested == [CompileKind::Host] && ws.gctx().target_applies_to_host()? {
                // Cargo canonicalizes native target units to host units without copying their options.
                target_options.clone()
            } else {
                options(CompileKind::Host)?
            };
        let mut data = Self {
            compiler,
            host,
            host_options,
            target_options,
        };
        for kind in requested
            .iter()
            .copied()
            .chain(ws.members().flat_map(|package| {
                package
                    .manifest()
                    .default_kind()
                    .into_iter()
                    .chain(package.manifest().forced_kind())
            }))
        {
            data.merge_compile_kind(kind)?;
        }
        Ok(data)
    }
}

impl UnitTargetData for SioxcTargetData {
    fn compiler(&self) -> CompilerRef<'_> {
        CompilerRef::Sioxc(&self.compiler)
    }

    fn unit_options(&self, kind: CompileKind) -> UnitCompilerOptions {
        match kind {
            CompileKind::Host => self.host_options.clone(),
            CompileKind::Target(_) => self.target_options.clone(),
        }
    }
}

impl PlatformData for SioxcTargetData {
    fn host_triple(&self) -> InternedString {
        self.host
    }
    fn dep_platform_activated(&self, dep: &Dependency, kind: CompileKind) -> bool {
        let name = match &kind {
            CompileKind::Host => self.host.as_str(),
            CompileKind::Target(target) => target.short_name(),
        };
        dep.platform()
            .is_none_or(|platform| platform.matches(name, &self.compiler.host_cfg))
    }
    fn merge_compile_kind(&mut self, kind: CompileKind) -> CargoResult<()> {
        if let CompileKind::Target(target) = kind {
            let name = target.rustc_target();
            if name != self.host.as_str() {
                anyhow::bail!(
                    "Sioxc only provides native execution-host platform facts for {}; cross-target platform {name} is not supported",
                    self.host
                );
            }
        }
        Ok(())
    }
}

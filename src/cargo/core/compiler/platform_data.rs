//! Platform facts needed by Cargo's shared dependency/feature resolver.

use super::{CompileKind, RustcTargetData};
use crate::core::{Dependency, Workspace};
use crate::util::{CargoResult, Sioxc, interning::InternedString};

pub trait PlatformData {
    fn host_triple(&self) -> InternedString;
    fn dep_platform_activated(&self, dep: &Dependency, kind: CompileKind) -> bool;
    fn merge_compile_kind(&mut self, kind: CompileKind) -> CargoResult<()>;
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

/// Actual Sioxc execution-host facts; native cross-target codegen is not implied.
pub struct SioxcTargetData {
    compiler: Sioxc,
    host: InternedString,
}

impl SioxcTargetData {
    pub fn new(ws: &Workspace<'_>, requested: &[CompileKind]) -> CargoResult<Self> {
        let compiler = ws.gctx().load_sioxc()?;
        let host = compiler.host.as_str().into();
        let mut data = Self { compiler, host };
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

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct BuildProvenance {
    pub package_version: &'static str,
    pub git_revision: &'static str,
    pub source_dirty: Option<bool>,
    pub rustc: &'static str,
    pub target: &'static str,
    pub profile: &'static str,
    pub optimization_level: &'static str,
}

impl BuildProvenance {
    pub fn current() -> Self {
        Self {
            package_version: env!("CARGO_PKG_VERSION"),
            git_revision: env!("MONOPOLEZ_REVISION"),
            source_dirty: env!("MONOPOLEZ_DIRTY").parse().ok(),
            rustc: env!("MONOPOLEZ_COMPILER"),
            target: env!("MONOPOLEZ_TARGET"),
            profile: env!("MONOPOLEZ_PROFILE"),
            optimization_level: env!("MONOPOLEZ_OPT_LEVEL"),
        }
    }
}

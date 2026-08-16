//! Build identity embedded into every generated task's runtime provenance.

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD_COMMIT: &str = env!("AI_STUDIO_BUILD_COMMIT");

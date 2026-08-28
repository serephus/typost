//! The embedded Typst stdlib.
//!
//! The canonical sources live in `typost-core/typst/` and are embedded at
//! compile time. They are materialized into a project's `lib/` directory so
//! that entries can import them by relative path — which keeps plain
//! `typst compile`, Tinymist, and offline builds working.

use std::path::Path;

use anyhow::{Context, Result};

/// The core stdlib, embedded at compile time.
pub const STDLIB_TYP: &str = include_str!("../typst/typost.typ");

/// The files that make up the stdlib, as `(project-relative path, source)`.
pub const STDLIB_FILES: &[(&str, &str)] = &[("lib/typost.typ", STDLIB_TYP)];

/// Add the embedded stdlib to an overlay.
pub fn add_to(overlay: &mut crate::plugin::TypstOverlay) {
    for (path, source) in STDLIB_FILES {
        overlay.add(path, *source);
    }
}

/// Write the embedded stdlib into `root`, creating parent directories.
///
/// The `lib/` directory is generated: files are overwritten on every build so
/// they always match the `typost-core` version in use.
pub fn materialize(root: &Path) -> Result<()> {
    for (relative, source) in STDLIB_FILES {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create `{}`", parent.display()))?;
        }
        std::fs::write(&path, source)
            .with_context(|| format!("failed to write `{}`", path.display()))?;
    }
    Ok(())
}

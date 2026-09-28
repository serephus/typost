//! The embedded Typst stdlib.
//!
//! The canonical sources live in `typost-core/typst/` and are embedded at
//! compile time. They are materialized into a project's `lib/` directory so
//! that entries can import them by relative path — which keeps plain
//! `typst compile`, Tinymist, and offline builds working.

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

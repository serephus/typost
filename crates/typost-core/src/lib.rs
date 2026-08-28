//! Core engine for typost.
//!
//! A typost site is a Typst program compiled in Typst's `bundle` target. This
//! crate supplies only the machinery: a [`World`](typst::World) backed by the
//! project directory, a typed [`SiteManifest`] lifted from the evaluated Typst
//! program, the [`Plugin`] trait, and the build pipeline. Everything optional
//! lives in plugins.

pub mod manifest;
pub mod plugin;
pub mod site;
pub mod stdlib;
pub mod world;

pub use manifest::{FrontMatter, MetadataEntry, PageMeta, SiteManifest};
pub use plugin::{Plugin, RenderOutput, TypstOverlay};
pub use site::{BuildOptions, Site, build};
pub use stdlib::{STDLIB_FILES, STDLIB_TYP, materialize as materialize_stdlib};
pub use world::TypostWorld;

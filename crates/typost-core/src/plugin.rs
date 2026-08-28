//! The plugin interface.
//!
//! Plugins are composed at compile time. A plugin may contribute Typst sources
//! to the [`World`](typst::World), mutate the [`SiteManifest`] before render,
//! and transform exported bytes after render.

use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use typst::foundations::Bytes;
use typst::syntax::VirtualPath;

use crate::manifest::SiteManifest;

/// Extra Typst sources injected into the [`World`](typst::World) before
/// evaluation, keyed by virtual path (e.g. `lib/tags.typ`).
#[derive(Debug, Default)]
pub struct TypstOverlay {
    /// The overlay files.
    pub files: HashMap<VirtualPath, Bytes>,
}

impl TypstOverlay {
    /// Create an empty overlay.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a Typst source file at the given project-relative path.
    pub fn add(&mut self, path: &str, source: impl Into<String>) {
        let vpath = VirtualPath::new(path).expect("overlay path must be valid");
        self.files.insert(vpath, Bytes::from_string(source.into()));
    }
}

/// The result of rendering: output files, keyed by path relative to the site
/// root (no leading slash).
#[derive(Debug, Default)]
pub struct RenderOutput {
    /// Path → bytes.
    pub files: BTreeMap<String, Vec<u8>>,
}

impl RenderOutput {
    /// Create an empty output.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a file.
    pub fn insert(&mut self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) {
        self.files.insert(path.into(), bytes.into());
    }

    /// Get a file's bytes.
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }
}

/// A compile-time plugin.
pub trait Plugin: Send + Sync {
    /// The plugin's name, for diagnostics.
    fn name(&self) -> &str;

    /// Contribute Typst modules or data into the World before evaluation.
    fn typst(&self, _overlay: &mut TypstOverlay) -> Result<()> {
        Ok(())
    }

    /// Mutate the site structure after evaluation, before rendering.
    fn prepare(&self, _manifest: &mut SiteManifest) -> Result<()> {
        Ok(())
    }

    /// Transform exported files after rendering.
    fn post(&self, _out: &mut RenderOutput, _manifest: &SiteManifest) -> Result<()> {
        Ok(())
    }
}

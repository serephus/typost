//! Multi-taxonomy support (tags, categories, difficulty, ...).
//!
//! A site may want more than one kind of tag: `tags`, `categories`,
//! `difficulty`, and so on. Each page declares its values in front matter under
//! the matching key; the site declares which keys are taxonomies and the plugin
//! contributes a Typst module that emits an index and a term page for each of
//! them.
//!
//! Unlike most plugins this one is almost entirely Typst. Routes must exist
//! before the single bundle render, and only Typst can lay a page out, so the
//! generated pages are produced by the contributed `lib/typost/taxonomies.typ`
//! at evaluation time.

use anyhow::Result;
use typost_core::{Plugin, TypstOverlay};

/// Contributes the multi-taxonomy Typst module.
pub struct Taxonomies;

impl Taxonomies {
    /// Create the plugin.
    pub fn new() -> Self {
        Self
    }
}

impl Default for Taxonomies {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for Taxonomies {
    fn name(&self) -> &str {
        "taxonomies"
    }

    fn typst(&self, overlay: &mut TypstOverlay) -> Result<()> {
        overlay.add("lib/typost/taxonomies.typ", TAXONOMIES_TYP)?;
        Ok(())
    }
}

const TAXONOMIES_TYP: &str = include_str!("../typst/taxonomies.typ");

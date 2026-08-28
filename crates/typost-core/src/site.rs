//! The build pipeline.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use comemo::Track;
use typst::World;
use typst::diag::SourceDiagnostic;
use typst::engine::{Route, Sink, Traced};
use typst::foundations::{Content, Packed, Value};
use typst::introspection::MetadataElem;
use typst::model::DocumentElem;
use typst_bundle::{Bundle, BundleOptions, export};

use crate::manifest::{FrontMatter, MetadataEntry, PageMeta, SiteManifest};
use crate::plugin::{Plugin, RenderOutput, TypstOverlay};
use crate::world::TypostWorld;

/// Inputs for a build.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// The project root.
    pub root: PathBuf,
    /// The entry file, relative to the root.
    pub entry: String,
    /// The output directory.
    pub out: PathBuf,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            entry: "home.typ".to_owned(),
            out: PathBuf::from("dist"),
        }
    }
}

/// A configured site: build options plus a compile-time plugin list.
///
/// Plugins run in registration order, at each of their three seams.
#[derive(Default)]
pub struct Site {
    options: BuildOptions,
    plugins: Vec<Box<dyn Plugin>>,
}

impl Site {
    /// Create a site with the given build options and no plugins.
    pub fn new(options: BuildOptions) -> Self {
        Self {
            options,
            plugins: Vec::new(),
        }
    }

    /// Register a plugin. Plugins run in registration order.
    pub fn plugin(mut self, plugin: impl Plugin + 'static) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Run the full pipeline.
    pub fn build(&self) -> Result<()> {
        build(&self.options, &self.plugins)
    }
}

/// Run the full pipeline.
///
/// `Config → typst → eval-only → prepare → render → post → emit`
pub fn build(options: &BuildOptions, plugins: &[Box<dyn Plugin>]) -> Result<()> {
    // 1. Let plugins contribute Typst sources to the World (after the stdlib,
    //    so plugins can override it if they need to).
    let mut overlay = TypstOverlay::new();
    crate::stdlib::add_to(&mut overlay);
    for plugin in plugins {
        plugin
            .typst(&mut overlay)
            .with_context(|| format!("plugin `{}` failed in its `typst` stage", plugin.name()))?;
    }

    // 2. Materialize the stdlib (and plugin Typst modules) into `lib/` so that
    //    plain `typst` and Tinymist can resolve them.
    materialize_overlay(&options.root, &overlay)?;

    let world = TypostWorld::new(options.root.clone(), &options.entry, overlay);

    // 3. Eval-only pass: lift the typed manifest out of the Typst program.
    let mut manifest = extract_manifest(&world)
        .with_context(|| format!("failed to evaluate entry `{}`", options.entry))?;

    // 4. Let plugins mutate the structure before render.
    for plugin in plugins {
        plugin
            .prepare(&mut manifest)
            .with_context(|| format!("plugin `{}` failed in its `prepare` stage", plugin.name()))?;
    }

    // 5. Render the bundle and export it to files.
    let mut output = render(&world)?;

    // 6. Let plugins transform exported files.
    for plugin in plugins {
        plugin
            .post(&mut output, &manifest)
            .with_context(|| format!("plugin `{}` failed in its `post` stage", plugin.name()))?;
    }

    // 7. Write everything to disk.
    emit(&output, &options.out)?;
    Ok(())
}

/// Write plugin-contributed Typst sources to disk so editors can resolve them.
fn materialize_overlay(root: &Path, overlay: &TypstOverlay) -> Result<()> {
    for (vpath, bytes) in &overlay.files {
        let path = root.join(vpath.get_without_slash());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create `{}`", parent.display()))?;
        }
        std::fs::write(&path, &bytes[..])
            .with_context(|| format!("failed to write `{}`", path.display()))?;
    }
    Ok(())
}

/// Evaluate the entry without layout and collect the typed manifest.
fn extract_manifest(world: &TypostWorld) -> Result<SiteManifest> {
    let main = world.main();
    let source = world
        .source(main)
        .map_err(|err| anyhow!("failed to load entry source: {err}"))?;

    let mut sink = Sink::new();
    let world_dyn: &dyn World = world;
    let module = typst_eval::eval(
        world_dyn.track(),
        world_dyn.library(),
        Traced::default().track(),
        sink.track_mut(),
        Route::default().track(),
        &source,
    )
    .map_err(|diags| anyhow!("evaluation failed:\n{}", format_diagnostics(&diags)))?;

    let content = module.content();
    let mut manifest = SiteManifest::default();
    walk(&content, None, &mut manifest);

    let warnings = sink.warnings();
    if !warnings.is_empty() {
        eprintln!("warning: {}", format_diagnostics(&warnings));
    }

    Ok(manifest)
}

/// Walk evaluated content, collecting `typost` metadata. Entries inside a
/// `document` are tagged with that document's route.
fn walk(content: &Content, route: Option<&str>, manifest: &mut SiteManifest) {
    if let Some(metadata) = content.to_packed::<MetadataElem>() {
        parse_metadata(metadata, route, manifest);
    }

    if let Some(document) = content.to_packed::<DocumentElem>() {
        let doc_route = document.path.as_ref().get_without_slash().to_owned();
        walk(&document.body, Some(&doc_route), manifest);
    } else {
        for (_, value) in content.fields() {
            walk_value(value, route, manifest);
        }
    }
}

/// Walk a field value (content or an array of them).
fn walk_value(value: Value, route: Option<&str>, manifest: &mut SiteManifest) {
    match value {
        Value::Content(content) => walk(&content, route, manifest),
        Value::Array(array) => {
            for value in array {
                walk_value(value, route, manifest);
            }
        }
        _ => {}
    }
}

/// Interpret one `metadata((typost: (...)))` element.
fn parse_metadata(
    metadata: &Packed<MetadataElem>,
    route: Option<&str>,
    manifest: &mut SiteManifest,
) {
    let Value::Dict(outer) = &metadata.value else {
        return;
    };
    let Ok(Value::Dict(typost)) = outer.get("typost") else {
        return;
    };
    let Ok(Value::Str(kind)) = typost.get("kind") else {
        return;
    };

    match kind.as_str() {
        "site" => {
            if let Ok(Value::Str(title)) = typost.get("title") {
                manifest.title = Some(title.as_str().to_owned());
            }
        }
        "page" => {
            let Ok(Value::Str(route)) = typost.get("route") else {
                return;
            };
            let data = typost
                .get("data")
                .map(FrontMatter::from_value)
                .unwrap_or_default();
            manifest.pages.push(PageMeta {
                route: route.as_str().to_owned(),
                src: None,
                data,
            });
        }
        other => {
            manifest.entries.push(MetadataEntry {
                kind: other.to_owned(),
                route: route.map(ToOwned::to_owned),
                data: FrontMatter::from_value(&Value::Dict(typost.clone())),
            });
        }
    }
}

/// Compile the entry to a [`Bundle`] and export it to files.
fn render(world: &TypostWorld) -> Result<RenderOutput> {
    let warned = typst::compile::<Bundle>(world);
    if !warned.warnings.is_empty() {
        eprintln!("warning: {}", format_diagnostics(&warned.warnings));
    }

    let bundle = match warned.output {
        Ok(bundle) => bundle,
        Err(diags) => bail!("compilation failed:\n{}", format_diagnostics(&diags)),
    };

    let files = export(&bundle, &BundleOptions::default())
        .map_err(|diags| anyhow!("export failed:\n{}", format_diagnostics(&diags)))?;

    let mut output = RenderOutput::new();
    for (vpath, bytes) in files {
        let path = vpath.get_with_slash().trim_start_matches('/').to_owned();
        let bytes = if path.ends_with(".html") {
            normalize_html(&bytes)
        } else {
            bytes.to_vec()
        };
        output.insert(path, bytes);
    }
    Ok(output)
}

/// Apply typost's light HTML normalizations.
fn normalize_html(bytes: &[u8]) -> Vec<u8> {
    match std::str::from_utf8(bytes) {
        Ok(html) => match relocate_endnotes(html) {
            Some(fixed) => fixed.into_bytes(),
            None => html.as_bytes().to_vec(),
        },
        Err(_) => bytes.to_vec(),
    }
}

/// Move Typst's footnote container into the content region.
///
/// Typst's HTML exporter appends `<section role="doc-endnotes">` to the very
/// end of `<body>` — after any layout chrome such as a footer. Footnotes belong
/// to the page's content, so we relocate them to the end of
/// `<main id="typost-content">`. This also matters for content-scoped plugins
/// (which must not leave footnotes outside the region).
fn relocate_endnotes(html: &str) -> Option<String> {
    const OPEN: &str = "<section role=\"doc-endnotes\">";
    const CONTENT_CLOSE: &str = "</main>";

    let start = html.find(OPEN)?;
    let end = find_section_end(html, start)?;
    let block = &html[start..end];

    let without = format!("{}{}", &html[..start], &html[end..]);
    let close = without.rfind(CONTENT_CLOSE)?;

    let mut fixed = String::with_capacity(without.len() + block.len());
    fixed.push_str(&without[..close]);
    fixed.push_str(block);
    fixed.push_str(&without[close..]);
    Some(fixed)
}

/// Find the byte index just past the `</section>` matching the `<section>` at
/// `start`, accounting for nesting.
fn find_section_end(html: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut pos = start;
    while let Some(offset) = html[pos..].find('<') {
        let at = pos + offset;
        let rest = &html[at..];
        if rest.starts_with("</section") && is_tag_boundary(rest, "</section") {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(at + rest.find('>')? + 1);
            }
        } else if rest.starts_with("<section") && is_tag_boundary(rest, "<section") {
            depth += 1;
        }
        pos = at + 1;
    }
    None
}

/// Whether the character after a tag name is a space, `>`, or `/`.
fn is_tag_boundary(text: &str, tag: &str) -> bool {
    matches!(
        text.as_bytes().get(tag.len()),
        Some(b' ') | Some(b'>') | Some(b'/')
    )
}

/// Write the rendered files to disk.
fn emit(output: &RenderOutput, dir: &Path) -> Result<()> {
    for (path, bytes) in &output.files {
        let full = dir.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create `{}`", parent.display()))?;
        }
        std::fs::write(&full, bytes)
            .with_context(|| format!("failed to write `{}`", full.display()))?;
    }
    Ok(())
}

/// Render Typst diagnostics into a readable string.
fn format_diagnostics(diags: &[SourceDiagnostic]) -> String {
    diags
        .iter()
        .map(|diag| {
            let mut text = diag.message.to_string();
            for hint in &diag.hints {
                text.push_str("\n  hint: ");
                text.push_str(&hint.v);
            }
            text
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::relocate_endnotes;

    #[test]
    fn moves_endnotes_into_content_region() {
        let html = "<body><header>h</header>\
                    <main id=\"typost-content\"><p>x</p></main>\
                    <footer>f</footer>\
                    <section role=\"doc-endnotes\"><ol><li>n</li></ol></section>\
                    </body>";
        let fixed = relocate_endnotes(html).unwrap();
        let notes = fixed.find("doc-endnotes").unwrap();
        let main_close = fixed.find("</main>").unwrap();
        let footer = fixed.find("<footer>").unwrap();
        assert!(
            notes < main_close,
            "endnotes should be inside the content region"
        );
        assert!(
            main_close < footer,
            "footer should follow the content region"
        );
    }

    #[test]
    fn no_endnotes_is_a_noop() {
        let html = "<body><main id=\"typost-content\">x</main></body>";
        assert!(relocate_endnotes(html).is_none());
    }

    #[test]
    fn handles_nested_sections() {
        let html = "<body><main id=\"typost-content\">x</main>\
                    <section role=\"doc-endnotes\"><section>inner</section></section>\
                    </body>";
        let fixed = relocate_endnotes(html).unwrap();
        assert!(
            fixed.contains(
                "<section role=\"doc-endnotes\"><section>inner</section></section></main>"
            )
        );
    }
}

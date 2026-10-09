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
    /// The URL path the site is served under (e.g. `/typost` for a GitHub
    /// Pages project page). Empty means the site root.
    pub base: String,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            entry: "home.typ".to_owned(),
            out: PathBuf::from("dist"),
            base: String::new(),
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

    /// Register a list of boxed plugins (e.g. from the CLI).
    pub fn plugins(mut self, plugins: Vec<Box<dyn Plugin>>) -> Self {
        self.plugins.extend(plugins);
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
    //    so plugins can override it if they need to), and materialize the
    //    combined overlay into `lib/` so plain `typst` and Tinymist can resolve
    //    it.
    let overlay = collect_typst(plugins)?;
    materialize_overlay(&options.root, &overlay)?;

    let world = TypostWorld::new(options.root.clone(), &options.entry, overlay)?;

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

    // 7. Rewrite root-absolute URLs for the site's base path (a Pages project
    //    page lives under `/<repo>`), then write everything to disk.
    apply_base(&mut output, &options.base);
    emit(&output, &options.root, &options.out)?;
    Ok(())
}

/// Collect the Typst sources contributed by the stdlib and the plugins.
///
/// The stdlib is added first, then each plugin in registration order, so a
/// plugin can override a stdlib file if it needs to.
pub fn collect_typst(plugins: &[Box<dyn Plugin>]) -> Result<TypstOverlay> {
    let mut overlay = TypstOverlay::new();
    crate::stdlib::add_to(&mut overlay)?;
    for plugin in plugins {
        plugin
            .typst(&mut overlay)
            .with_context(|| format!("plugin `{}` failed in its `typst` stage", plugin.name()))?;
    }
    Ok(overlay)
}

/// Materialize the stdlib and plugin Typst sources into `<root>/lib`.
///
/// Shared by `typost build` and `typost init`, so editors see the same helpers
/// before the first build.
pub fn materialize_typst(root: &Path, plugins: &[Box<dyn Plugin>]) -> Result<()> {
    materialize_overlay(root, &collect_typst(plugins)?)
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
    let mut problems = Vec::new();
    walk(&content, None, &mut manifest, &mut problems);
    for problem in &problems {
        eprintln!("warning: {problem}");
    }

    let warnings = sink.warnings();
    if !warnings.is_empty() {
        eprintln!("warning: {}", format_diagnostics(&warnings));
    }

    Ok(manifest)
}

/// Walk evaluated content, collecting `typost` metadata. Entries inside a
/// `document` are tagged with that document's route.
fn walk(
    content: &Content,
    route: Option<&str>,
    manifest: &mut SiteManifest,
    problems: &mut Vec<String>,
) {
    if let Some(metadata) = content.to_packed::<MetadataElem>() {
        parse_metadata(metadata, route, manifest, problems);
    }

    if let Some(document) = content.to_packed::<DocumentElem>() {
        let doc_route = document.path.as_ref().get_without_slash().to_owned();
        walk(&document.body, Some(&doc_route), manifest, problems);
    } else {
        for (_, value) in content.fields() {
            walk_value(value, route, manifest, problems);
        }
    }
}

/// Walk a field value (content or an array of them).
fn walk_value(
    value: Value,
    route: Option<&str>,
    manifest: &mut SiteManifest,
    problems: &mut Vec<String>,
) {
    match value {
        Value::Content(content) => walk(&content, route, manifest, problems),
        Value::Array(array) => {
            for value in array {
                walk_value(value, route, manifest, problems);
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
    problems: &mut Vec<String>,
) {
    let Value::Dict(outer) = &metadata.value else {
        return;
    };
    let Ok(Value::Dict(typost)) = outer.get("typost") else {
        return;
    };
    let Ok(Value::Str(kind)) = typost.get("kind") else {
        problems.push("a `typost` metadata entry has no string `kind`".into());
        return;
    };

    match kind.as_str() {
        "site" => match typost.get("title") {
            Ok(Value::Str(title)) => manifest.title = Some(title.as_str().to_owned()),
            Ok(Value::None) | Err(_) => {}
            Ok(other) => problems.push(format!(
                "the site `title` must be a string, found a `{}` value",
                other.ty()
            )),
        },
        "page" => {
            let Ok(Value::Str(route)) = typost.get("route") else {
                problems.push("a `page` entry has no string `route`".into());
                return;
            };
            let data = typost
                .get("data")
                .map(FrontMatter::from_value)
                .unwrap_or_default();
            for (path, ty) in data.unsupported() {
                problems.push(format!(
                    "page `{route}`: front matter `{path}` is a `{ty}` value, \
                     which typost cannot decode"
                ));
            }
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

/// Rewrite root-absolute URLs in exported HTML for the site's base path.
///
/// Runs after the plugins, so it also catches URLs they inject (for example
/// the feed's `<link>`). Content inside `<pre>`/`<code>` is left alone, so a
/// code sample that happens to contain `href="/…"` is not rewritten.
fn apply_base(output: &mut RenderOutput, base: &str) {
    let base = base.trim_end_matches('/');
    if base.is_empty() {
        return;
    }
    for (path, bytes) in output.files.iter_mut() {
        if !path.ends_with(".html") {
            continue;
        }
        if let Ok(html) = std::str::from_utf8(bytes) {
            *bytes = prefix_urls(html, base).into_bytes();
        }
    }
}

/// Prefix root-absolute URLs with `base`, skipping code blocks.
fn prefix_urls(html: &str, base: &str) -> String {
    let mut out = String::with_capacity(html.len() + 64);
    let mut rest = html;
    loop {
        let next = ["<pre", "<code"]
            .iter()
            .filter_map(|tag| rest.find(tag))
            .min();
        let Some(at) = next else {
            out.push_str(&prefix_attrs(rest, base));
            break;
        };
        out.push_str(&prefix_attrs(&rest[..at], base));
        let tag = if rest[at..].starts_with("<pre") {
            "</pre>"
        } else {
            "</code>"
        };
        let end = rest[at..]
            .find(tag)
            .map(|e| at + e + tag.len())
            .unwrap_or(rest.len());
        out.push_str(&rest[at..end]);
        rest = &rest[end..];
    }
    out
}

/// Prefix the root-absolute `href`/`src` values in a fragment (not `//host`).
fn prefix_attrs(html: &str, base: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some((at, attr)) = ["href=\"", "src=\""]
        .iter()
        .filter_map(|attr| rest.find(attr).map(|at| (at, *attr)))
        .min_by_key(|(at, _)| *at)
    {
        out.push_str(&rest[..at + attr.len()]);
        let after = &rest[at + attr.len()..];
        if after.starts_with('/') && !after.starts_with("//") {
            out.push_str(base);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Write the rendered files to disk, replacing any previous build.
fn emit(output: &RenderOutput, root: &Path, dir: &Path) -> Result<()> {
    clean(dir, root)?;
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

/// Remove a previous build's output directory.
///
/// Refuses to delete the project root or any ancestor of it, so a misconfigured
/// `out` cannot wipe the sources.
fn clean(dir: &Path, root: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let dir = dir
        .canonicalize()
        .with_context(|| format!("failed to resolve `{}`", dir.display()))?;
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to resolve `{}`", root.display()))?;
    if root.starts_with(&dir) {
        bail!(
            "refusing to clean `{}`: it contains the project root `{}`",
            dir.display(),
            root.display()
        );
    }
    std::fs::remove_dir_all(&dir)
        .with_context(|| format!("failed to clean `{}`", dir.display()))?;
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
    use super::{clean, prefix_urls, relocate_endnotes};

    #[test]
    fn prefixes_root_absolute_urls_but_not_code() {
        let html = "<a href=\"/blog/\">b</a>\
                    <link href=\"/atom.xml\">\
                    <img src=\"/i.png\">\
                    <pre>href=\"/x\"</pre>\
                    <a href=\"//host\">h</a>";
        assert_eq!(
            prefix_urls(html, "/typost"),
            "<a href=\"/typost/blog/\">b</a>\
             <link href=\"/typost/atom.xml\">\
             <img src=\"/typost/i.png\">\
             <pre>href=\"/x\"</pre>\
             <a href=\"//host\">h</a>"
        );
    }

    #[test]
    fn clean_refuses_the_project_root() {
        let dir = std::env::temp_dir().join(format!("typost-clean-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let error = clean(&dir, &dir).expect_err("should refuse the root");
        assert!(format!("{error:?}").contains("refusing"));
        assert!(dir.exists(), "the root must be left alone");

        let _ = std::fs::remove_dir_all(&dir);
    }

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

//! Hover-to-reveal spoilers.
//!
//! Contributes `lib/typost/spoiler.typ` with `#spoiler[...]`, and injects the
//! (small) stylesheet into every page that uses one. The style is themed
//! through CSS variables so a site can restyle it without touching the plugin.

use anyhow::Result;
use typost_core::{Plugin, RenderOutput, SiteManifest, TypstOverlay};

/// The class put on the rendered spoiler element.
const SPOILER_CLASS: &str = "typost-spoiler";
/// The class put on the injected `<style>`, used to stay idempotent.
const STYLE_CLASS: &str = "typost-spoiler-style";

/// A plugin that hides content until it is hovered (or focused).
pub struct Spoiler;

impl Spoiler {
    /// Create the plugin.
    pub fn new() -> Self {
        Self
    }
}

impl Default for Spoiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for Spoiler {
    fn name(&self) -> &str {
        "spoiler"
    }

    fn typst(&self, overlay: &mut TypstOverlay) -> Result<()> {
        overlay.add("lib/typost/spoiler.typ", SPOILER_TYP);
        Ok(())
    }

    fn post(&self, out: &mut RenderOutput, _manifest: &SiteManifest) -> Result<()> {
        for (path, bytes) in out.files.iter_mut() {
            if !path.ends_with(".html") {
                continue;
            }
            let Ok(html) = std::str::from_utf8(bytes) else {
                continue;
            };
            if !html.contains(SPOILER_CLASS) || html.contains(STYLE_CLASS) {
                continue;
            }
            let Some(at) = html.rfind("</head>") else {
                continue;
            };
            let mut updated = String::with_capacity(html.len() + SPOILER_CSS.len() + 64);
            updated.push_str(&html[..at]);
            updated.push_str("<style class=\"");
            updated.push_str(STYLE_CLASS);
            updated.push_str("\">");
            updated.push_str(SPOILER_CSS);
            updated.push_str("</style>");
            updated.push_str(&html[at..]);
            *bytes = updated.into_bytes();
        }
        Ok(())
    }
}

/// The plugin's Typst helper, materialized into `lib/typost/spoiler.typ`.
const SPOILER_TYP: &str = r#"
/// Hide `body` behind a spoiler, revealed on hover or keyboard focus.
///
/// ```typst
/// The butler did it: #spoiler[Colonel Mustard, in the library].
/// ```
///
/// Pass `block: true` to hide a block (paragraphs, lists, ...) rather than an
/// inline run.
#let spoiler(body, block: false) = html.elem(
  if block { "div" } else { "span" },
  body,
  attrs: (class: "typost-spoiler", tabindex: "0"),
)
"#;

/// The stylesheet injected into pages that use a spoiler.
///
/// Kept in sync with `theme.css` conventions: the variables fall back to
/// gruvbox-material values so the plugin works on any theme.
const SPOILER_CSS: &str = "\
.typost-spoiler{color:transparent;background:var(--spoiler-bg,rgba(146,131,116,.3));\
border-radius:.25em;padding:0 .3em;cursor:help;transition:color .15s ease,\
background-color .15s ease;-webkit-user-select:none;user-select:none}\
.typost-spoiler:hover,.typost-spoiler:focus-visible{color:inherit;\
background:var(--spoiler-bg-revealed,transparent);-webkit-user-select:text;\
user-select:text}";

#[cfg(test)]
mod tests {
    use super::*;

    fn page(body: &str) -> Vec<u8> {
        format!("<html><head><title>t</title></head><body>{body}</body></html>").into_bytes()
    }

    #[test]
    fn injects_css_into_pages_with_spoilers() {
        let mut out = RenderOutput::new();
        out.insert(
            "index.html",
            page("<p>answer: <span class=\"typost-spoiler\">42</span></p>"),
        );

        Spoiler::new()
            .post(&mut out, &SiteManifest::default())
            .unwrap();

        let html = String::from_utf8(out.get("index.html").unwrap().to_vec()).unwrap();
        assert!(html.contains("<style class=\"typost-spoiler-style\">"));
        assert!(html.contains(".typost-spoiler:hover"));
        // Injected into the head, before any content.
        assert!(html.find("<style").unwrap() < html.find("<body>").unwrap());
    }

    #[test]
    fn ignores_pages_without_spoilers() {
        let mut out = RenderOutput::new();
        out.insert("index.html", page("<p>nothing hidden</p>"));

        Spoiler::new()
            .post(&mut out, &SiteManifest::default())
            .unwrap();

        let html = String::from_utf8(out.get("index.html").unwrap().to_vec()).unwrap();
        assert!(!html.contains("<style"));
    }

    #[test]
    fn is_idempotent() {
        let mut out = RenderOutput::new();
        out.insert(
            "index.html",
            page("<span class=\"typost-spoiler\">x</span>"),
        );

        let plugin = Spoiler::new();
        plugin.post(&mut out, &SiteManifest::default()).unwrap();
        plugin.post(&mut out, &SiteManifest::default()).unwrap();

        let html = String::from_utf8(out.get("index.html").unwrap().to_vec()).unwrap();
        assert_eq!(
            html.matches("<style class=\"typost-spoiler-style\">")
                .count(),
            1
        );
    }

    #[test]
    fn contributes_the_helper() {
        let mut overlay = TypstOverlay::new();
        Spoiler::new().typst(&mut overlay).unwrap();
        let source = overlay
            .files
            .iter()
            .find(|(path, _)| path.get_without_slash() == "lib/typost/spoiler.typ")
            .map(|(_, bytes)| bytes)
            .expect("helper contributed");
        assert!(String::from_utf8_lossy(source).contains("#let spoiler"));
    }
}

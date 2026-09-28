//! Spoilers: reveal content on hover (inline) or click (block).
//!
//! Contributes `lib/typost/spoiler.typ` with `#spoiler[...]`, and injects the
//! (small) stylesheet and script into every page that uses one. The style is
//! themed through CSS variables so a site can restyle it without touching the
//! plugin.

use anyhow::Result;
use typost_core::html::inject_head;
use typost_core::{Plugin, RenderOutput, SiteManifest, TypstOverlay};

/// The class put on the rendered spoiler element.
const SPOILER_CLASS: &str = "typost-spoiler";
/// The class put on the injected `<style>`, used to stay idempotent.
const STYLE_CLASS: &str = "typost-spoiler-style";
/// The class put on the injected `<script>`, used to stay idempotent.
const SCRIPT_CLASS: &str = "typost-spoiler-script";

/// A plugin that hides content behind an inline or block spoiler.
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
        let snippet = format!(
            "<style class=\"{STYLE_CLASS}\">{SPOILER_CSS}</style>\
             <script class=\"{SCRIPT_CLASS}\">{SPOILER_JS}</script>"
        );
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
            let mut updated = html.to_owned();
            if inject_head(&mut updated, &snippet) {
                *bytes = updated.into_bytes();
            }
        }
        Ok(())
    }
}

/// The plugin's Typst helper, materialized into `lib/typost/spoiler.typ`.
const SPOILER_TYP: &str = r#"
/// Hide `body` behind a spoiler.
///
/// Inline spoilers (the default) reveal on hover or keyboard focus:
///
/// ```typst
/// The butler did it: #spoiler[Colonel Mustard, in the library].
/// ```
///
/// Block spoilers (`block: true`) stay blurred behind a `hint`; clicking (or
/// pressing enter) reveals them, and they stay revealed.
///
/// ```typst
/// #spoiler(block: true)[
///   ...a hidden block (paragraphs, lists, code, ...)...
/// ]
/// ```
#let spoiler(body, block: false, hint: "click to reveal") = {
  if block {
    html.elem(
      "div",
      [
        #html.elem("span", hint, attrs: (class: "typost-spoiler-hint"))
        #html.elem("div", body, attrs: (class: "typost-spoiler-content"))
      ],
      attrs: (class: "typost-spoiler-block", tabindex: "0"),
    )
  } else {
    html.elem("span", body, attrs: (class: "typost-spoiler", tabindex: "0"))
  }
}
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
user-select:text}\
.typost-spoiler-block{position:relative;margin:1em 0;cursor:pointer}\
.typost-spoiler-content{filter:blur(var(--spoiler-blur,.4em));\
transition:filter .2s ease}\
.typost-spoiler-hint{position:absolute;top:50%;left:50%;\
transform:translate(-50%,-50%);z-index:1;padding:.35em .9em;border-radius:999px;\
background:var(--spoiler-hint-bg,rgba(29,32,33,.85));\
border:1px solid var(--spoiler-border,rgba(146,131,116,.5));\
color:var(--spoiler-hint,var(--fg-dim,#a89984));font-size:.85rem;white-space:nowrap;\
pointer-events:none;-webkit-user-select:none;user-select:none;\
transition:opacity .2s ease}\
.typost-spoiler-block.is-revealed{cursor:auto}\
.typost-spoiler-block.is-revealed .typost-spoiler-content{filter:none}\
.typost-spoiler-block.is-revealed .typost-spoiler-hint{opacity:0}";

/// The script injected into pages that use a spoiler: it reveals a block on the
/// first click (or enter/space) and never hides it again.
const SPOILER_JS: &str = "\
(function(){function init(){document.querySelectorAll('.typost-spoiler-block')\
.forEach(function(block){function reveal(){block.classList.add('is-revealed')}\
block.addEventListener('click',reveal,{once:true});\
block.addEventListener('keydown',function(event){\
if(block.classList.contains('is-revealed'))return;\
if(event.key==='Enter'||event.key===' '){event.preventDefault();reveal()}})})}\
if(document.readyState==='loading'){document.addEventListener('DOMContentLoaded',init)}\
else{init()}})();";

#[cfg(test)]
mod tests {
    use super::*;

    fn page(body: &str) -> Vec<u8> {
        format!("<html><head><title>t</title></head><body>{body}</body></html>").into_bytes()
    }

    #[test]
    fn injects_css_and_script_into_pages_with_spoilers() {
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
        assert!(html.contains(".typost-spoiler-block.is-revealed .typost-spoiler-content"));
        assert!(html.contains("<script class=\"typost-spoiler-script\">"));
        assert!(html.contains("is-revealed"));
        // Injected into the head, before any content.
        assert!(html.find("<style").unwrap() < html.find("<body>").unwrap());
        assert!(html.find("<script").unwrap() < html.find("<body>").unwrap());
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
        assert!(!html.contains("<script"));
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
        assert_eq!(
            html.matches("<script class=\"typost-spoiler-script\">")
                .count(),
            1
        );
    }

    #[test]
    fn helper_has_hover_and_blur_forms() {
        let mut overlay = TypstOverlay::new();
        Spoiler::new().typst(&mut overlay).unwrap();
        let source = overlay
            .files
            .iter()
            .find(|(path, _)| path.get_without_slash() == "lib/typost/spoiler.typ")
            .map(|(_, bytes)| bytes)
            .expect("helper contributed");
        let source = String::from_utf8_lossy(source);
        assert!(source.contains("#let spoiler"));
        assert!(source.contains("typost-spoiler-content"));
        assert!(source.contains("typost-spoiler-hint"));
        assert!(source.contains("click to reveal"));
    }
}

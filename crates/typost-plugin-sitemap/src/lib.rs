//! A `sitemap.xml` plugin.

use anyhow::Result;
use typost_core::{Plugin, RenderOutput, SiteManifest};

/// Emits `sitemap.xml` listing every rendered HTML page.
///
/// This is a `post`-stage plugin. It reads the routes out of the rendered
/// output rather than the manifest so that pages produced during render (for
/// example the ones the taxonomy module generates inside a `context`) are
/// included too.
pub struct Sitemap {
    base_url: String,
}

impl Sitemap {
    /// Create a sitemap plugin with the site's base URL (no trailing slash
    /// needed).
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

impl Plugin for Sitemap {
    fn name(&self) -> &str {
        "sitemap"
    }

    fn post(&self, out: &mut RenderOutput, _manifest: &SiteManifest) -> Result<()> {
        let base = self.base_url.trim_end_matches('/');

        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
        );
        for path in out.files.keys() {
            if !path.ends_with(".html") {
                continue;
            }
            let loc = format!("{base}/{}", path.trim_start_matches('/'));
            xml.push_str("  <url><loc>");
            xml.push_str(&escape(&loc));
            xml.push_str("</loc></url>\n");
        }
        xml.push_str("</urlset>\n");

        out.insert("sitemap.xml", xml.into_bytes());
        Ok(())
    }
}

/// Minimal XML text escaping for URLs.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::Sitemap;
    use typost_core::{Plugin, RenderOutput, SiteManifest};

    #[test]
    fn writes_a_url_for_every_rendered_page() {
        let mut out = RenderOutput::new();
        out.insert("index.html", b"<html></html>".to_vec());
        out.insert("hello/index.html", b"<html></html>".to_vec());
        out.insert("tags/index.html", b"<html></html>".to_vec());
        out.insert("static/css/theme.css", b"body{}".to_vec());

        Sitemap::new("https://example.com/")
            .post(&mut out, &SiteManifest::default())
            .unwrap();

        let xml = String::from_utf8(out.get("sitemap.xml").unwrap().to_vec()).unwrap();
        assert!(xml.contains("<loc>https://example.com/index.html</loc>"));
        assert!(xml.contains("<loc>https://example.com/hello/index.html</loc>"));
        assert!(xml.contains("<loc>https://example.com/tags/index.html</loc>"));
        assert!(!xml.contains("theme.css"));
    }
}

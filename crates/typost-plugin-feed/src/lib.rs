//! An Atom feed plugin.
//!
//! Generates an Atom 1.0 feed from the manifest's dated pages (by default the
//! `post` section), newest first, and links it from every page's `<head>`.
//!
//! This is a `post` plugin: it reads the rendered HTML to embed each entry's
//! content. Register it after `Encrypt` so that encrypted posts contribute
//! ciphertext rather than plaintext.

use anyhow::Result;
use typost_core::{FrontMatter, PageMeta, Plugin, RenderOutput, SiteManifest};

/// The default feed file name.
const DEFAULT_PATH: &str = "atom.xml";
/// The default maximum number of entries.
const DEFAULT_LIMIT: usize = 20;
/// The default front-matter section to include.
const DEFAULT_SECTION: &str = "post";
/// The content-region marker emitted by the stdlib.
const CONTENT_MARKER: &str = "id=\"typost-content\"";

/// Generates an Atom feed.
pub struct Feed {
    base_url: String,
    path: String,
    limit: usize,
    section: String,
    title: Option<String>,
    description: Option<String>,
    author: Option<String>,
}

impl Feed {
    /// Create a feed plugin for a site rooted at `base_url` (trailing slash
    /// optional).
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            path: DEFAULT_PATH.to_owned(),
            limit: DEFAULT_LIMIT,
            section: DEFAULT_SECTION.to_owned(),
            title: None,
            description: None,
            author: None,
        }
    }

    /// Set the feed's output path (default `atom.xml`).
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// Set the maximum number of entries (default 20).
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set the front-matter `section` to include (default `post`).
    pub fn section(mut self, section: impl Into<String>) -> Self {
        self.section = section.into();
        self
    }

    /// Override the feed title (defaults to the site title).
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the feed subtitle.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the feed author name.
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }
}

impl Plugin for Feed {
    fn name(&self) -> &str {
        "feed"
    }

    fn post(&self, out: &mut RenderOutput, manifest: &SiteManifest) -> Result<()> {
        let base = self.base_url.trim_end_matches('/');

        // Collect this section's dated pages, newest first.
        let date_of = |page: &PageMeta| {
            page.data
                .get("date")
                .and_then(FrontMatter::as_str)
                .map(str::to_owned)
        };
        let mut entries: Vec<&PageMeta> = manifest
            .pages
            .iter()
            .filter(|page| {
                page.data.get("section").and_then(FrontMatter::as_str)
                    == Some(self.section.as_str())
            })
            .filter(|page| date_of(page).is_some())
            .collect();
        entries.sort_by(|a, b| date_of(b).cmp(&date_of(a)));
        entries.truncate(self.limit);

        let title = self
            .title
            .clone()
            .or_else(|| manifest.title.clone())
            .unwrap_or_else(|| "Feed".to_owned());
        let updated = entries
            .iter()
            .filter_map(|page| date_of(page))
            .max()
            .map(|date| rfc3339(&date))
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_owned());

        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<feed xmlns=\"http://www.w3.org/2005/Atom\">\n");
        text(&mut xml, "  ", "title", &title);
        if let Some(description) = &self.description {
            text(&mut xml, "  ", "subtitle", description);
        }
        text(&mut xml, "  ", "id", &format!("{base}/"));
        text(&mut xml, "  ", "updated", &updated);
        link(
            &mut xml,
            "  ",
            &format!("{base}/{}", self.path.trim_start_matches('/')),
            "self",
        );
        link(&mut xml, "  ", &format!("{base}/"), "alternate");
        if let Some(author) = &self.author {
            xml.push_str("  <author>\n");
            text(&mut xml, "    ", "name", author);
            xml.push_str("  </author>\n");
        }
        text(&mut xml, "  ", "generator", "typost");

        for page in entries {
            let data = &page.data;
            let url = page_url(base, &page.route);
            let date = data
                .get("date")
                .and_then(FrontMatter::as_str)
                .unwrap_or_default();
            let entry_title = data
                .get("title")
                .and_then(FrontMatter::as_str)
                .unwrap_or(&page.route);

            xml.push_str("  <entry>\n");
            text(&mut xml, "    ", "title", entry_title);
            link(&mut xml, "    ", &url, "alternate");
            text(&mut xml, "    ", "id", &url);
            text(&mut xml, "    ", "updated", &rfc3339(date));
            text(&mut xml, "    ", "published", &rfc3339(date));
            if let Some(summary) = data.get("summary").and_then(FrontMatter::as_str) {
                text(&mut xml, "    ", "summary", summary);
            }
            if let Some(tags) = data.get("tags").and_then(FrontMatter::as_array) {
                for tag in tags.iter().filter_map(FrontMatter::as_str) {
                    xml.push_str("    <category term=\"");
                    xml.push_str(&escape(tag));
                    xml.push_str("\"/>\n");
                }
            }
            if let Some(content) = content_of(out, &page.route, base) {
                xml.push_str("    <content type=\"html\">");
                xml.push_str(&escape(&content));
                xml.push_str("</content>\n");
            }
            xml.push_str("  </entry>\n");
        }
        xml.push_str("</feed>\n");

        let href = format!("/{}", self.path.trim_start_matches('/'));
        inject_head_link(out, &href, &title);

        out.insert(self.path.clone(), xml.into_bytes());
        Ok(())
    }
}

/// The absolute URL of a page, using a directory URL for `index.html`.
fn page_url(base: &str, route: &str) -> String {
    let path = route.strip_suffix("index.html").unwrap_or(route);
    format!("{base}/{}", path.trim_start_matches('/'))
}

/// Read a page's content region and make its root-relative URLs absolute.
fn content_of(out: &RenderOutput, route: &str, base: &str) -> Option<String> {
    let html = std::str::from_utf8(out.get(route)?).ok()?;
    let marker = html.find(CONTENT_MARKER)?;
    let open_end = marker + html[marker..].find('>')? + 1;
    let close = open_end + html[open_end..].find("</main>")?;
    Some(absolutize(&html[open_end..close], base))
}

/// Turn `="` + `/path` (but not `="//host`) into an absolute URL.
fn absolutize(html: &str, base: &str) -> String {
    const ATTRS: [&str; 4] = ["href=\"", "src=\"", "poster=\"", "xlink:href=\""];

    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some((at, attr)) = ATTRS
        .iter()
        .filter_map(|attr| rest.find(attr).map(|at| (at, *attr)))
        .min_by_key(|(at, _)| *at)
    {
        out.push_str(&rest[..at + attr.len()]);
        let after = &rest[at + attr.len()..];
        if after.starts_with("//") {
            rest = after;
        } else if let Some(path) = after.strip_prefix('/') {
            out.push_str(base);
            out.push('/');
            rest = path;
        } else {
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Append `<tag>value</tag>` at the given indent.
fn text(xml: &mut String, indent: &str, tag: &str, value: &str) {
    xml.push_str(indent);
    xml.push('<');
    xml.push_str(tag);
    xml.push('>');
    xml.push_str(&escape(value));
    xml.push_str("</");
    xml.push_str(tag);
    xml.push_str(">\n");
}

/// Append a `<link …/>` at the given indent.
fn link(xml: &mut String, indent: &str, href: &str, rel: &str) {
    xml.push_str(indent);
    xml.push_str("<link href=\"");
    xml.push_str(&escape(href));
    xml.push_str("\" rel=\"");
    xml.push_str(rel);
    xml.push_str("\"/>\n");
}

/// Format a front-matter date (`YYYY-MM-DD`) as RFC 3339.
fn rfc3339(date: &str) -> String {
    if date.len() == 10 {
        format!("{date}T00:00:00Z")
    } else {
        date.to_owned()
    }
}

/// Escape text for XML and HTML attributes.
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

/// Add `<link rel="alternate">` to every HTML page's `<head>`.
fn inject_head_link(out: &mut RenderOutput, href: &str, title: &str) {
    let tag = format!(
        "<link rel=\"alternate\" type=\"application/atom+xml\" title=\"{}\" href=\"{}\">",
        escape(title),
        escape(href),
    );
    for (path, bytes) in out.files.iter_mut() {
        if !path.ends_with(".html") {
            continue;
        }
        let Ok(html) = std::str::from_utf8(bytes) else {
            continue;
        };
        if html.contains("application/atom+xml") {
            continue;
        }
        let Some(at) = html.rfind("</head>") else {
            continue;
        };
        let mut updated = String::with_capacity(html.len() + tag.len());
        updated.push_str(&html[..at]);
        updated.push_str(&tag);
        updated.push_str(&html[at..]);
        *bytes = updated.into_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use typost_core::{FrontMatter, PageMeta};

    fn page(route: &str, title: &str, date: &str) -> PageMeta {
        PageMeta {
            route: route.into(),
            src: None,
            data: FrontMatter::Dict(
                [
                    ("section".to_owned(), FrontMatter::Str("post".into())),
                    ("title".to_owned(), FrontMatter::Str(title.into())),
                    ("date".to_owned(), FrontMatter::Str(date.into())),
                    (
                        "tags".to_owned(),
                        FrontMatter::Array(vec![FrontMatter::Str("rust".into())]),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        }
    }

    #[test]
    fn emits_entries_newest_first_with_absolute_content() {
        let mut out = RenderOutput::new();
        out.insert(
            "post/old/index.html",
            b"<head></head><body><main id=\"typost-content\"><a href=\"/post/old/\">old</a></main></body>".to_vec(),
        );
        out.insert(
            "post/new/index.html",
            b"<head></head><body><main id=\"typost-content\"><p>new</p></main></body>".to_vec(),
        );
        let manifest = SiteManifest {
            title: Some("My Site".into()),
            pages: vec![
                page("post/old/index.html", "Old", "2020-01-01"),
                page("post/new/index.html", "New", "2024-06-01"),
                // Undated pages are ignored.
                PageMeta {
                    route: "about/index.html".into(),
                    src: None,
                    data: FrontMatter::None,
                },
            ],
            entries: Vec::new(),
        };

        Feed::new("https://example.com/")
            .author("Ada")
            .post(&mut out, &manifest)
            .unwrap();

        let xml = String::from_utf8(out.get("atom.xml").unwrap().to_vec()).unwrap();
        assert!(xml.contains("<title>My Site</title>"));
        assert!(xml.contains("<name>Ada</name>"));
        assert!(xml.contains("<id>https://example.com/post/new/</id>"));
        assert!(xml.contains("<updated>2024-06-01T00:00:00Z</updated>"));
        assert!(xml.contains("<category term=\"rust\"/>"));
        // Newest entry precedes the older one.
        assert!(xml.find("New").unwrap() < xml.find("Old").unwrap());
        // Root-relative content links were made absolute.
        assert!(xml.contains("https://example.com/post/old/"));

        // The feed is linked from the pages' heads.
        let html = String::from_utf8(out.get("post/new/index.html").unwrap().to_vec()).unwrap();
        assert!(html.contains("application/atom+xml"));
    }

    #[test]
    fn limit_is_respected() {
        let mut out = RenderOutput::new();
        let pages: Vec<PageMeta> = (0..5)
            .map(|i| page(&format!("post/{i}/index.html"), &format!("P{i}"), &format!("202{i}-01-01")))
            .collect();
        let manifest = SiteManifest {
            title: None,
            pages,
            entries: Vec::new(),
        };

        Feed::new("https://example.com").limit(2).post(&mut out, &manifest).unwrap();
        let xml = String::from_utf8(out.get("atom.xml").unwrap().to_vec()).unwrap();
        assert_eq!(xml.matches("<entry>").count(), 2);
    }
}

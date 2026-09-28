//! HTML string helpers shared by plugins.
//!
//! Plugins post-process the exported HTML, so escaping and `<head>` splicing
//! live here instead of being re-implemented in each plugin.

/// The id of the stdlib's content region.
///
/// `lib/typost.typ` puts this on the `<main>` wrapper around every page body,
/// and content-scoped plugins look for it. Kept in sync by a test below.
pub const CONTENT_ID: &str = "typost-content";

/// Escape text for XML text and HTML/XML attribute values.
pub fn escape(text: &str) -> String {
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

/// The inner HTML of a page's content region (`<main id="typost-content">`).
pub fn content_region(html: &str) -> Option<&str> {
    let marker = html.find(CONTENT_ID)?;
    let open_end = marker + html[marker..].find('>')? + 1;
    let close = open_end + html[open_end..].find("</main>")?;
    Some(&html[open_end..close])
}

/// Insert `snippet` just before `</head>`.
///
/// Returns whether a head was found and the snippet inserted.
pub fn inject_head(html: &mut String, snippet: &str) -> bool {
    let Some(at) = html.rfind("</head>") else {
        return false;
    };
    html.insert_str(at, snippet);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_markup() {
        assert_eq!(escape("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&apos;");
    }

    #[test]
    fn extracts_the_content_region() {
        let html = "<body><main id=\"typost-content\"><p>hi</p></main></body>";
        assert_eq!(content_region(html), Some("<p>hi</p>"));
        assert_eq!(content_region("<body>nope</body>"), None);
    }

    #[test]
    fn injects_into_the_head() {
        let mut html = String::from("<head><title>t</title></head><body></body>");
        assert!(inject_head(&mut html, "<style>x</style>"));
        assert!(html.contains("<title>t</title><style>x</style></head>"));
    }

    #[test]
    fn stdlib_emits_the_content_id() {
        // Guards the coupling between this constant and the Typst stdlib.
        assert!(crate::stdlib::STDLIB_TYP.contains(CONTENT_ID));
    }
}

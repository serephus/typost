// Site theme: chrome, the configured `page` directive, and small components.

#import "lib/typost.typ": page as typost-page
#import "lib/typost/encrypted.typ": encrypted

#let site-title = "typost demo"

#let nav = (
  (href: "/", label: "Home"),
  (href: "/about/", label: "About"),
  (href: "/blog/", label: "Blog"),
)

#let styles = "
  :root {
    color-scheme: dark;
    --bg: #16181d;
    --surface: #1d2026;
    --surface-2: #23272f;
    --text: #d7dae0;
    --muted: #8b919c;
    --border: #2a2e36;
    --accent: #8ab4f8;
  }

  * { box-sizing: border-box; }

  html { -webkit-text-size-adjust: 100%; }

  body {
    margin: 0 auto;
    max-width: 44rem;
    padding: 3.5rem 1.25rem 5rem;
    background: var(--bg);
    color: var(--text);
    font: 17px/1.75 -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto,
      Helvetica, Arial, sans-serif;
    -webkit-font-smoothing: antialiased;
    text-rendering: optimizeLegibility;
  }

  ::selection { background: rgba(138, 180, 248, 0.25); }

  a { color: var(--accent); text-decoration: none; }
  a:hover { text-decoration: underline; text-underline-offset: 2px; }

  header {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem 1rem;
    align-items: baseline;
    justify-content: space-between;
    padding-bottom: 0.85rem;
    margin-bottom: 2.75rem;
    border-bottom: 1px solid var(--border);
  }

  header > a { color: var(--text); font-weight: 650; letter-spacing: -0.01em; }
  header > a:hover { text-decoration: none; }

  nav { display: flex; gap: 0.9rem; font-size: 0.95rem; }
  nav a { color: var(--muted); }
  nav a:hover { color: var(--text); text-decoration: none; }
  nav a.active { color: var(--text); }

  h1, h2, h3 {
    line-height: 1.25;
    letter-spacing: -0.015em;
    font-weight: 650;
    margin: 2.5rem 0 0.85rem;
  }
  h1 { font-size: 1.95rem; margin-top: 0; }
  h2 { font-size: 1.4rem; }
  h3 { font-size: 1.15rem; }

  p { margin: 0 0 1.15rem; }

  ul, ol { padding-left: 1.4rem; margin: 0 0 1.15rem; }
  li { margin: 0.3rem 0; }

  blockquote {
    margin: 1.75rem 0;
    padding: 0.15rem 0 0.15rem 1.15rem;
    border-left: 3px solid var(--border);
    color: var(--muted);
    font-style: italic;
  }

  code {
    font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas,
      monospace;
    font-size: 0.88em;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 0.1em 0.35em;
  }

  pre {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 1rem 1.15rem;
    margin: 0 0 1.35rem;
    overflow-x: auto;
    line-height: 1.55;
  }
  pre code { background: none; border: 0; padding: 0; font-size: 0.86rem; }

  hr { border: 0; border-top: 1px solid var(--border); margin: 2.5rem 0; }

  table { width: 100%; border-collapse: collapse; margin: 0 0 1.35rem; font-size: 0.95rem; }
  th, td { text-align: left; padding: 0.5rem 0.75rem; border-bottom: 1px solid var(--border); }
  th { color: var(--muted); font-weight: 600; }

  article { padding-bottom: 1.5rem; border-bottom: 1px solid var(--border); margin-bottom: 1.75rem; }
  article:last-child { border-bottom: 0; }
  article h3 { margin-top: 0; }

  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem 0.75rem;
    align-items: center;
    color: var(--muted);
    font-size: 0.9rem;
    margin-bottom: 1.75rem;
  }

  .tags { list-style: none; display: flex; flex-wrap: wrap; gap: 0.4rem; padding: 0; margin: 0; }
  .tags li {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 0.05em 0.6em;
    font-size: 0.85em;
    color: var(--muted);
  }

  .callout {
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 3px solid var(--accent);
    border-radius: 8px;
    padding: 0.9rem 1.1rem;
    margin: 1.75rem 0;
  }
  .callout > :last-child { margin-bottom: 0; }

  footer {
    margin-top: 4.5rem;
    padding-top: 1.1rem;
    border-top: 1px solid var(--border);
    color: var(--muted);
    font-size: 0.9rem;
  }

  /* Typst-generated footnote endnotes (relocated into the content region). */
  section[role='doc-endnotes'] {
    margin-top: 3rem;
    padding-top: 1.25rem;
    border-top: 1px solid var(--border);
    color: var(--muted);
    font-size: 0.92rem;
  }
  section[role='doc-endnotes'] ol { padding-left: 1.2rem; }
  section[role='doc-endnotes'] a { color: var(--muted); }
"

#let post-meta(date: none, tags: ()) = html.elem(
  "div",
  attrs: (class: "meta"),
  [
    #if date != none [ #date ]
    #if tags.len() > 0 [
      #html.elem(
        "ul",
        attrs: (class: "tags"),
        [#for tag in tags [ #html.elem("li", tag) ]],
      )
    ]
  ],
)

/// A semantic `<aside>` produced by a Typst function.
#let callout(inner) = html.elem("aside", attrs: (class: "callout"), inner)

/// A nav link, marked active when the current route is under it.
#let nav-link(route, item) = {
  let path = "/" + route
  let active = if item.href == "/" { path == "/index.html" } else { path.starts-with(item.href) }
  if active {
    html.elem("a", [#item.label], attrs: (href: item.href, class: "active"))
  } else {
    html.elem("a", [#item.label], attrs: (href: item.href))
  }
}

/// The layout: chrome around the already-marked content. `data` is the page's
/// front matter, so the theme can render things like a post byline.
#let frame(route, data, inner) = [
  #html.elem("style", styles)
  #html.elem("header", [
    #link("/", strong[#site-title])
    #html.elem("nav", [
      #for item in nav [ #nav-link(route, item) ]
    ])
  ])
  #if data.at("date", default: none) != none [
    #post-meta(date: data.date, tags: data.at("tags", default: ()))
  ]
  #inner
  #html.elem("footer", [
    Made with #link("https://typst.app/", [Typst]) and typost.
  ])
]

/// The project's `page` directive: the stdlib `page` with this theme's layout.
#let page(body, route: none, title: none, tags: (), ..extra) = typost-page(
  body,
  route: route,
  layout: frame,
  title: title,
  tags: tags,
  ..extra,
)

#import "../theme.typ": page, callout

#show: page.with(
  route: "blog/notes-on-html-export/index.html",
  title: "Notes on HTML export",
  date: "2026-02-20",
  tags: ("typst", "html"),
  description: "What the bundle target gives an SSG.",
  url: "/blog/notes-on-html-export/",
  section: "blog",
)

= Notes on HTML export

typost compiles the entry in Typst's experimental *bundle* target, which
is the only target that can emit many documents and assets from one
project.

== Semantic regions

Chrome and content are separated: the stdlib wraps the page body in
`<main id="typost-content">` before the layout runs, so a plugin can find
exactly the content later.

== Eager show rules

A selectorless show rule like `#show: page.with(...)` is applied during
*evaluation*, not layout. That is why the page's `metadata` is available to
typost's pre-render manifest pass.

#callout[
  This separation and timing are what will let the encryption plugin lock a
  single post's content without touching the surrounding page.
]

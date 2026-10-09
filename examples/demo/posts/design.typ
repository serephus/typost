#import "../theme.typ": page, callout

#show: page.with(
  route: "blog/design/index.html",
  title: "The design of typost",
  date: "2026-02-10",
  tags: ("typost", "design"),
  description: "A site is a Typst program.",
  url: "/blog/design/",
  section: "blog",
)

= The design of typost

typost turns a Typst program into a website. Typst is the authoring *and*
structure language: there is no config DSL, no front-matter format, and no
content convention.

== The entry is the site

The entry file is a Typst program. It declares the site's metadata and
`#include`s every page — nothing is discovered from filenames:

```typst
#import "lib/typost.typ": site

#site(title: "typost demo")

#include "pages/home.typ"
#include "pages/about.typ"
#include "pages/blog.typ"
```

== A page is a show rule

Each page is its own Typst file. It starts with a selectorless show rule that
picks a layout; the rest of the file is the body, and named arguments other
than `route`, `layout`, `title`, and `tags` become its front matter:

```typst
#import "lib/typost.typ": page

#show: page.with(route: "about/index.html", title: "About")

= About
...
```

A layout is called as `layout(route, data, content)`, so the theme can render
the nav, title, and byline around the body.

== One render, many documents

typost compiles the entry in Typst's experimental *bundle* target: a single
compilation emits every document and asset. Because the pages share that
bundle, they can query each other. The lists on the #link("/")[home] and
#link("/blog/")[blog] pages are not maintained by hand:

```typst
#context {
  for m in query(metadata).filter(m => /* …section == "blog"… */) [
    #link(m.value.typost.data.url, m.value.typost.data.title)
  ]
}
```

== Plugins and the pipeline

Everything optional is a plugin. A plugin can contribute Typst modules, mutate
the site manifest before render, and rewrite the exported files. They run at
fixed stages:

```text
config → typst(plugins) → eval-only → prepare(plugins) → render → post(plugins) → emit
```

This demo uses five:

- *sitemap* — writes `sitemap.xml`;
- *feed* — writes an Atom feed and links it from every page;
- *encrypt* — encrypts a content region at build time;
- *taxonomies* — generates a page per tag/category;
- *spoiler* — hover- and click-to-reveal regions.

Sites and plugins communicate through a `metadata((typost: …))` channel that is
lifted into the manifest before rendering, so the core stays free of any
plugin's concepts.

#callout[
  The #link("/blog/tour/")[feature tour] shows the same ideas rendered, with
  each snippet next to its result.
]

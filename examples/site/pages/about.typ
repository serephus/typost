#import "../theme.typ": page

#show: page.with(
  route: "about/index.html",
  title: "About",
  description: "About this demo.",
)

= About

This example shows typost turning a Typst program into a website.

== How a page is defined

Each page is its own file. It starts with a selectorless show rule and then
its content:

```typst
#import "../theme.typ": page
#show: page.with(route: "about/index.html", title: "About")
= About
...
```

The entry file `#include`s the page files. There is no filename convention.

== Typst features on display

- Multi-page, multi-section structure declared in `home.typ`
- Post indexes built from `query(metadata)` (no hand-maintained list)
- Code blocks, math, tables, quotes, footnotes, term lists
- Reusable components (`callout`, `post-meta`) and a site layout

Read the #link("/blog/")[blog] for the rest.

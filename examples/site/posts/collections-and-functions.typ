#import "../theme.typ": page, callout

#show: page.with(
  route: "blog/collections-and-functions/index.html",
  title: "Collections and functions",
  date: "2026-02-14",
  tags: ("typst", "functions"),
  description: "Values, loops, and reusable components.",
  url: "/blog/collections-and-functions/",
  section: "blog",
)

= Collections and functions

Typst is a programming language. Shared helpers live in `theme.typ` and
are imported where they are needed.

== A generated list

#let points = ("one", "two", "three")
#enum(..points.map(point => [#point]))

== Building indexes from the manifest

The home and blog pages do not carry a hand-maintained list. They query the
bundle's metadata:

```typst
#context {
  for m in query(metadata).filter(/* …typost kind == "page"… */) [
    #link(m.value.typost.data.url, m.value.typost.data.title)
  ]
}
```

== A callout

#callout[
  This is an `<aside class="callout">`, produced by a Typst function.
]

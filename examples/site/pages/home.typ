#import "../theme.typ": page, callout, post-meta

#show: page.with(
  route: "index.html",
  title: "Home",
  description: "A typost demo site.",
)

= A Typst-powered site

Everything here — structure, layout, and content — is written in Typst.

#callout[
  This demo is multi-page and multi-section. The entry file includes each
  page; the post lists are built by querying the bundle's metadata.
]

== Recent posts

#context {
  let posts = query(metadata).filter(m => {
    let v = m.value
    type(v) == dictionary and "typost" in v and v.typost.kind == "page" and v.typost.data.at("section", default: none) == "blog"
  })
  for m in posts [
    #html.elem("article", [
      #html.elem("h3", link(m.value.typost.data.url, m.value.typost.data.title))
      #post-meta(
        date: m.value.typost.data.date,
        tags: m.value.typost.data.tags,
      )
    ])
  ]
}

See the #link("/blog/")[full blog index] or the #link("/about/")[about page].

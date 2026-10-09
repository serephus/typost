#import "../theme.typ": page, post-meta

#show: page.with(
  route: "blog/index.html",
  title: "Blog",
  description: "All posts.",
)

= Blog

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
      #m.value.typost.data.description
    ])
  ]
}

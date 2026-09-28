// Multi-taxonomy support (tags, categories, difficulty, ...).
//
// A page's taxonomies live in its front matter. The site declares the
// taxonomies it wants as `(key, route, title)` records and calls
// `render-taxonomies` from its entry; the theme uses `term-links` to render a
// page's terms.

/// Slugify a term for use in a route.
#let slug(name) = lower(name).replace(" ", "-")

/// Coerce a front-matter value into an array of terms.
#let as-terms(value) = {
  if value == none { () }
  else if type(value) == array { value }
  else { (value,) }
}

/// The links for one page's terms, as an array of `(route, name)` records.
#let term-links(data, specs) = {
  let items = ()
  for spec in specs {
    for term in as-terms(data.at(spec.key, default: none)) {
      items += ((route: spec.route + "/" + slug(term) + "/", name: term),)
    }
  }
  items
}

/// Emit the index and term pages for every declared taxonomy.
///
/// `specs` is an array of `(key, route, title)` records. `render` is called as
/// `render(route, title, body)`.
#let render-taxonomies(specs, render) = context {
  let pages = query(metadata).filter(m => {
    let v = m.value
    type(v) == dictionary and "typost" in v and v.typost.kind == "page"
  }).map(m => m.value.typost)

  for spec in specs {
    // Group the pages by this taxonomy's terms.
    let groups = (:)
    for page in pages {
      for term in as-terms(page.data.at(spec.key, default: none)) {
        groups.insert(term, groups.at(term, default: ()) + (page,))
      }
    }
    let names = groups.keys().sorted()

    // The taxonomy index lists every term and its page count.
    render(spec.route + "/index.html", spec.title, html.elem(
      "ul",
      attrs: (class: "taxonomy-terms"),
      [
        #for name in names [
          #html.elem("li", [
            #html.elem("a", attrs: (href: "/" + spec.route + "/" + slug(name) + "/"), [#name])
            #html.elem("span", attrs: (class: "count"), [#groups.at(name).len()])
          ])
        ]
      ],
    ))

    // One page per term, listing the pages that carry it.
    for name in names {
      render(spec.route + "/" + slug(name) + "/index.html", name, html.elem(
        "ul",
        attrs: (class: "content-list"),
        [
          #for page in groups.at(name) [
            #html.elem("li", attrs: (class: "title-list"), [
              #html.elem("a", attrs: (href: "/" + page.route, class: "content-link"), [
                #page.data.at("title", default: "")
              ])
            ])
          ]
        ],
      ))
    }
  }
}

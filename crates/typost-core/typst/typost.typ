// The typost stdlib.
//
// This file is embedded in `typost-core` and materialized into a project's
// `lib/` directory at build/init time. It stays plain Typst and works with
// `typst compile`.

/// Declare site-level metadata.
#let site(title: none) = metadata((
  typost: (kind: "site", title: title),
))

/// Define one page.
///
/// Use it as a selectorless show rule at the top of an included file; the rest
/// of the file becomes the page body:
///
/// ```typst
/// #import "lib/typost.typ": page
/// #show: page.with(route: "hello/index.html", title: "Hello", tags: ("intro",))
/// = Hello
/// ...content...
/// ```
///
/// Named arguments other than `route`, `layout`, `title`, and `tags` become the
/// page's front matter. `layout` is called as `layout(route, data, content)`.
///
/// Selectorless show rules are applied eagerly during evaluation, so the
/// `metadata` below is visible to typost's pre-render manifest pass.
#let page(body, route: none, layout: none, title: none, tags: (), ..extra) = {
  let data = (title: title, tags: tags, ..extra.named())
  metadata((
    typost: (kind: "page", route: route, data: data),
  ))
  let content = html.elem("main", body, attrs: (id: "typost-content"))
  let rendered = if layout == none { content } else { layout(route, data, content) }
  document(route, rendered, format: "html", title: title)
}

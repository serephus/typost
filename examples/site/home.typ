// The entry: a Typst program that declares the whole site.
//
// Each page is a separate file whose first statement is
// `#show: page.with(route: ..., title: ...)`. This file includes them, in
// order. There is no filesystem convention.

#import "lib/typost.typ": site

#site(title: "typost demo")

#include "pages/home.typ"
#include "pages/about.typ"
#include "pages/blog.typ"

#include "posts/structure-in-typst.typ"
#include "posts/collections-and-functions.typ"
#include "posts/notes-on-html-export.typ"
#include "posts/secret-notes.typ"

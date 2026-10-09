#import "../theme.typ": page, callout
#import "../lib/typost/spoiler.typ": spoiler
#import "../lib/typost/encrypted.typ": encrypted

#show: page.with(
  route: "blog/tour/index.html",
  title: "A tour of Typst and typost",
  date: "2026-02-14",
  tags: ("typst", "typost", "features"),
  description: "Every snippet next to its result.",
  url: "/blog/tour/",
  section: "blog",
)

= A tour of Typst and typost

Every block below shows the source, then the actual result.

#divider()

== Headings

```typst
=== A section
```

=== A section

```typst
==== A subsection
```

==== A subsection

#divider()

== Emphasis and inline styles

```typst
*bold*, _italic_, `code`, #strike[struck], #underline[underlined],
#highlight[highlighted], #sub[sub], #super[super]
```

*bold*, _italic_, `code`, #strike[struck], #underline[underlined],
#highlight[highlighted], #sub[sub], #super[super]

#divider()

== Lists

```typst
- one
- two
  - nested

+ first
+ second

/ term: its description
```

- one
- two
  - nested

+ first
+ second

/ term: its description

#divider()

== Code

````typst
```rust
fn main() {
    println!("hello from typost");
}
```
````

```rust
fn main() {
    println!("hello from typost");
}
```

#divider()

== Math

```typst
Euler's identity: $e^(i pi) + 1 = 0$.
```

Euler's identity: $e^(i pi) + 1 = 0$.

```typst
$ integral_0^infinity e^(-x^2) dif x = sqrt(pi) / 2 $
```

$ integral_0^infinity e^(-x^2) dif x = sqrt(pi) / 2 $

#divider()

== Tables

```typst
#table(
  columns: 3,
  [*Feature*], [*Typst*], [*Markdown*],
  [Math], [yes], [partial],
  [Functions], [yes], [no],
)
```

#table(
  columns: 3,
  [*Feature*], [*Typst*], [*Markdown*],
  [Math], [yes], [partial],
  [Functions], [yes], [no],
)

#divider()

== Quotes and footnotes

```typst
#quote(block: true)[
  Typst is a new markup-based typesetting system that is designed to be as
  powerful as LaTeX while being much easier to learn and use.
]

A footnote#footnote[At the bottom of the page.].
```

#quote(block: true)[
  Typst is a new markup-based typesetting system that is designed to be as
  powerful as LaTeX while being much easier to learn and use.
]

A footnote#footnote[At the bottom of the page.].

#divider()

== typost extras

A spoiler hides an inline run until you hover it:

```typst
The butler did it #spoiler[with the candlestick, in the library].
```

The butler did it #spoiler[with the candlestick, in the library].

A block spoiler stays blurred until you click it, then stays revealed:

```typst
#spoiler(block: true)[
  This block stays blurred behind a "click to reveal" pill until you click it,
  and then it stays revealed.
]
```

#spoiler(block: true)[
  This block stays blurred behind a "click to reveal" pill until you click it,
  and then it stays revealed.
]

A region can be encrypted at build time — the plaintext never reaches the page:

```typst
#encrypted(password: "hunter2")[
  Only readers who know the password see this.
  The plaintext is not in the page source.
]
```

#encrypted(password: "hunter2")[
  Only readers who know the password see this.
  The plaintext is not in the page source.
]

# typost — design

A static site generator in Rust that treats **Typst as the site's authoring and
structure language**, with compile-time, trait-backed Rust plugins.

This document is the pinned design. It is intentionally opinionated; see
"Decisions" at the end for the reasoning.

## Goals

1. **As much as possible in Typst.** Content, templates, navigation, and the
   overall site structure are written in Typst.
2. **Compile-time plugins.** Flexibility comes from Rust traits composed at
   compile time — not WASM, not dynamic loading.
3. **Pipelines with predefined inputs and outputs.** The build is a fixed
   sequence of typed stages; plugins hook into fixed seams.
4. **Tiny core.** `typost-core` provides only the machinery. Every optional
   feature (tags, feeds, encryption, …) is a plugin.

Non-goals for now: output formats other than HTML; URL rewriting; runtime
plugin loading; API stability.

## The model: a site is a Typst program

There is **no filesystem convention** and no `_index.typ` magic. A site is a
single Typst entry file compiled in Typst's `bundle` target:

- Each page is its own file. Its first statement is a selectorless show
  directive; everything after it is the page body.
- The entry (`home.typ`) `#include`s the page files, in order. There is no
  filename convention.
- `.typ` page files and the `lib/` stdlib are ordinary Typst: they compile
  and preview with plain `typst` and Tinymist. The entry is a *bundle*
  program (it uses Typst's built-in `#document`), so it is built by `typost`;
  in Tinymist, preview individual page files.

The page contract: a file starts with `#show: page.with(...)` and then its
content flows.

```typst
// posts/hello.typ
#import "../theme.typ": page

#show: page.with(
  route: "blog/hello/index.html",
  title: "Hello",
  tags: ("intro",),
  date: "2026-01-01",
)

= Hello
...content flows...
```

```typst
// home.typ
#import "lib/typost.typ": site

#site(title: "Example")

#include "posts/hello.typ"
```

Named arguments other than `route`, `layout`, `title`, and `tags` become the
page's front matter. The layout is called as `layout(route, data, content)` so
theme chrome (and things like a post byline) can use the metadata while staying
outside the content region.

Two Typst facts make this work:

- **Selectorless show rules run during evaluation** (`Content::styled_with_recipe`
  applies eagerly when the recipe has no selector), so the `metadata` emitted by
  `page` is visible to the pre-render manifest pass.
- **`context { query(metadata) }` sees the whole bundle** (one shared
  `BundleIntrospector`), so an index page can list every page without a
  hand-maintained list.

The cost of the include model is that `#include` paths are static, so pages are
not discovered by scanning the filesystem. Pages generated from data can still
call `page` directly inside a loop in the entry.

## Why this maps onto Typst (internals)

- Compilation is `parse → eval (Content tree) → realize/layout → export`.
  There is **no stable public AST-transform stage**; plugins do not rewrite
  content. They act by injecting Typst definitions/values into the `World`
  before eval, or by transforming exported bytes after export.
- The `bundle` target is the only target that emits many documents and assets
  from one compilation, with one shared introspector and cross-document link
  resolution. It is the right render engine for an SSG.
- HTML export is experimental and semantically constrained. Themes live in
  Typst; Rust post-processing stays light (head injection, byte transforms).
- Output paths are chosen *inside Typst* and links resolve against them. Any
  routing decision therefore happens before render.

## Crates

```
typost-core   engine: World, manifest, plugin trait, pipeline. Minimal.
typost        binary: CLI, wires the built-in plugins.
typost-plugin-sitemap / -encrypt / -taxonomies / -feed
              one crate per first-party plugin.
```

## Pipeline (fixed stages, typed I/O)

```
Config ─▶ typst(plugins) ─▶ eval-only ─▶ prepare(plugins) ─▶ render Bundle ─▶ post(plugins) ─▶ emit
```

| Stage | Input | Output |
|---|---|---|
| Config | `typost.toml` | `Config` |
| `Plugin::typst` | — | `TypstOverlay` (extra `.typ` sources for the `World`) |
| eval-only | World | `SiteManifest` |
| `Plugin::prepare` | `SiteManifest` | mutated manifest (routing, derived metadata, validation) |
| render | World + manifest | `Bundle` → exported `VirtualFs` |
| `Plugin::post` | `RenderOutput` + manifest | transformed files + extra assets |
| emit | final files | disk |

The **eval-only pass** runs the entry through Typst evaluation (no layout) and
lifts a typed manifest out of the evaluated `Content` tree. It is what makes the
manifest available *before* rendering. The manifest element is a no-op during
the real render.

## Manifest

Typed Rust structures. Not one generic type; not a JSON artifact; no
plugin-specific fields.

```rust
pub struct SiteManifest {
    pub title: Option<String>,
    pub pages: Vec<PageMeta>,
    pub entries: Vec<MetadataEntry>,   // generic, plugin-defined
}

pub struct PageMeta {
    pub route: String,          // output path, e.g. "hello/index.html"
    pub src: Option<PathBuf>,   // provenance, for diagnostics
    pub data: FrontMatter,      // decoded from the Typst `typost` dict
}

pub struct MetadataEntry {
    pub kind: String,           // e.g. "encrypted"
    pub route: Option<String>,  // enclosing document, if any
    pub data: FrontMatter,
}
```

`FrontMatter` is a plain Rust tree (`Bool/Int/Float/Str/Array/Dict`) decoded
from the Typst value, so the manifest is ordinary, thread-safe Rust. Plugins
interpret `data` (for example, a tags plugin reads `data.tags`; the encryption
plugin reads `data.password`). Core never mentions those keys.

The manifest is produced by markers emitted by the Typst helpers: each helper
wraps its descriptors in a `metadata((typost: (...)))` value; the eval pass
walks the content tree and collects them. `kind: "site"` and `kind: "page"` are
core; **any other kind is passed through verbatim** as a `MetadataEntry`, tagged
with the route of the enclosing `document`. That is the generic channel plugins
use to carry data from Typst to Rust without it ever appearing in the output —
for example, the password of an `#encrypted` region.

## Plugins

```rust
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;

    /// Contribute Typst modules/functions into the World before eval.
    fn typst(&self, _overlay: &mut TypstOverlay) -> anyhow::Result<()> { Ok(()) }

    /// Mutate the structure/metadata after eval, before render.
    fn prepare(&self, _manifest: &mut SiteManifest) -> anyhow::Result<()> { Ok(()) }

    /// Transform exported bytes after render.
    fn post(
        &self,
        _out: &mut RenderOutput,
        _manifest: &SiteManifest,
    ) -> anyhow::Result<()> { Ok(()) }
}
```

Plugins are registered in Rust, in `typost/src/main.rs::register_plugins`, and
receive their options from `typost.toml`:

```rust
let mut site = site;
if let Some(sitemap) = &config.sitemap {
    site = site.plugin(Sitemap::new(&sitemap.base_url));
}
site
```

## Helper library

Core ships its `.typ` stdlib embedded in `typost-core`; `typost` materializes
it (and each plugin's embedded Typst modules) into the project's `lib/`
directory on `init`/`build`. Entries import relative paths, so the generated
files are visible to Tinymist and plain `typst`; builds also carry the same
sources in an in-memory overlay, so they do not depend on the on-disk copy.
The `lib/` directory is generated and overwritten on every build, so it should
be gitignored. Publishing the core stdlib to Typst Universe (`@preview/typost`)
as an *optional* alternative is a later addition; plugin-provided modules
always come from local materialization.

## Config

- Engine/plugin configuration: `typost.toml`.
- Site/page/section metadata: arguments to `site` and `page` (front matter,
  carried in the `typost` metadata).

## Content region

The stdlib `page` helper wraps each page's body in a marker *before* applying
the layout:

```typst
#html.elem("main", attrs: (id: "typost-content"))[#body]
```

This is a generic "content region" convention (useful to any plugin), not an
encryption-specific field in the manifest. Because the layout receives the
already-marked content, theme chrome (header, nav, footer) stays outside the
region. Plugins that need "just the content" locate this element in the
exported HTML.

## Encryption (plugin)

Build-time encrypt, browser decrypt. Two forms:

```typst
// encrypt the rest of the file (selectorless show rule)
#show: encrypted.with(password: "...", hint: "...")
...content...

// or encrypt a block
#encrypted(password: "...", hint: "...")[ ...region... ]
```

`hint` is the prompt shown in the lock UI (default: "This part is encrypted.
Enter the password to read it."). It may be a string or content, so it can be
rich text — emphasis, links, and so on. `password` is required.

The prompt is rendered by Typst, not assembled from a manifest string: the
helper wraps it in a `<template class="typost-hint">` before each region, and
the plugin lifts those out (in document order) into the lock UI, then removes
them from the output.

The plugin runs as `Plugin::post`. It replaces each region's contents with a
lock form and inlines the ciphertext plus a small decrypt shim. Theme chrome and
the page `<head>` stay in plaintext.

The region password travels through the generic metadata channel (a `metadata`
entry of kind `"encrypted"`, tagged with the page route), so it is never
written to the output. Regions and their passwords are paired in document order.

- Password first; YubiKey (WebAuthn PRF) later.
- Per-page secrets, independent keys.
- Cryptographic unit: the content region only (`<main id="typost-content">`),
  preserving the page `<head>` and chrome.
- KDF: PBKDF2-HMAC-SHA256 (default 600,000 iterations) with a 16-byte random
  salt; AES-256-GCM with a 12-byte random nonce. Decryption uses WebCrypto, so
  it needs a secure context (HTTPS or localhost) — not `file://`. Argon2id via
  a small WASM module is the later upgrade.
- The ciphertext is base64-inlined in the page and decrypted by an inline
  script via `crypto.subtle`; no extra request and no base-path issues. A
  shared external shim asset is a possible later optimization.
- Threat model: plaintext passwords in Typst source are permitted by the
  author. With a public repository this is a speed bump, not secrecy. A real
  KDF is still used so the *rendered ciphertext* is not trivially reversible
  and reuse of a password elsewhere is not instantly exposed.

## Taxonomies (plugin)

A site can have more than one kind of tag — `tags`, `categories`,
`difficulty`, and so on. The plugin contributes `lib/typost/taxonomies.typ`;
the site declares its taxonomies in Typst and the module generates the pages:

```typst
// theme.typ
#let taxonomies = (
  (key: "tags", route: "tags", title: "Tags"),
  (key: "categories", route: "categories", title: "Categories"),
)

// main.typ (entry), after the page includes
#import "theme.typ": taxonomies, render-taxonomy-page
#render-taxonomies(taxonomies, render-taxonomy-page)
```

A page opts in by declaring the matching key in its front matter
(`tags: ("a", "b")`, `difficulty: "Medium"`); the theme calls `term-links`
to render pills for a page's terms. `render-taxonomies` emits, for each
taxonomy, an `<route>/index.html` and one `<route>/<term>/index.html` per term.

This one is almost entirely Typst: routes must exist before the single bundle
render, so the pages are produced during evaluation inside a `context` block
that groups pages from `query(metadata)`. Those pages are therefore absent from
the pre-render manifest, which is why the `Sitemap` plugin reads its routes
from the rendered output instead.

## Feed (plugin)

An Atom 1.0 feed of a section's dated pages (default `post`), newest first,
written to `atom.xml` and linked from every page's `<head>`:

```toml
[feed]
limit = 20          # default
section = "post"    # default
```

It is a `post` plugin: it reads each entry's rendered `<main id="typost-content">`
region, makes root-relative URLs absolute, and embeds it as `content type="html"`.
Register it after `Encrypt` so encrypted posts contribute ciphertext, not
plaintext. `base_url` is shared with `Sitemap` via the top-level config key.

## Decisions

- **Bundle render, HTML only.** The bundle target is the only multi-document
  target; other formats are out of scope.
- **Manifest before render, via eval-only pass.** Structure must be known
  before layout; links resolve against Typst-chosen paths.
- **No AST mutation.** Typst has no stable public transform stage, so plugins
  inject into the World or transform bytes.
- **Tiny core.** Anything domain-specific lives in a plugin.
- **Core content-region marker.** Lets content-scoped plugins work without
  coupling to a theme.

## Open questions

- Argon2id (WASM) and YubiKey (WebAuthn PRF) decryption methods.
- Concrete routing/pretty-URL helpers in the Typst stdlib.
- Incremental builds and a dev server.


/// Hide `body` behind a spoiler.
///
/// Inline spoilers (the default) reveal on hover or keyboard focus:
///
/// ```typst
/// The butler did it: #spoiler[Colonel Mustard, in the library].
/// ```
///
/// Block spoilers (`block: true`) stay blurred behind a `hint`; clicking (or
/// pressing enter) reveals them, and they stay revealed.
///
/// ```typst
/// #spoiler(block: true)[
///   ...a hidden block (paragraphs, lists, code, ...)...
/// ]
/// ```
#let spoiler(body, block: false, hint: "click to reveal") = {
  if block {
    html.elem(
      "div",
      [
        #html.elem("span", hint, attrs: (class: "typost-spoiler-hint"))
        #html.elem("div", body, attrs: (class: "typost-spoiler-content"))
      ],
      attrs: (class: "typost-spoiler-block", tabindex: "0"),
    )
  } else {
    html.elem("span", body, attrs: (class: "typost-spoiler", tabindex: "0"))
  }
}

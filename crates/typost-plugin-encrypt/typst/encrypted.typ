
/// Mark a region whose content should be encrypted at build time.
///
/// Use it either as a selectorless show rule (encrypts the rest of the file):
///
/// ```typst
/// #show: encrypted.with(password: "pw", hint: "Ask me for the password.")
/// ...content...
/// ```
///
/// or around a block:
///
/// ```typst
/// #encrypted(password: "pw", hint: "...")[ ...content... ]
/// ```
///
/// `hint` may be a string or content, so the prompt can be rich text (emphasis,
/// links, ...). It is rendered here and lifted into the lock UI by the plugin.
///
/// The password travels through the manifest (never into the output). The
/// plugin replaces the region's contents with a lock UI and inline ciphertext;
/// the browser decrypts it with WebCrypto.
#let encrypted(body, password: none, hint: none) = {
  assert(
    password != none and password != "",
    message: "encrypted: a non-empty `password` is required",
  )
  metadata((typost: (kind: "encrypted", password: password)))
  // An empty template means "use the default prompt".
  html.elem(
    "template",
    if hint == none { [] } else { hint },
    attrs: (class: "typost-hint"),
  )
  html.elem("div", body, attrs: (class: "typost-encrypted"))
}

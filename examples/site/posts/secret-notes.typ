#import "../theme.typ": page, callout, encrypted

#show: page.with(
  route: "blog/secret-notes/index.html",
  title: "Locked notes",
  date: "2026-02-24",
  tags: ("private",),
  description: "A page with an encrypted region.",
  url: "/blog/secret-notes/",
  section: "blog",
)

= Locked notes

This paragraph is public: anyone can read it.

// Everything after this directive is encrypted, to the end of the file.
#show: encrypted.with(
  password: "hunter2",
  hint: [This note is *private* — ask me on #link("https://t.me/serephus")[Telegram].],
)

== The locked part

This region is encrypted at build time and decrypted in the browser with
WebCrypto. Its plaintext is not in the output.

Unicode survives the round-trip too: 你好，世界 — café ☃.

#callout[
  View the page source: the text below does not appear. Only the ciphertext
  and a small decrypt shim ship, for this region alone.
]

The password for this demo is `hunter2` (it is public, so it is only a
speed bump).

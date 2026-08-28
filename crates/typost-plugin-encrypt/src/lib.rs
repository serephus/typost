//! Build-time page encryption.
//!
//! A page opts in by declaring a `password` in its front matter. After render,
//! this plugin encrypts the page's content region (`<main id="typost-content">`)
//! and replaces it with a lock form plus the ciphertext. The plaintext never
//! reaches the output; a small inline script decrypts it in the browser with
//! WebCrypto (PBKDF2-HMAC-SHA256 + AES-256-GCM).
//!
//! Threat model: a password written in Typst source is public if the repository
//! is public, so this is a speed bump, not secrecy. A real KDF is still used so
//! the rendered ciphertext is not trivially reversible and reusing a password
//! elsewhere is not instantly exposed.

use std::collections::BTreeMap;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use typost_core::{FrontMatter, Plugin, RenderOutput, SiteManifest, TypstOverlay};

/// Default PBKDF2 iteration count.
const DEFAULT_ITERATIONS: u32 = 600_000;
/// Salt length in bytes.
const SALT_LEN: usize = 16;
/// AES-GCM nonce length in bytes.
const NONCE_LEN: usize = 12;

/// The encryption plugin.
pub struct Encrypt {
    iterations: u32,
}

impl Default for Encrypt {
    fn default() -> Self {
        Self::new()
    }
}

impl Encrypt {
    /// Create the plugin with the default KDF cost.
    pub fn new() -> Self {
        Self {
            iterations: DEFAULT_ITERATIONS,
        }
    }

    /// Create the plugin with a custom PBKDF2 iteration count.
    pub fn with_iterations(iterations: u32) -> Self {
        Self { iterations }
    }
}

impl Plugin for Encrypt {
    fn name(&self) -> &str {
        "encrypt"
    }

    fn typst(&self, overlay: &mut TypstOverlay) -> Result<()> {
        overlay.add("lib/typost/encrypted.typ", ENCRYPTED_TYP);
        Ok(())
    }

    fn post(&self, out: &mut RenderOutput, manifest: &SiteManifest) -> Result<()> {
        // Inline `#encrypted(...)` regions, grouped by page in document order.
        let mut regions: BTreeMap<String, Vec<RegionLock>> = BTreeMap::new();
        for entry in &manifest.entries {
            if entry.kind != "encrypted" {
                continue;
            }
            let Some(route) = &entry.route else {
                continue;
            };
            let Some(password) = entry.data.get("password").and_then(FrontMatter::as_str) else {
                continue;
            };
            if password.is_empty() {
                continue;
            }
            let hint = entry
                .data
                .get("hint")
                .and_then(FrontMatter::as_str)
                .map(str::to_owned);
            regions.entry(route.clone()).or_default().push(RegionLock {
                password: password.to_owned(),
                hint,
            });
        }

        for (route, locks) in &regions {
            let html = out
                .get(route)
                .map(<[u8]>::to_vec)
                .with_context(|| format!("encrypt: page `{route}` was not rendered"))?;
            let html = String::from_utf8(html)
                .with_context(|| format!("encrypt: page `{route}` is not UTF-8"))?;
            let encrypted = encrypt_regions(&html, locks, self.iterations)?;
            out.insert(route.clone(), encrypted.into_bytes());
        }

        Ok(())
    }
}

/// One `#encrypted(...)` region: its password and optional prompt text.
struct RegionLock {
    password: String,
    hint: Option<String>,
}

/// Encrypt every inline region in a page, pairing regions with `locks` in
/// document order.
fn encrypt_regions(html: &str, locks: &[RegionLock], iterations: u32) -> Result<String> {
    const OPEN: &str = "<div class=\"typost-encrypted\">";
    const CLOSE: &str = "</div>";

    let mut result = String::with_capacity(html.len());
    let mut cursor = 0;

    for (index, lock) in locks.iter().enumerate() {
        let Some(offset) = html[cursor..].find(OPEN) else {
            bail!(
                "encrypt: expected {} encrypted region(s), found {}",
                locks.len(),
                index
            );
        };
        let start = cursor + offset;
        let inner_start = start + OPEN.len();
        let end = find_matching_div_close(html, start)
            .with_context(|| "encrypt: unterminated encrypted region")?;
        let inner_end = end - CLOSE.len();

        let locked = encrypt_bytes(
            &lock.password,
            &html.as_bytes()[inner_start..inner_end],
            iterations,
        )?;
        result.push_str(&html[cursor..inner_start]);
        result.push_str(&lock_html(&locked, lock.hint.as_deref()));
        cursor = inner_end;
    }

    if html[cursor..].contains(OPEN) {
        bail!("encrypt: more encrypted regions than passwords");
    }

    result.push_str(&html[cursor..]);
    Ok(result)
}

/// Find the byte index just past the `</div>` matching the `<div>` at `start`.
fn find_matching_div_close(html: &str, start: usize) -> Option<usize> {
    const CLOSE: &str = "</div>";
    let mut depth = 0usize;
    let mut pos = start;
    while pos < html.len() {
        if html[pos..].starts_with("<div") {
            depth += 1;
            pos += 4;
        } else if html[pos..].starts_with(CLOSE) {
            depth -= 1;
            if depth == 0 {
                return Some(pos + CLOSE.len());
            }
            pos += CLOSE.len();
        } else {
            pos += 1;
        }
    }
    None
}

/// The plugin's Typst helper. It is materialized into `lib/typost/encrypted.typ`
/// so pages can `#import` it.
const ENCRYPTED_TYP: &str = r#"
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
/// The password travels through the manifest (never into the output). The
/// plugin replaces the region's contents with a lock UI and inline ciphertext;
/// the browser decrypts it with WebCrypto.
#let encrypted(body, password: none, hint: none) = {
  assert(
    password != none and password != "",
    message: "encrypted: a non-empty `password` is required",
  )
  metadata((typost: (kind: "encrypted", password: password, hint: hint)))
  html.elem("div", body, attrs: (class: "typost-encrypted"))
}
"#;

/// A locked payload.
struct Locked {
    salt: [u8; SALT_LEN],
    nonce: [u8; NONCE_LEN],
    iterations: u32,
    ciphertext: Vec<u8>,
}

/// Derive a key from `password` and encrypt `plaintext`.
fn encrypt_bytes(password: &str, plaintext: &[u8], iterations: u32) -> Result<Locked> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut salt).map_err(|err| anyhow!("encrypt: rng failed: {err}"))?;
    getrandom::fill(&mut nonce).map_err(|err| anyhow!("encrypt: rng failed: {err}"))?;

    let key = derive_key(password, &salt, iterations);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| anyhow!("encrypt: bad key"))?;
    let ciphertext = cipher
        .encrypt(&Nonce::from(nonce), plaintext)
        .map_err(|_| anyhow!("encrypt: encryption failed"))?;

    Ok(Locked {
        salt,
        nonce,
        iterations,
        ciphertext,
    })
}

/// Derive a key and decrypt a payload. Used by tests and as the reference
/// implementation for the browser shim.
#[cfg(test)]
fn decrypt_bytes(password: &str, locked: &Locked) -> Result<Vec<u8>> {
    let key = derive_key(password, &locked.salt, locked.iterations);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| anyhow!("decrypt: bad key"))?;
    cipher
        .decrypt(&Nonce::from(locked.nonce), locked.ciphertext.as_ref())
        .map_err(|_| anyhow!("decrypt: wrong password or corrupt ciphertext"))
}

/// PBKDF2-HMAC-SHA256 -> 32-byte key.
fn derive_key(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<pbkdf2::sha2::Sha256>(password.as_bytes(), salt, iterations, &mut key);
    key
}

/// The default prompt shown in the lock UI.
const DEFAULT_HINT: &str = "This part is encrypted. Enter the password to read it.";

/// Build the lock markup that replaces the plaintext content.
fn lock_html(locked: &Locked, hint: Option<&str>) -> String {
    let salt = B64.encode(locked.salt);
    let iv = B64.encode(locked.nonce);
    let cipher = B64.encode(&locked.ciphertext);
    let iterations = locked.iterations;
    let params = format!("{{\"salt\":\"{salt}\",\"iv\":\"{iv}\",\"iterations\":{iterations}}}");
    let hint = escape_html(hint.unwrap_or(DEFAULT_HINT));

    let mut html = String::with_capacity(
        params.len() + cipher.len() + LOCK_STYLE.len() + LOCK_SCRIPT.len() + 512,
    );
    html.push_str("<div class=\"typost-lock\">");
    html.push_str(LOCK_STYLE);
    html.push_str("<form class=\"typost-lock-form\">");
    html.push_str("<p class=\"typost-lock-hint\">");
    html.push_str(&hint);
    html.push_str("</p>");
    html.push_str(
        "<input type=\"password\" class=\"typost-lock-password\" \
         autocomplete=\"current-password\" placeholder=\"Password\" autofocus>",
    );
    html.push_str("<button type=\"submit\">Unlock</button>");
    html.push_str("<p class=\"typost-lock-error\" role=\"alert\" hidden>Wrong password.</p>");
    html.push_str("</form>");
    html.push_str("<script type=\"application/json\" class=\"typost-lock-params\">");
    html.push_str(&params);
    html.push_str("</script>");
    html.push_str("<script type=\"text/plain\" class=\"typost-lock-cipher\">");
    html.push_str(&cipher);
    html.push_str("</script>");
    html.push_str("<script>");
    html.push_str(LOCK_SCRIPT);
    html.push_str("</script>");
    html.push_str("</div>");
    html
}

/// Escape text for safe inclusion in HTML.
fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

const LOCK_STYLE: &str = "<style>\
.typost-lock{max-width:26rem;margin:3rem auto;padding:1.5rem;text-align:center;\
background:var(--surface,#32302f);border:1px solid var(--surface-2,#3c3836);\
border-radius:var(--radius,10px);box-shadow:0 12px 32px -24px rgba(0,0,0,.8)}\
.typost-lock-hint{margin:0 0 1rem;color:var(--fg-dim,#a89984);font-size:.95rem}\
.typost-lock form{display:flex;flex-direction:column;gap:.6rem}\
.typost-lock input{width:100%;box-sizing:border-box;padding:.6rem .8rem;\
background:var(--bg-soft,#282828);color:var(--fg,#d4be98);\
border:1px solid var(--border,#504945);border-radius:8px;font:inherit;outline:none}\
.typost-lock input:focus{border-color:var(--accent,#a9b665)}\
.typost-lock button{padding:.6rem 1.2rem;border:0;border-radius:8px;\
cursor:pointer;background:var(--accent,#a9b665);color:var(--bg,#1d2021);\
font:inherit;font-weight:600}\
.typost-lock button:hover{opacity:.9}\
.typost-lock-error{margin:.2rem 0 0;color:var(--red,#ea6962);font-size:.9rem}\
</style>";

const LOCK_SCRIPT: &str = r#"
(function () {
  var lock = document.currentScript.closest('.typost-lock');
  if (!lock) return;
  var params = JSON.parse(lock.querySelector('.typost-lock-params').textContent);
  var cipherB64 = lock.querySelector('.typost-lock-cipher').textContent.trim();
  var form = lock.querySelector('form');
  var input = lock.querySelector('.typost-lock-password');
  var error = lock.querySelector('.typost-lock-error');
  function bytes(b64) {
    return Uint8Array.from(atob(b64), function (c) { return c.charCodeAt(0); });
  }
  if (!window.crypto || !window.crypto.subtle) {
    error.hidden = false;
    error.textContent = 'WebCrypto is unavailable; serve this site over http(s).';
    return;
  }
  form.addEventListener('submit', function (event) {
    event.preventDefault();
    error.hidden = true;
    var encoder = new TextEncoder();
    window.crypto.subtle
      .importKey('raw', encoder.encode(input.value), 'PBKDF2', false, ['deriveKey'])
      .then(function (material) {
        return window.crypto.subtle.deriveKey(
          { name: 'PBKDF2', salt: bytes(params.salt), iterations: params.iterations, hash: 'SHA-256' },
          material,
          { name: 'AES-GCM', length: 256 },
          false,
          ['decrypt']
        );
      })
      .then(function (key) {
        return window.crypto.subtle.decrypt(
          { name: 'AES-GCM', iv: bytes(params.iv) },
          key,
          bytes(cipherB64)
        );
      })
      .then(function (plain) {
        var holder = document.createElement('div');
        holder.innerHTML = new TextDecoder().decode(plain);
        var parent = lock.parentNode;
        while (holder.firstChild) parent.insertBefore(holder.firstChild, lock);
        parent.removeChild(lock);
      })
      .catch(function () {
        error.hidden = false;
        error.textContent = 'Wrong password.';
        input.select();
      });
  });
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use typost_core::{MetadataEntry, Plugin};

    /// Build a `FrontMatter` dict from string pairs.
    fn fm(pairs: &[(&str, &str)]) -> FrontMatter {
        FrontMatter::Dict(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), FrontMatter::Str((*v).to_owned())))
                .collect(),
        )
    }

    #[test]
    fn round_trips() {
        let locked = encrypt_bytes("hunter2", b"top secret", 1000).unwrap();
        assert_eq!(decrypt_bytes("hunter2", &locked).unwrap(), b"top secret");
        assert!(decrypt_bytes("wrong", &locked).is_err());
    }

    #[test]
    fn encrypts_inline_regions_in_order() {
        let html = "<main id=\"typost-content\">\
                    <p>public</p>\
                    <div class=\"typost-encrypted\"><p>secret one <b>bold</b></p></div>\
                    <div class=\"typost-encrypted\"><p>secret two</p></div>\
                    </main>";
        let mut out = RenderOutput::new();
        out.insert("page.html", html.as_bytes().to_vec());

        let manifest = SiteManifest {
            title: None,
            pages: Vec::new(),
            entries: vec![
                MetadataEntry {
                    kind: "encrypted".into(),
                    route: Some("page.html".into()),
                    data: fm(&[("password", "pw1"), ("hint", "Type <the> word")]),
                },
                MetadataEntry {
                    kind: "encrypted".into(),
                    route: Some("page.html".into()),
                    data: fm(&[("password", "pw2")]),
                },
            ],
        };

        Encrypt::with_iterations(1000)
            .post(&mut out, &manifest)
            .unwrap();

        let result = String::from_utf8(out.get("page.html").unwrap().to_vec()).unwrap();
        assert!(!result.contains("secret one"));
        assert!(!result.contains("secret two"));
        assert!(result.contains("public"));
        assert_eq!(result.matches("typost-lock\"").count(), 2);
        assert_eq!(result.matches("typost-encrypted").count(), 2);
        // A custom hint is rendered and HTML-escaped.
        assert!(result.contains("Type &lt;the&gt; word"));
        // Regions without a hint get the default prompt.
        assert!(result.contains("This part is encrypted"));
    }

    #[test]
    fn no_entries_is_a_noop() {
        let html = "<main id=\"typost-content\"><p>public</p></main>";
        let mut out = RenderOutput::new();
        out.insert("page.html", html.as_bytes().to_vec());
        let manifest = SiteManifest {
            title: None,
            pages: Vec::new(),
            entries: Vec::new(),
        };

        Encrypt::new().post(&mut out, &manifest).unwrap();
        assert_eq!(out.get("page.html").unwrap(), html.as_bytes());
    }

    #[test]
    fn contributes_the_typst_helper() {
        let mut overlay = TypstOverlay::new();
        Encrypt::new().typst(&mut overlay).unwrap();
        let source = overlay
            .files
            .iter()
            .find(|(path, _)| path.get_without_slash() == "lib/typost/encrypted.typ")
            .map(|(_, bytes)| bytes)
            .expect("helper contributed");
        assert!(String::from_utf8_lossy(source).contains("#let encrypted"));
    }

    #[test]
    fn missing_password_fails_the_build() {
        let dir = std::env::temp_dir().join(format!("typost-encrypt-nopw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("home.typ"),
            "#import \"lib/typost/encrypted.typ\": encrypted\n\
             #encrypted(hint: \"x\")[secret]\n",
        )
        .unwrap();

        let options = typost_core::BuildOptions {
            root: dir.clone(),
            entry: "home.typ".to_owned(),
            out: dir.join("dist"),
        };
        let plugins: Vec<Box<dyn Plugin>> = vec![Box::new(Encrypt::new())];
        let error =
            typost_core::build(&options, &plugins).expect_err("missing password should fail");
        assert!(
            format!("{error:?}").contains("password"),
            "unexpected error: {error:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

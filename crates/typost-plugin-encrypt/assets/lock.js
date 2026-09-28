
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

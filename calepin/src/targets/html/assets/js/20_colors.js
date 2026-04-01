// --- Color scheme picker ---

(function() {
  var picker = document.getElementById('colors-picker');
  var menu = document.getElementById('colors-menu');
  if (!picker || !menu) return;

  var cache = {};
  var hlCache = {};
  var defaultHighlightCSS = null;

  function applyCSS(css) {
    var s = document.getElementById('calepin-color-scheme-style');
    if (!s) { s = document.createElement('style'); s.id = 'calepin-color-scheme-style'; document.head.appendChild(s); }
    s.textContent = css;
  }

  function applyHighlightCSS(css) {
    var s = document.getElementById('calepin-highlight-style');
    if (s) {
      if (defaultHighlightCSS === null) defaultHighlightCSS = s.textContent;
      s.textContent = css;
    }
  }

  function clearColors() {
    var s = document.getElementById('calepin-color-scheme-style');
    if (s) s.remove();
    var h = document.getElementById('calepin-highlight-style');
    if (h && defaultHighlightCSS !== null) h.textContent = defaultHighlightCSS;
  }

  function applyColors(name, url, hlUrl) {
    if (!name || !url) { clearColors(); return; }
    if (cache[name]) { applyCSS(cache[name]); }
    else {
      fetch(url).then(function(r) { return r.text(); }).then(function(css) {
        cache[name] = css;
        applyCSS(css);
      });
    }
    if (hlUrl) {
      if (hlCache[name]) { applyHighlightCSS(hlCache[name]); }
      else {
        fetch(hlUrl).then(function(r) { return r.text(); }).then(function(css) {
          hlCache[name] = css;
          applyHighlightCSS(css);
        });
      }
    }
  }

  // Restore saved color scheme
  var saved = localStorage.getItem('calepin-colors');
  if (saved) {
    var btn = menu.querySelector('[data-colors="' + saved + '"]');
    if (btn) applyColors(saved, btn.getAttribute('data-colors-file'), btn.getAttribute('data-highlight-file'));
  }

  picker.addEventListener('click', function(e) {
    e.stopPropagation();
    menu.classList.toggle('hidden');
  });

  menu.querySelectorAll('.colors-option').forEach(function(btn) {
    btn.addEventListener('click', function(e) {
      e.stopPropagation();
      var name = btn.getAttribute('data-colors');
      var url = btn.getAttribute('data-colors-file');
      var hlUrl = btn.getAttribute('data-highlight-file');
      applyColors(name, url, hlUrl);
      if (name) {
        localStorage.setItem('calepin-colors', name);
      } else {
        localStorage.removeItem('calepin-colors');
      }
      menu.classList.add('hidden');
    });
  });

  document.addEventListener('click', function() {
    menu.classList.add('hidden');
  });
})();

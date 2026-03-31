// Calepin core runtime
// Shared by standalone documents (inlined) and websites (concatenated into calepin.js).
// Must be a plain script (no import/export) so it can be inlined in <script> tags.

// --- Theme toggle ---

(function() {
  var btn = document.getElementById('theme-toggle');
  if (!btn) return;

  function current() {
    return document.documentElement.getAttribute('data-theme') ||
      (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
  }

  function update(theme) {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('calepin-theme', theme);
    btn.setAttribute('aria-pressed', theme === 'dark' ? 'true' : 'false');
    // SVG icon toggle (website widget_dark.html)
    var sun = btn.querySelector('.icon-sun');
    var moon = btn.querySelector('.icon-moon');
    if (sun && moon) {
      sun.style.display = theme === 'dark' ? 'block' : 'none';
      moon.style.display = theme === 'dark' ? 'none' : 'block';
    }
    // Unicode text toggle (standalone document)
    if (!sun && !moon) {
      btn.textContent = theme === 'dark' ? '\u2600' : '\u263e';
    }
  }

  update(current());
  btn.addEventListener('click', function() {
    update(current() === 'dark' ? 'light' : 'dark');
  });
})();

// --- Tabset switching ---

document.querySelectorAll('.panel-tabset .nav-link').forEach(function(btn) {
  btn.addEventListener('click', function() {
    var tabset = btn.closest('.panel-tabset');
    var tab = btn.getAttribute('data-tab');
    var group = tabset.getAttribute('data-group');
    var targets = group
      ? document.querySelectorAll('.panel-tabset[data-group="' + group + '"]')
      : [tabset];
    targets.forEach(function(ts) {
      ts.querySelectorAll('.nav-link').forEach(function(b) {
        var isActive = b.getAttribute('data-tab') === tab;
        b.classList.toggle('active', isActive);
        b.setAttribute('aria-selected', isActive ? 'true' : 'false');
      });
      ts.querySelectorAll('.tab-pane').forEach(function(p) {
        var isActive = p.getAttribute('data-tab') === tab;
        p.classList.toggle('active', isActive);
        p.setAttribute('aria-hidden', isActive ? 'false' : 'true');
      });
    });
  });
});

// --- TOC active section tracking ---

(function() {
  var links = document.querySelectorAll('.toc a');
  if (!links.length) return;
  var headings = [];
  links.forEach(function(a) {
    var id = a.getAttribute('href');
    if (id) { var el = document.querySelector(id); if (el) headings.push({ el: el, link: a }); }
  });
  if (!headings.length) return;
  var observer = new IntersectionObserver(function(entries) {
    entries.forEach(function(entry) {
      if (entry.isIntersecting) {
        links.forEach(function(a) { a.classList.remove('active'); });
        var match = headings.find(function(h) { return h.el === entry.target; });
        if (match) {
          match.link.classList.add('active');
          match.link.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
        }
      }
    });
  }, { rootMargin: '0px 0px -80% 0px' });
  headings.forEach(function(h) { observer.observe(h.el); });
})();

// --- Code copy buttons ---

document.querySelectorAll('.code-copy-btn').forEach(function(btn) {
  btn.addEventListener('click', function() {
    var code = btn.parentElement.querySelector('code');
    if (!code) return;
    navigator.clipboard.writeText(code.textContent).then(function() {
      var icon = btn.querySelector('.code-copy-icon');
      btn.classList.add('copied');
      icon.classList.add('checked');
      setTimeout(function() {
        btn.classList.remove('copied');
        icon.classList.remove('checked');
      }, 1500);
    });
  });
});

// --- Footnote hover previews ---

document.querySelectorAll('.footnote-ref a').forEach(function(a) {
  var id = a.getAttribute('href');
  if (!id) return;
  var fn = document.querySelector(id);
  if (!fn) return;
  var text = fn.textContent.replace(/\s*\u21a9\s*$/, '').trim();
  if (!text) return;
  var tip = document.createElement('span');
  tip.className = 'fn-preview';
  tip.textContent = text;
  a.parentElement.appendChild(tip);
});
// Calepin site runtime
// Website-only features. Concatenated after core.js into calepin.js for websites.
// Must be a plain script (no import/export).

// --- Mobile sidebar toggle ---

(function() {
  var menuBtn = document.getElementById('sidebar-toggle');
  var sidebar = document.querySelector('.sidebar-left');
  var navMenu = document.getElementById('navbar-menu');
  if (!menuBtn) return;
  menuBtn.addEventListener('click', function() {
    if (sidebar) {
      sidebar.classList.toggle('open');
      menuBtn.setAttribute('aria-expanded', sidebar.classList.contains('open') ? 'true' : 'false');
    } else if (navMenu) {
      navMenu.classList.toggle('open');
      menuBtn.setAttribute('aria-expanded', navMenu.classList.contains('open') ? 'true' : 'false');
    }
  });
})();

// --- Sidebar section persistence ---

(function() {
  var KEY = 'calepin-sidebar-section';
  var sections = document.querySelectorAll('details.sidebar-section[name="sidebar-nav"]');
  if (!sections.length) return;

  // Restore saved section (override the server-rendered open state)
  var saved = localStorage.getItem(KEY);
  if (saved !== null) {
    sections.forEach(function(d) {
      var title = d.querySelector('.sidebar-section-title');
      var name = title ? title.textContent.trim() : '';
      d.open = (name === saved);
    });
  }

  // Save on toggle
  sections.forEach(function(d) {
    d.addEventListener('toggle', function() {
      if (d.open) {
        var title = d.querySelector('.sidebar-section-title');
        localStorage.setItem(KEY, title ? title.textContent.trim() : '');
      }
    });
  });
})();

// --- Navbar dropdown toggle ---

document.querySelectorAll('.navbar-dropdown-toggle').forEach(function(btn) {
  btn.addEventListener('click', function(e) {
    e.stopPropagation();
    var dropdown = btn.closest('.navbar-dropdown');
    var wasOpen = dropdown.classList.contains('open');
    document.querySelectorAll('.navbar-dropdown.open').forEach(function(d) {
      d.classList.remove('open');
      d.querySelector('.navbar-dropdown-toggle').setAttribute('aria-expanded', 'false');
    });
    if (!wasOpen) {
      dropdown.classList.add('open');
      btn.setAttribute('aria-expanded', 'true');
    }
  });
});
document.addEventListener('click', function() {
  document.querySelectorAll('.navbar-dropdown.open').forEach(function(d) {
    d.classList.remove('open');
    d.querySelector('.navbar-dropdown-toggle').setAttribute('aria-expanded', 'false');
  });
});

// --- Header auto-hide on scroll ---

(function() {
  var header = document.querySelector('.site-header');
  if (!header) return;
  var lastScroll = 0;
  var hideTimer = null;
  var headerHeight = header.offsetHeight;

  window.addEventListener('scroll', function() {
    var current = window.scrollY;
    if (current > headerHeight && current > lastScroll) {
      header.classList.add('hidden');
    }
    lastScroll = current;
    if (current <= headerHeight) {
      header.classList.remove('hidden');
    }
  }, { passive: true });

  document.addEventListener('mousemove', function(e) {
    if (e.clientY < headerHeight * 2) {
      header.classList.remove('hidden');
      clearTimeout(hideTimer);
      hideTimer = setTimeout(function() {
        if (window.scrollY > headerHeight) {
          header.classList.add('hidden');
        }
      }, 1500);
    }
  });
})();

// --- Source toggle (split view) ---

(function() {
  var sourceBtn = document.getElementById('source-toggle');
  if (!sourceBtn) return;
  sourceBtn.addEventListener('click', function() {
    document.body.classList.toggle('split-active');
    sourceBtn.classList.toggle('active');
    var isActive = sourceBtn.classList.contains('active');
    sourceBtn.setAttribute('aria-expanded', isActive ? 'true' : 'false');
    var code = document.getElementById('source-code');
    if (code && code.textContent === 'Loading source...') {
      var url = sourceBtn.getAttribute('data-source');
      if (url) {
        fetch(url)
          .then(function(r) { return r.text(); })
          .then(function(t) { code.textContent = t; })
          .catch(function() { code.textContent = 'Failed to load source.'; });
      }
    }
  });
})();

// --- Search (pagefind) ---

(function() {
  var searchOverlay = document.querySelector('.search-overlay');
  var searchToggle = document.querySelector('[data-search-toggle]');
  if (!searchOverlay || !searchToggle) return;

  var searchInput = searchOverlay.querySelector('.search-input');
  var resultsEl = searchOverlay.querySelector('.search-results');
  var statusEl = document.getElementById('search-status');
  var selectedIdx = -1;
  var pagefind = null;

  function loadPagefind() {
    if (pagefind) return Promise.resolve(pagefind);
    return import('../pagefind/pagefind.js').then(function(pf) {
      pagefind = pf;
      return pf;
    }).catch(function() {
      resultsEl.innerHTML = '<div class="search-hint">Search index not available.</div>';
      return null;
    });
  }

  function escHtml(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  function doSearch(query) {
    if (!query.trim()) {
      resultsEl.innerHTML = '<div class="search-hint">Start typing to search...</div>';
      searchInput.setAttribute('aria-expanded', 'false');
      searchInput.removeAttribute('aria-activedescendant');
      if (statusEl) statusEl.textContent = '';
      selectedIdx = -1;
      return;
    }

    loadPagefind().then(function(pf) {
      if (!pf) return;
      return pf.search(query).then(function(search) {
        return Promise.all(search.results.slice(0, 15).map(function(r) { return r.data(); }));
      }).then(function(results) {
        selectedIdx = -1;
        searchInput.removeAttribute('aria-activedescendant');

        if (!results || results.length === 0) {
          resultsEl.innerHTML = '<div class="search-hint">No results found.</div>';
          searchInput.setAttribute('aria-expanded', 'false');
          if (statusEl) statusEl.textContent = 'No results found.';
          return;
        }

        searchInput.setAttribute('aria-expanded', 'true');
        if (statusEl) statusEl.textContent = results.length + ' result' + (results.length === 1 ? '' : 's') + ' found.';
        resultsEl.innerHTML = results.map(function(r, i) {
          return '<a class="search-result" id="search-result-' + i + '" role="option" href="' + escHtml(r.url) + '">' +
            '<div class="search-result-title">' + escHtml(r.meta.title || r.url) + '</div>' +
            '<div class="search-result-text">' + (r.excerpt || '') + '</div>' +
          '</a>';
        }).join('');
      });
    }).catch(function() {
      resultsEl.innerHTML = '<div class="search-hint">Search error.</div>';
    });
  }

  function updateSelected() {
    var items = resultsEl.querySelectorAll('.search-result');
    items.forEach(function(el, i) {
      el.classList.toggle('selected', i === selectedIdx);
      el.setAttribute('aria-selected', i === selectedIdx ? 'true' : 'false');
    });
    if (items[selectedIdx]) {
      items[selectedIdx].scrollIntoView({ block: 'nearest' });
      searchInput.setAttribute('aria-activedescendant', 'search-result-' + selectedIdx);
    }
  }

  function openSearch() {
    searchOverlay.classList.add('active');
    searchInput.value = '';
    searchInput.setAttribute('aria-expanded', 'false');
    searchInput.removeAttribute('aria-activedescendant');
    resultsEl.innerHTML = '<div class="search-hint">Start typing to search...</div>';
    if (statusEl) statusEl.textContent = '';
    selectedIdx = -1;
    setTimeout(function() { searchInput.focus(); }, 50);
  }

  function closeSearch() {
    searchOverlay.classList.remove('active');
    searchToggle.focus();
  }

  var debounce = null;
  searchInput.addEventListener('input', function() {
    clearTimeout(debounce);
    debounce = setTimeout(function() { doSearch(searchInput.value); }, 100);
  });

  searchInput.addEventListener('keydown', function(e) {
    var items = resultsEl.querySelectorAll('.search-result');
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIdx = Math.min(selectedIdx + 1, items.length - 1);
      updateSelected();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIdx = Math.max(selectedIdx - 1, 0);
      updateSelected();
    } else if (e.key === 'Enter' && items[selectedIdx]) {
      e.preventDefault();
      items[selectedIdx].click();
    }
  });

  searchToggle.addEventListener('click', openSearch);
  searchOverlay.addEventListener('click', function(e) {
    if (e.target === searchOverlay) closeSearch();
  });
  document.addEventListener('keydown', function(e) {
    if ((e.ctrlKey || e.metaKey) && e.key === 'k') { e.preventDefault(); openSearch(); }
    if (e.key === 'Escape') closeSearch();
  });
})();

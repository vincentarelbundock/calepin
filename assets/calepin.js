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
    var sun = btn.querySelector('.icon-sun');
    var moon = btn.querySelector('.icon-moon');
    if (sun && moon) {
      sun.classList.toggle('hidden', theme !== 'dark');
      moon.classList.toggle('hidden', theme === 'dark');
    }
  }

  update(current());
  btn.addEventListener('click', function() {
    update(current() === 'dark' ? 'light' : 'dark');
  });
})();

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

// --- Mobile sidebar toggle ---

(function() {
  var menuBtn = document.getElementById('sidebar-toggle');
  var sidebar = document.getElementById('sidebar-nav');
  var navMenu = document.getElementById('navbar-menu');
  if (!menuBtn) return;
  menuBtn.addEventListener('click', function() {
    if (sidebar) {
      sidebar.classList.toggle('max-md:-translate-x-full');
      var open = !sidebar.classList.contains('max-md:-translate-x-full');
      menuBtn.setAttribute('aria-expanded', open ? 'true' : 'false');
    } else if (navMenu) {
      navMenu.classList.toggle('hidden');
      menuBtn.setAttribute('aria-expanded', navMenu.classList.contains('hidden') ? 'false' : 'true');
    }
  });
})();

// --- Sidebar section persistence ---

(function() {
  var KEY = 'calepin-sidebar-section';
  var sections = document.querySelectorAll('details.sidebar-section[name="sidebar-nav"]');
  if (!sections.length) return;

  var saved = localStorage.getItem(KEY);
  if (saved !== null) {
    sections.forEach(function(d) {
      var title = d.querySelector('.sidebar-section-title');
      var name = title ? title.textContent.trim() : '';
      d.open = (name === saved);
    });
  }

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

// --- Source toggle (split view) -- bound once, reads data-source fresh each time ---

var _clpSourceBtn = document.getElementById('source-toggle');
if (_clpSourceBtn) {
  var splitOverlay = document.getElementById('split-overlay');
  _clpSourceBtn.addEventListener('click', function() {
    var isActive = splitOverlay && splitOverlay.classList.contains('hidden');
    if (splitOverlay) splitOverlay.classList.toggle('hidden');
    _clpSourceBtn.setAttribute('aria-expanded', isActive ? 'true' : 'false');

    // Populate left panel with rendered content
    var splitLeft = document.getElementById('split-left');
    var mainContent = document.getElementById('main-content');
    if (splitLeft && mainContent && !splitLeft.innerHTML.trim()) {
      splitLeft.innerHTML = mainContent.innerHTML;
    }

    var code = document.getElementById('source-code');
    if (code && code.textContent === 'Loading source...') {
      var url = _clpSourceBtn.getAttribute('data-source');
      if (url) {
        fetch(url)
          .then(function(r) { return r.text(); })
          .then(function(t) { code.textContent = t; })
          .catch(function() { code.textContent = 'Failed to load source.'; });
      }
    }
  });
}

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
          return '<a class="search-result block p-3 rounded-lg cursor-pointer no-underline text-[color:inherit] hover:bg-hover" id="search-result-' + i + '" role="option" href="' + escHtml(r.url) + '">' +
            '<div class="font-semibold text-[0.95rem]">' + escHtml(r.meta.title || r.url) + '</div>' +
            '<div class="text-sm text-muted mt-0.5">' + (r.excerpt || '') + '</div>' +
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
      el.classList.toggle('bg-hover', i === selectedIdx);
      el.setAttribute('aria-selected', i === selectedIdx ? 'true' : 'false');
    });
    if (items[selectedIdx]) {
      items[selectedIdx].scrollIntoView({ block: 'nearest' });
      searchInput.setAttribute('aria-activedescendant', 'search-result-' + selectedIdx);
    }
  }

  function openSearch() {
    searchOverlay.classList.remove('hidden');
    searchOverlay.classList.add('flex');
    searchInput.value = '';
    searchInput.setAttribute('aria-expanded', 'false');
    searchInput.removeAttribute('aria-activedescendant');
    resultsEl.innerHTML = '<div class="search-hint">Start typing to search...</div>';
    if (statusEl) statusEl.textContent = '';
    selectedIdx = -1;
    setTimeout(function() { searchInput.focus(); }, 50);
  }

  function closeSearch() {
    searchOverlay.classList.add('hidden');
    searchOverlay.classList.remove('flex');
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

// --- Per-page initialization (re-run after client-side navigation) ---

var _clpTocObserver = null;

function _clpInitPage() {
  // Tabset switching
  document.querySelectorAll('.panel-tabset .nav-link').forEach(function(btn) {
    if (btn._clpBound) return;
    btn._clpBound = true;
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
          p.classList.toggle('hidden', !isActive);
          p.setAttribute('aria-hidden', isActive ? 'false' : 'true');
        });
      });
    });
  });

  // Inject copy buttons into bare <pre> blocks (non-executable code)
  document.querySelectorAll('pre').forEach(function(pre) {
    if (pre.querySelector('.code-copy-btn')) return;
    pre.classList.add('relative');
    var btn = document.createElement('button');
    btn.className = 'code-copy-btn absolute top-2 right-2 text-faint hover:text-muted opacity-30 hover:opacity-100 transition-opacity';
    btn.setAttribute('aria-label', 'Copy');
    btn.innerHTML = '<svg class="code-copy-icon w-4 h-4" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 448 512"><path fill="currentColor" d="M384 336H192c-8.8 0-16-7.2-16-16V64c0-8.8 7.2-16 16-16h133.5c4.2 0 8.3 1.7 11.3 4.7l58.5 58.5c3 3 4.7 7.1 4.7 11.3V320c0 8.8-7.2 16-16 16m-192 48h192c35.3 0 64-28.7 64-64V122.5c0-17-6.7-33.3-18.7-45.3l-58.6-58.5C358.7 6.7 342.5 0 325.5 0H192c-35.3 0-64 28.7-64 64v256c0 35.3 28.7 64 64 64M64 128c-35.3 0-64 28.7-64 64v256c0 35.3 28.7 64 64 64h192c35.3 0 64-28.7 64-64v-16h-48v16c0 8.8-7.2 16-16 16H64c-8.8 0-16-7.2-16-16V192c0-8.8 7.2-16 16-16h16v-48z"/></svg><svg class="code-check-icon w-4 h-4 hidden" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"><path fill="currentColor" d="M256 512a256 256 0 1 1 0-512 256 256 0 1 1 0 512m0-464a208 208 0 1 0 0 416 208 208 0 1 0 0-416m70.7 121.9c7.8-10.7 22.8-13.1 33.5-5.3s13.1 22.8 5.3 33.5l-122.1 168c-4.1 5.7-10.5 9.3-17.5 9.8s-13.9-2-18.8-6.9l-55.9-55.9c-9.4-9.4-9.4-24.6 0-33.9s24.6-9.4 33.9 0l36 36L326.7 170z"/></svg>';
    pre.appendChild(btn);
  });

  // Code copy buttons
  document.querySelectorAll('.code-copy-btn').forEach(function(btn) {
    if (btn._clpBound) return;
    btn._clpBound = true;
    btn.addEventListener('click', function() {
      var code = btn.parentElement.querySelector('code');
      if (!code) return;
      navigator.clipboard.writeText(code.textContent).then(function() {
        var copyIcon = btn.querySelector('.code-copy-icon');
        var checkIcon = btn.querySelector('.code-check-icon');
        copyIcon.classList.add('hidden');
        checkIcon.classList.remove('hidden');
        setTimeout(function() {
          checkIcon.classList.add('hidden');
          copyIcon.classList.remove('hidden');
        }, 1500);
      });
    });
  });

  // TOC active section tracking
  if (_clpTocObserver) { _clpTocObserver.disconnect(); _clpTocObserver = null; }
  var tocLinks = document.querySelectorAll('.toc a');
  if (tocLinks.length) {
    var headings = [];
    tocLinks.forEach(function(a) {
      var id = a.getAttribute('href');
      if (id) {
        var el = document.querySelector(id);
        if (el) headings.push({ el: el, link: a });
      }
    });
    if (headings.length) {
      _clpTocObserver = new IntersectionObserver(function(entries) {
        entries.forEach(function(entry) {
          if (entry.isIntersecting) {
            tocLinks.forEach(function(a) { a.classList.remove('active'); });
            var match = headings.find(function(h) { return h.el === entry.target; });
            if (match) {
              match.link.classList.add('active');
              match.link.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
            }
          }
        });
      }, { rootMargin: '0px 0px -80% 0px' });
      headings.forEach(function(h) { _clpTocObserver.observe(h.el); });
    }
  }

  // Footnote hover previews
  document.querySelectorAll('.footnote-ref a').forEach(function(a) {
    if (a._clpBound) return;
    a._clpBound = true;
    var id = a.getAttribute('href');
    if (!id) return;
    var fn = document.querySelector(id);
    if (!fn) return;
    var text = fn.textContent.replace(/\s*\u21a9\s*$/, '').trim();
    if (!text) return;
    a.parentElement.classList.add('relative', 'group');
    var tip = document.createElement('span');
    tip.className = 'hidden group-hover:block absolute bottom-full left-1/2 -translate-x-1/2 px-2.5 py-1.5 rounded-md text-sm leading-snug w-max max-w-xs z-50 bg-code-bg border border-brd text-[color:var(--c-text)] shadow-lg';
    tip.textContent = text;
    a.parentElement.appendChild(tip);
  });

  // Re-render KaTeX if present
  if (window.renderMathInElement) {
    var main = document.getElementById('main-content');
    if (main) window.renderMathInElement(main);
  }
}

_clpInitPage();

// --- Client-side navigation (websites only, no-op for standalone) ---

(function() {
  // Only activate if there is a sidebar (i.e., this is a multi-page site)
  if (!document.getElementById('sidebar-nav')) return;

  // Disable browser's automatic scroll restoration for SPA navigation
  if ('scrollRestoration' in history) history.scrollRestoration = 'manual';

  function isInternalLink(a) {
    if (a.hostname !== location.hostname) return false;
    if (a.getAttribute('target') === '_blank') return false;
    if (a.getAttribute('download') != null) return false;
    var href = a.getAttribute('href');
    if (!href || href.startsWith('#') || href.startsWith('mailto:') || href.startsWith('javascript:')) return false;
    // Only SPA-navigate .html pages (not assets, not bare paths like /)
    var path = a.pathname || '';
    if (!/\.html$/.test(path)) return false;
    if (/\/assets\//.test(path)) return false;
    return true;
  }

  function updateSidebarActive(url) {
    var path = new URL(url, location.href).pathname;
    var activeSection = null;
    document.querySelectorAll('#sidebar-nav a').forEach(function(a) {
      var aPath = new URL(a.href).pathname;
      var isActive = aPath === path;
      a.classList.toggle('text-link', isActive);
      a.classList.toggle('font-semibold', isActive);
      if (isActive) {
        a.setAttribute('aria-current', 'page');
        activeSection = a.closest('details.sidebar-section');
      } else {
        a.removeAttribute('aria-current');
      }
    });
    // Close all sections, then open the one containing the active link
    document.querySelectorAll('#sidebar-nav details.sidebar-section').forEach(function(d) {
      d.open = (d === activeSection);
    });
  }

  function navigateTo(url) {
    return fetch(url).then(function(resp) {
      if (!resp.ok) { location.href = url; return; }
      return resp.text().then(function(html) {
        // Resolve relative URLs in inline styles against the target URL
        var baseUrl = new URL(url, location.href);
        var baseDir = baseUrl.href.replace(/[^/]*$/, '');
        function resolveHtml(raw) {
          return raw.replace(/url\(([^)]+)\)/g, function(m, p) {
            p = p.trim().replace(/^['"]|['"]$/g, '');
            if (/^(https?:|data:|\/)/i.test(p)) return m;
            return 'url(' + new URL(p, baseDir).href + ')';
          });
        }

        var doc = new DOMParser().parseFromString(html, 'text/html');

        // Swap main content
        var newMain = doc.getElementById('main-content');
        var oldMain = document.getElementById('main-content');
        if (!newMain || !oldMain) { location.href = url; return; }
        oldMain.innerHTML = resolveHtml(newMain.innerHTML);

        // Browsers ignore <style>, <script>, and <link> injected via innerHTML.
        // Clone each into a fresh DOM element so CSS/JS from packages like
        // tinytable and gt takes effect after client-side navigation.
        // Scripts must be chained: external scripts (src=) load async, so
        // inline scripts that depend on them must wait via onload.
        (function reactivate() {
          var els = oldMain.querySelectorAll('style, script, link[rel="stylesheet"]');
          var scripts = [];
          els.forEach(function(el) {
            if (el.tagName === 'SCRIPT') {
              scripts.push(el);
            } else {
              var active = document.createElement(el.tagName.toLowerCase());
              Array.from(el.attributes).forEach(function(a) { active.setAttribute(a.name, a.value); });
              if (el.textContent) active.textContent = el.textContent;
              el.replaceWith(active);
            }
          });
          // Chain scripts so external src= loads before inline scripts run.
          // The document is already loaded during SPA nav, so temporarily patch
          // window.addEventListener to run 'load' callbacks immediately. Packages
          // like tinytable wrap their init in window.addEventListener('load', fn).
          var origAddEvent = window.addEventListener;
          window.addEventListener = function(type, fn) {
            if (type === 'load') { fn(); }
            else { origAddEvent.apply(window, arguments); }
          };
          function runNext(i) {
            if (i >= scripts.length) {
              window.addEventListener = origAddEvent;
              return;
            }
            var el = scripts[i];
            var active = document.createElement('script');
            Array.from(el.attributes).forEach(function(a) { active.setAttribute(a.name, a.value); });
            if (!el.src) active.textContent = el.textContent;
            active.onload = active.onerror = function() { runNext(i + 1); };
            el.replaceWith(active);
            if (!el.src) runNext(i + 1);
          }
          runNext(0);
        })();

        // Swap TOC
        var oldToc = document.getElementById('sidebar-toc');
        var newToc = doc.getElementById('sidebar-toc');
        if (oldToc && newToc) {
          oldToc.innerHTML = newToc.innerHTML;
        } else if (oldToc && !newToc) {
          oldToc.innerHTML = '';
        }

        // Sync sidebar/toc visibility from new page
        var oldBody = document.querySelector('.site-body');
        var newBody = doc.querySelector('.site-body');
        if (oldBody && newBody) {
          oldBody.classList.toggle('no-sidebar', newBody.classList.contains('no-sidebar'));
          oldBody.classList.toggle('no-toc', newBody.classList.contains('no-toc'));
        }

        // Update source toggle URL for split view
        var newSourceBtn = doc.getElementById('source-toggle');
        if (_clpSourceBtn && newSourceBtn) {
          _clpSourceBtn.setAttribute('data-source', newSourceBtn.getAttribute('data-source') || '');
        }

        // Update title
        var newTitle = doc.querySelector('title');
        if (newTitle) document.title = newTitle.textContent;

        // Update URL and scroll
        history.pushState(null, '', url);
        _lastPath = new URL(url, location.href).pathname;
        var hash = new URL(url, location.href).hash;
        window.scrollTo(0, 0);
        if (hash) {
          var target = document.getElementById(hash.slice(1));
          if (target) target.scrollIntoView();
        }
        // Ensure scroll sticks after layout recalculation (e.g., Tailwind CDN)
        setTimeout(function() {
          if (hash) {
            var target = document.getElementById(hash.slice(1));
            if (target) target.scrollIntoView();
          } else {
            window.scrollTo(0, 0);
          }
        }, 50);

        // Update sidebar active state
        updateSidebarActive(url);

        // Reset split view
        var splitOverlay = document.getElementById('split-overlay');
        if (splitOverlay) splitOverlay.classList.add('hidden');
        var splitLeft = document.getElementById('split-left');
        if (splitLeft) splitLeft.innerHTML = '';
        var sourceCode = document.getElementById('source-code');
        if (sourceCode) sourceCode.textContent = 'Loading source...';
        if (_clpSourceBtn) {
          _clpSourceBtn.setAttribute('aria-expanded', 'false');
        }

        // Re-initialize page-level features
        _clpInitPage();
      });
    }).catch(function() {
      location.href = url;
    });
  }

  // Intercept clicks on internal links
  document.addEventListener('click', function(e) {
    var a = e.target.closest('a');
    if (!a || !isInternalLink(a)) return;
    if (e.ctrlKey || e.metaKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    if (a.href === location.href) return;
    navigateTo(a.href);
  });

  // Handle back/forward
  var _lastPath = location.pathname;
  window.addEventListener('popstate', function() {
    if (location.pathname === _lastPath) {
      // Same page, just a hash change -- let the browser handle it
      return;
    }
    _lastPath = location.pathname;
    navigateTo(location.href);
  });
})();

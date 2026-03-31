// Calepin website runtime
// Loaded as <script type="module"> so import() is available.

// --- Theme toggle, sidebar, dropdowns, tabsets, header auto-hide ---

const STORAGE_KEY = 'calepin-theme';

function getPreferred() {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored) return stored;
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function applyTheme(theme) {
  document.documentElement.setAttribute('data-theme', theme);
  localStorage.setItem(STORAGE_KEY, theme);
  const btn = document.getElementById('theme-toggle');
  if (btn) {
    const sunIcon = btn.querySelector('.icon-sun');
    const moonIcon = btn.querySelector('.icon-moon');
    if (sunIcon && moonIcon) {
      sunIcon.style.display = theme === 'dark' ? 'block' : 'none';
      moonIcon.style.display = theme === 'dark' ? 'none' : 'block';
    }
    btn.setAttribute('aria-pressed', theme === 'dark' ? 'true' : 'false');
  }
}

// Apply immediately to prevent flash
applyTheme(getPreferred());

// Theme toggle
const themeBtn = document.getElementById('theme-toggle');
if (themeBtn) {
  applyTheme(getPreferred());
  themeBtn.addEventListener('click', function() {
    const current = document.documentElement.getAttribute('data-theme');
    applyTheme(current === 'dark' ? 'light' : 'dark');
  });
}

// Mobile sidebar toggle
const menuBtn = document.getElementById('sidebar-toggle');
const sidebar = document.querySelector('.sidebar-left');
const navMenu = document.getElementById('navbar-menu');
if (menuBtn) {
  menuBtn.addEventListener('click', function() {
    if (sidebar) {
      sidebar.classList.toggle('open');
      menuBtn.setAttribute('aria-expanded', sidebar.classList.contains('open') ? 'true' : 'false');
    } else if (navMenu) {
      navMenu.classList.toggle('open');
      menuBtn.setAttribute('aria-expanded', navMenu.classList.contains('open') ? 'true' : 'false');
    }
  });
}

// Navbar dropdown toggle
document.querySelectorAll('.navbar-dropdown-toggle').forEach(function(btn) {
  btn.addEventListener('click', function(e) {
    e.stopPropagation();
    const dropdown = btn.closest('.navbar-dropdown');
    const wasOpen = dropdown.classList.contains('open');
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

// Tabset switching
document.querySelectorAll('.panel-tabset .nav-link').forEach(function(btn) {
  btn.addEventListener('click', function() {
    const tabset = btn.closest('.panel-tabset');
    const tab = btn.getAttribute('data-tab');
    const group = tabset.getAttribute('data-group');
    const targets = group
      ? document.querySelectorAll('.panel-tabset[data-group="' + group + '"]')
      : [tabset];
    targets.forEach(function(ts) {
      ts.querySelectorAll('.nav-link').forEach(function(b) {
        const isActive = b.getAttribute('data-tab') === tab;
        b.classList.toggle('active', isActive);
        b.setAttribute('aria-selected', isActive ? 'true' : 'false');
      });
      ts.querySelectorAll('.tab-pane').forEach(function(p) {
        const isActive = p.getAttribute('data-tab') === tab;
        p.classList.toggle('active', isActive);
        p.setAttribute('aria-hidden', isActive ? 'false' : 'true');
      });
    });
  });
});

// Header auto-hide on scroll, reappear on mouse near top
const header = document.querySelector('.site-header');
if (header) {
  let lastScroll = 0;
  let hideTimer = null;
  const headerHeight = header.offsetHeight;

  window.addEventListener('scroll', function() {
    const current = window.scrollY;
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
}

// --- Code copy buttons ---

document.querySelectorAll('.code-copy-btn').forEach(function(btn) {
  btn.addEventListener('click', function() {
    const code = btn.parentElement.querySelector('code');
    if (!code) return;
    navigator.clipboard.writeText(code.textContent).then(function() {
      const icon = btn.querySelector('.code-copy-icon');
      btn.classList.add('copied');
      icon.classList.add('checked');
      setTimeout(function() {
        btn.classList.remove('copied');
        icon.classList.remove('checked');
      }, 1500);
    });
  });
});

// --- Source toggle (split view) ---

const sourceBtn = document.getElementById('source-toggle');
if (sourceBtn) {
  sourceBtn.addEventListener('click', function() {
    document.body.classList.toggle('split-active');
    sourceBtn.classList.toggle('active');
    const isActive = sourceBtn.classList.contains('active');
    sourceBtn.setAttribute('aria-expanded', isActive ? 'true' : 'false');
    const code = document.getElementById('source-code');
    if (code && code.textContent === 'Loading source...') {
      const url = sourceBtn.getAttribute('data-source');
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

const searchOverlay = document.querySelector('.search-overlay');
const searchToggle = document.querySelector('[data-search-toggle]');

if (searchOverlay && searchToggle) {
  const searchInput = searchOverlay.querySelector('.search-input');
  const resultsEl = searchOverlay.querySelector('.search-results');
  const statusEl = document.getElementById('search-status');
  let selectedIdx = -1;
  let pagefind = null;

  async function loadPagefind() {
    if (pagefind) return pagefind;
    try {
      pagefind = await import('../pagefind/pagefind.js');
    } catch (e) {
      resultsEl.innerHTML = '<div class="search-hint">Search index not available.</div>';
    }
    return pagefind;
  }

  function escHtml(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  async function doSearch(query) {
    if (!query.trim()) {
      resultsEl.innerHTML = '<div class="search-hint">Start typing to search...</div>';
      searchInput.setAttribute('aria-expanded', 'false');
      searchInput.removeAttribute('aria-activedescendant');
      if (statusEl) statusEl.textContent = '';
      selectedIdx = -1;
      return;
    }

    const pf = await loadPagefind();
    if (!pf) return;

    let search, results;
    try {
      search = await pf.search(query);
      results = await Promise.all(search.results.slice(0, 15).map(r => r.data()));
    } catch (e) {
      resultsEl.innerHTML = '<div class="search-hint">Search error.</div>';
      return;
    }
    selectedIdx = -1;
    searchInput.removeAttribute('aria-activedescendant');

    if (results.length === 0) {
      resultsEl.innerHTML = '<div class="search-hint">No results found.</div>';
      searchInput.setAttribute('aria-expanded', 'false');
      if (statusEl) statusEl.textContent = 'No results found.';
      return;
    }

    searchInput.setAttribute('aria-expanded', 'true');
    if (statusEl) statusEl.textContent = results.length + ' result' + (results.length === 1 ? '' : 's') + ' found.';
    resultsEl.innerHTML = results.map((r, i) =>
      '<a class="search-result" id="search-result-' + i + '" role="option" href="' + escHtml(r.url) + '">' +
        '<div class="search-result-title">' + escHtml(r.meta.title || r.url) + '</div>' +
        '<div class="search-result-text">' + (r.excerpt || '') + '</div>' +
      '</a>'
    ).join('');
  }

  function updateSelected() {
    const items = resultsEl.querySelectorAll('.search-result');
    items.forEach((el, i) => {
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
    setTimeout(() => searchInput.focus(), 50);
  }

  function closeSearch() {
    searchOverlay.classList.remove('active');
    searchToggle.focus();
  }

  let debounce = null;
  searchInput.addEventListener('input', function() {
    clearTimeout(debounce);
    debounce = setTimeout(() => doSearch(searchInput.value), 100);
  });

  searchInput.addEventListener('keydown', function(e) {
    const items = resultsEl.querySelectorAll('.search-result');
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
}

// --- TOC active section tracking ---

const tocLinks = document.querySelectorAll('.toc a');
if (tocLinks.length) {
  const headings = [];
  tocLinks.forEach(function(a) {
    const id = a.getAttribute('href');
    if (id) {
      const el = document.querySelector(id);
      if (el) headings.push({ el: el, link: a });
    }
  });
  if (headings.length) {
    const observer = new IntersectionObserver(function(entries) {
      entries.forEach(function(entry) {
        if (entry.isIntersecting) {
          tocLinks.forEach(function(a) { a.classList.remove('active'); });
          const match = headings.find(function(h) { return h.el === entry.target; });
          if (match) {
            match.link.classList.add('active');
            match.link.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
          }
        }
      });
    }, { rootMargin: '0px 0px -80% 0px' });
    headings.forEach(function(h) { observer.observe(h.el); });
  }
}

// --- Footnote hover previews ---

document.querySelectorAll('.footnote-ref a').forEach(function(a) {
  const id = a.getAttribute('href');
  if (!id) return;
  const fn = document.querySelector(id);
  if (!fn) return;
  const text = fn.textContent.replace(/\s*\u21a9\s*$/, '').trim();
  if (!text) return;
  const tip = document.createElement('span');
  tip.className = 'fn-preview';
  tip.textContent = text;
  a.parentElement.appendChild(tip);
});

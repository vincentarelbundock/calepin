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

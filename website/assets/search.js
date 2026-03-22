// Client-side search
(function() {
  var searchIndex = null;
  var overlay = null;
  var input = null;
  var resultsContainer = null;
  var selectedIndex = -1;

  function loadIndex() {
    if (searchIndex) return Promise.resolve(searchIndex);
    return fetch('/search-index.json')
      .then(function(r) { return r.json(); })
      .then(function(data) { searchIndex = data; return data; });
  }

  function open() {
    if (!overlay) return;
    overlay.classList.add('active');
    input.value = '';
    input.focus();
    selectedIndex = -1;
    resultsContainer.innerHTML = '<div class="search-hint">Start typing to search...</div>';
    loadIndex();
  }

  function close() {
    if (!overlay) return;
    overlay.classList.remove('active');
  }

  function search(query) {
    if (!searchIndex || !query.trim()) {
      resultsContainer.innerHTML = '<div class="search-hint">Start typing to search...</div>';
      return;
    }

    var terms = query.toLowerCase().split(/\s+/).filter(Boolean);
    var scored = [];

    for (var i = 0; i < searchIndex.length; i++) {
      var entry = searchIndex[i];
      var titleLower = (entry.title || '').toLowerCase();
      var textLower = (entry.text || '').toLowerCase();
      var headingsLower = (entry.headings || []).join(' ').toLowerCase();
      var score = 0;

      for (var t = 0; t < terms.length; t++) {
        var term = terms[t];
        if (titleLower.indexOf(term) !== -1) score += 10;
        if (headingsLower.indexOf(term) !== -1) score += 5;
        if (textLower.indexOf(term) !== -1) score += 1;
      }

      if (score > 0) {
        scored.push({ entry: entry, score: score });
      }
    }

    scored.sort(function(a, b) { return b.score - a.score; });
    scored = scored.slice(0, 20);

    if (scored.length === 0) {
      resultsContainer.innerHTML = '<div class="search-hint">No results found.</div>';
      selectedIndex = -1;
      return;
    }

    var html = '';
    for (var j = 0; j < scored.length; j++) {
      var e = scored[j].entry;
      var snippet = getSnippet(e.text, terms[0], 100);
      html += '<div class="search-result" data-url="' + escapeAttr(e.url) + '">' +
        '<div class="search-result-title">' + escapeHtml(e.title || e.url) + '</div>' +
        '<div class="search-result-text">' + snippet + '</div>' +
        '</div>';
    }

    resultsContainer.innerHTML = html;
    selectedIndex = -1;
  }

  function getSnippet(text, term, maxLen) {
    if (!text || !term) return '';
    var lower = text.toLowerCase();
    var idx = lower.indexOf(term.toLowerCase());
    var start = Math.max(0, idx - 40);
    var end = Math.min(text.length, start + maxLen);
    var snippet = (start > 0 ? '...' : '') + text.slice(start, end) + (end < text.length ? '...' : '');
    return escapeHtml(snippet);
  }

  function escapeHtml(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  function escapeAttr(s) {
    return s.replace(/&/g, '&amp;').replace(/"/g, '&quot;');
  }

  function navigate(index) {
    var items = resultsContainer.querySelectorAll('.search-result');
    if (items.length === 0) return;

    // Remove previous selection
    for (var i = 0; i < items.length; i++) {
      items[i].classList.remove('selected');
    }

    if (index < 0) index = items.length - 1;
    if (index >= items.length) index = 0;
    selectedIndex = index;

    items[selectedIndex].classList.add('selected');
    items[selectedIndex].scrollIntoView({ block: 'nearest' });
  }

  function goToSelected() {
    var items = resultsContainer.querySelectorAll('.search-result');
    if (selectedIndex >= 0 && selectedIndex < items.length) {
      window.location.href = items[selectedIndex].getAttribute('data-url');
    }
  }

  document.addEventListener('DOMContentLoaded', function() {
    overlay = document.querySelector('.search-overlay');
    if (!overlay) return;

    input = overlay.querySelector('.search-input');
    resultsContainer = overlay.querySelector('.search-results');

    // Open search
    var searchBtns = document.querySelectorAll('[data-search-toggle]');
    for (var i = 0; i < searchBtns.length; i++) {
      searchBtns[i].addEventListener('click', open);
    }

    // Close on overlay click
    overlay.addEventListener('click', function(e) {
      if (e.target === overlay) close();
    });

    // Close on Escape
    document.addEventListener('keydown', function(e) {
      if (e.key === 'Escape' && overlay.classList.contains('active')) {
        close();
      }
      // Ctrl/Cmd+K to open search
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        if (overlay.classList.contains('active')) {
          close();
        } else {
          open();
        }
      }
    });

    // Search on input
    input.addEventListener('input', function() {
      search(input.value);
    });

    // Keyboard navigation
    input.addEventListener('keydown', function(e) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        navigate(selectedIndex + 1);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        navigate(selectedIndex - 1);
      } else if (e.key === 'Enter') {
        e.preventDefault();
        goToSelected();
      }
    });

    // Click on result
    resultsContainer.addEventListener('click', function(e) {
      var result = e.target.closest('.search-result');
      if (result) {
        window.location.href = result.getAttribute('data-url');
      }
    });
  });
})();

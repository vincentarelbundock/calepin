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
    return true;
  }

  function updateSidebarActive(url) {
    var path = new URL(url, location.href).pathname;
    document.querySelectorAll('#sidebar-nav a').forEach(function(a) {
      var aPath = new URL(a.href).pathname;
      a.classList.toggle('active', aPath === path);
      if (aPath === path) {
        var details = a.closest('details.sidebar-section');
        if (details) details.open = true;
      }
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

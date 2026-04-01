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

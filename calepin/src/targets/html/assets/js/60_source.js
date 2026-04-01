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

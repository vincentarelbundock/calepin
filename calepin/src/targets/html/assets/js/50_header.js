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

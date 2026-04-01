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

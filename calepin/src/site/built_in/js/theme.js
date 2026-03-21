// Dark/light mode toggle — runs early to prevent FOUC
(function() {
  const STORAGE_KEY = 'calepin-theme';

  function getPreferred() {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) return stored;
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }

  function apply(theme) {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem(STORAGE_KEY, theme);

    // Update toggle button icons
    const btn = document.getElementById('theme-toggle');
    if (btn) {
      const sunIcon = btn.querySelector('.icon-sun');
      const moonIcon = btn.querySelector('.icon-moon');
      if (sunIcon && moonIcon) {
        sunIcon.style.display = theme === 'dark' ? 'block' : 'none';
        moonIcon.style.display = theme === 'dark' ? 'none' : 'block';
      }
    }
  }

  // Apply immediately (before DOM ready) to prevent flash
  apply(getPreferred());

  // Set up toggle after DOM loads
  document.addEventListener('DOMContentLoaded', function() {
    apply(getPreferred()); // re-apply to update button icons

    var btn = document.getElementById('theme-toggle');
    if (btn) {
      btn.addEventListener('click', function() {
        var current = document.documentElement.getAttribute('data-theme');
        apply(current === 'dark' ? 'light' : 'dark');
      });
    }

    // Mobile sidebar toggle
    var menuBtn = document.getElementById('sidebar-toggle');
    var sidebar = document.querySelector('.sidebar-left');
    if (menuBtn && sidebar) {
      menuBtn.addEventListener('click', function() {
        sidebar.classList.toggle('open');
      });
    }

    // Navbar auto-hide: disappears on scroll down, reappears on mouse move
    var navbar = document.querySelector('.navbar');
    if (navbar) {
      var lastScroll = 0;
      var hideTimer = null;
      var navbarHeight = navbar.offsetHeight;

      window.addEventListener('scroll', function() {
        var current = window.scrollY;
        if (current > navbarHeight && current > lastScroll) {
          navbar.classList.add('hidden');
        }
        lastScroll = current;
        if (current <= navbarHeight) {
          navbar.classList.remove('hidden');
        }
      }, { passive: true });

      document.addEventListener('mousemove', function(e) {
        if (e.clientY < navbarHeight * 2) {
          navbar.classList.remove('hidden');
          clearTimeout(hideTimer);
          hideTimer = setTimeout(function() {
            if (window.scrollY > navbarHeight) {
              navbar.classList.add('hidden');
            }
          }, 1500);
        }
      });
    }
  });
})();

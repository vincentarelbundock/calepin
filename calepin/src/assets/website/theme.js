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
      btn.setAttribute('aria-pressed', theme === 'dark' ? 'true' : 'false');
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
        var isOpen = sidebar.classList.contains('open');
        menuBtn.setAttribute('aria-expanded', isOpen ? 'true' : 'false');
      });
    }

    // Navbar menu toggle (no-sidebar mode)
    var navMenuBtn = document.getElementById('navbar-menu-toggle');
    var navMenu = document.getElementById('navbar-menu');
    if (navMenuBtn && navMenu) {
      navMenuBtn.addEventListener('click', function() {
        navMenu.classList.toggle('open');
        var isOpen = navMenu.classList.contains('open');
        navMenuBtn.setAttribute('aria-expanded', isOpen ? 'true' : 'false');
      });
    }

    // Navbar dropdown toggle
    document.querySelectorAll('.navbar-dropdown-toggle').forEach(function(btn) {
      btn.addEventListener('click', function(e) {
        e.stopPropagation();
        var dropdown = btn.closest('.navbar-dropdown');
        var wasOpen = dropdown.classList.contains('open');
        // Close all dropdowns
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
    // Close dropdowns on outside click
    document.addEventListener('click', function() {
      document.querySelectorAll('.navbar-dropdown.open').forEach(function(d) {
        d.classList.remove('open');
        d.querySelector('.navbar-dropdown-toggle').setAttribute('aria-expanded', 'false');
      });
    });

    // Tabset: click to switch tabs
    document.querySelectorAll('.panel-tabset .nav-link').forEach(function(btn) {
      btn.addEventListener('click', function() {
        var tabset = btn.closest('.panel-tabset');
        var tab = btn.getAttribute('data-tab');
        var group = tabset.getAttribute('data-group');

        // Determine which tabsets to update: all in the same group, or just this one
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
            p.classList.toggle('active', isActive);
            p.setAttribute('aria-hidden', isActive ? 'false' : 'true');
          });
        });
      });
    });

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

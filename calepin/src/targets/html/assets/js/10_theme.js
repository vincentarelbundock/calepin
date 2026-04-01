// --- Theme toggle ---

(function() {
  var btn = document.getElementById('theme-toggle');
  if (!btn) return;

  function current() {
    return document.documentElement.getAttribute('data-theme') ||
      (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
  }

  function update(theme) {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('calepin-theme', theme);
    btn.setAttribute('aria-pressed', theme === 'dark' ? 'true' : 'false');
    var sun = btn.querySelector('.icon-sun');
    var moon = btn.querySelector('.icon-moon');
    if (sun && moon) {
      sun.classList.toggle('hidden', theme !== 'dark');
      moon.classList.toggle('hidden', theme === 'dark');
    }
  }

  update(current());
  btn.addEventListener('click', function() {
    update(current() === 'dark' ? 'light' : 'dark');
  });
})();

// --- Navbar dropdown toggle ---

document.querySelectorAll('.navbar-dropdown-toggle').forEach(function(btn) {
  btn.addEventListener('click', function(e) {
    e.stopPropagation();
    var dropdown = btn.closest('.navbar-dropdown');
    var wasOpen = dropdown.classList.contains('open');
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
document.addEventListener('click', function() {
  document.querySelectorAll('.navbar-dropdown.open').forEach(function(d) {
    d.classList.remove('open');
    d.querySelector('.navbar-dropdown-toggle').setAttribute('aria-expanded', 'false');
  });
});

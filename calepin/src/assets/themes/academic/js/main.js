(() => {
  "use strict";

  function initScrollAwareTopbar() {
    const topbar = document.querySelector(".academic-topbar");
    if (!topbar) return;

    const menu = document.getElementById("academic-menu");
    const scrollThreshold = Math.max(32, topbar.offsetHeight * 1.5);
    let lastScrollY = window.scrollY;
    let ticking = false;

    function menuIsOpen() {
      return menu?.classList.contains("is-open") ?? false;
    }

    function setHidden(hidden) {
      topbar.classList.toggle("is-hidden", hidden);
    }

    function update() {
      const currentScrollY = Math.max(0, window.scrollY);
      const delta = currentScrollY - lastScrollY;
      const belowThreshold = currentScrollY > scrollThreshold;
      const scrollingDown = delta > 6;
      const scrollingUp = delta < -6;

      if (!belowThreshold || scrollingUp || menuIsOpen() || topbar.matches(":focus-within")) {
        setHidden(false);
      } else if (scrollingDown) {
        setHidden(true);
      }

      lastScrollY = currentScrollY;
      ticking = false;
    }

    function requestUpdate() {
      if (ticking) return;
      ticking = true;
      window.requestAnimationFrame(update);
    }

    window.addEventListener("scroll", requestUpdate, { passive: true });
    topbar.addEventListener("focusin", () => setHidden(false));
    topbar.addEventListener("focusout", requestUpdate);
  }

  function initMobileMenu() {
    const button = document.querySelector(".academic-nav-toggle");
    const menu = document.getElementById("academic-menu");
    if (!button || !menu) return;

    const topbar = button.closest(".academic-topbar");

    function setOpen(open) {
      menu.classList.toggle("is-open", open);
      topbar?.classList.remove("is-hidden");
      button.setAttribute("aria-expanded", open ? "true" : "false");
    }

    button.addEventListener("click", () => {
      setOpen(!menu.classList.contains("is-open"));
    });

    menu.addEventListener("click", (event) => {
      if (event.target.closest("a")) setOpen(false);
    });

    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape") setOpen(false);
    });
  }

  initScrollAwareTopbar();
  initMobileMenu();
})();

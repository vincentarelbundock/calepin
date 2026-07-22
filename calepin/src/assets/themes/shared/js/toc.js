(() => {
  "use strict";

  const toc = document.querySelector(".calepin-website-toc");
  if (!toc) return;

  const entries = Array.from(toc.querySelectorAll('a[href^="#"]'))
    .map((link) => {
      const id = decodeURIComponent(link.hash.slice(1));
      return { link, heading: document.getElementById(id) };
    })
    .filter((entry) => entry.heading);
  if (!entries.length) return;

  let activeLink = null;
  let ticking = false;

  function revealInToc(link) {
    const tocBox = toc.getBoundingClientRect();
    const linkBox = link.getBoundingClientRect();
    const padding = 12;

    if (linkBox.top < tocBox.top + padding) {
      toc.scrollBy({ top: linkBox.top - tocBox.top - padding, behavior: "smooth" });
    } else if (linkBox.bottom > tocBox.bottom - padding) {
      toc.scrollBy({ top: linkBox.bottom - tocBox.bottom + padding, behavior: "smooth" });
    }
  }

  function setActive(link) {
    if (link === activeLink) return;
    activeLink?.classList.remove("active");
    activeLink?.removeAttribute("aria-current");
    activeLink = link;
    activeLink.classList.add("active");
    activeLink.setAttribute("aria-current", "location");
    revealInToc(activeLink);
  }

  function update() {
    const topbar = document.querySelector(".calepin-website-topbar, .academic-topbar");
    const threshold = (topbar?.offsetHeight || 0) + 24;
    let current = entries[0];

    for (const entry of entries) {
      if (entry.heading.getBoundingClientRect().top > threshold) break;
      current = entry;
    }

    setActive(current.link);
    ticking = false;
  }

  function requestUpdate() {
    if (ticking) return;
    ticking = true;
    window.requestAnimationFrame(update);
  }

  window.addEventListener("scroll", requestUpdate, { passive: true });
  window.addEventListener("resize", requestUpdate);
  window.addEventListener("hashchange", requestUpdate);
  requestUpdate();
})();

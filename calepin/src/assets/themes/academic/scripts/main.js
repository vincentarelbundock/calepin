(() => {
  "use strict";

  function topbarOffset() {
    const topbar = document.querySelector(".academic-topbar");
    return (topbar ? topbar.getBoundingClientRect().height : 0) + 16;
  }

  function scrollToElement(target) {
    const top = target.getBoundingClientRect().top + window.scrollY - topbarOffset();
    window.scrollTo({ top: Math.max(0, top), behavior: "smooth" });
    if (typeof target.focus === "function") target.focus({ preventScroll: true });
  }

  function updateHash(id) {
    if (!id) return;
    if (history.pushState) {
      history.pushState(null, "", `#${id}`);
    } else {
      window.location.hash = id;
    }
  }

  function enhanceFootnotes() {
    const section = document.querySelector('section[role="doc-endnotes"]');
    if (!section) return;

    const notes = Array.from(section.querySelectorAll("li[id]"));
    let moved = 0;

    notes.forEach((note, index) => {
      const backlink = note.querySelector('sup[role="doc-backlink"] a[href^="#"]');
      const backref = backlink ? backlink.closest('sup[role="doc-backlink"]') : null;
      const refId = backlink ? backlink.getAttribute("href").slice(1) : "";
      const reference = refId ? document.getElementById(refId) : null;
      if (!backref || !reference) return;

      const sidenote = document.createElement("span");
      sidenote.className = "sidenote academic-footnote";
      sidenote.id = note.id;
      sidenote.tabIndex = -1;
      sidenote.setAttribute("role", "note");

      const number = document.createElement("a");
      number.className = "academic-footnote-backref";
      number.href = `#${reference.id}`;
      number.textContent =
        backlink.textContent.trim() || reference.textContent.trim() || String(index + 1);
      number.setAttribute("aria-label", "Back to footnote reference");
      sidenote.append(number);

      backref.remove();
      while (note.firstChild) sidenote.append(note.firstChild);

      reference.insertAdjacentElement("afterend", sidenote);

      const referenceLink = reference.querySelector('a[href^="#"]');
      if (referenceLink) {
        referenceLink.addEventListener("click", (event) => {
          event.preventDefault();
          updateHash(sidenote.id);
          scrollToElement(sidenote);
        });
      }

      number.addEventListener("click", (event) => {
        event.preventDefault();
        updateHash(reference.id);
        scrollToElement(reference);
      });

      note.remove();
      moved += 1;
    });

    if (moved > 0 && !section.querySelector("li")) section.remove();
  }

  enhanceFootnotes();

  const button = document.querySelector(".academic-nav-toggle");
  const menu = document.getElementById("academic-menu");
  if (!button || !menu) return;

  function setOpen(open) {
    menu.classList.toggle("is-open", open);
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
})();

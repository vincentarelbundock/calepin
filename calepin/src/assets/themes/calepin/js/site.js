/* Calepin website behaviour, consolidated.
 *
 * One module-local script wires: view switcher (HTML/Source/PDF), the mobile
 * sidebar drawer, sidebar section folding, inline SVGs, internal link state
 * preservation, the native <dialog> video lightbox, and the Pagefind navbar
 * search bridge.
 *
 * The Rust theme loader links this file near the end of the page body, so
 * the DOM already exists when it runs and no DOMContentLoaded guard is
 * needed. */
(() => {
  "use strict";

  // Single-document render: website behaviors do not apply. Page-specific
  // website layouts may omit the sidebar shell but still use shared topbar
  // controls, dialogs, and inline SVGs.
  const hasWebsiteBehavior = document.querySelector(
    ".calepin-website-topbar, .calepin-website-main, pagefind-modal, img[data-inline-svg], [data-video-dialog], [data-lightbox-dialog]",
  );
  if (!hasWebsiteBehavior) return;

  const VIEW = {
    param: "view",
    rendered: "rendered",
    source: "source",
    pdf: "pdf",
  };

  const MOBILE_QUERY = "(max-width: 56rem)";
  const SOURCE_DATA_ID = "calepin-website-source-data";

  // ---------------------------------------------------------------- helpers

  const isMobile = () => window.matchMedia(MOBILE_QUERY).matches;

  const isPlainLeftClick = (event) =>
    event &&
    event.button === 0 &&
    !event.metaKey &&
    !event.ctrlKey &&
    !event.shiftKey &&
    !event.altKey;

  const isInternalPageHref = (href) =>
    Boolean(href) &&
    !href.startsWith("#") &&
    !href.startsWith("http://") &&
    !href.startsWith("https://") &&
    !href.startsWith("//") &&
    !href.startsWith("mailto:") &&
    !href.startsWith("tel:");

  function setUrlParam(url, name, value) {
    if (value) {
      url.searchParams.set(name, value);
    } else {
      url.searchParams.delete(name);
    }
  }

  function readUrlParam(name, normalize, fallback = "") {
    try {
      const url = new URL(window.location.href);
      return normalize((url.searchParams.get(name) || "").toLowerCase());
    } catch {
      return fallback;
    }
  }

  function updateCurrentUrlParam(name, value) {
    try {
      const url = new URL(window.location.href);
      setUrlParam(url, name, value);
      window.history.replaceState({}, "", url.toString());
    } catch {
      /* ignore URL errors */
    }
  }

  const getMain = () => document.querySelector(".calepin-website-main");

  // ------------------------------------------------------------------- view

  const normalizeView = (value) =>
    value === VIEW.source || value === VIEW.pdf ? value : VIEW.rendered;

  function currentView(selects) {
    const select = Array.isArray(selects) ? selects[0] : selects;
    return normalizeView((select && select.value) || VIEW.rendered);
  }

  function pageAssetCandidates(extension) {
    const { pathname } = window.location;
    if (pathname.endsWith("/")) return [`${pathname}index.${extension}`];
    if (pathname.endsWith(".html")) return [`${pathname.slice(0, -5)}.${extension}`];
    if (!pathname || pathname.endsWith(".")) return [`index.${extension}`];
    if (!pathname.includes(".")) return [`${pathname}/index.${extension}`];
    return [`${pathname}.${extension}`];
  }

  function replaceMainContent(node) {
    const main = getMain();
    if (!main) return false;
    main.innerHTML = "";
    main.appendChild(node);
    return true;
  }

  function showMainFallback(message) {
    const main = getMain();
    if (!main) return;
    const fallback = document.createElement("p");
    fallback.textContent = message;
    main.prepend(fallback);
  }

  function renderSourceText(text) {
    const container = document.createElement("div");
    container.className = "calepin-website-source-view";
    const pre = document.createElement("pre");
    const code = document.createElement("code");
    code.className = "language-typ";
    code.textContent = text;
    pre.appendChild(code);
    container.appendChild(pre);
    replaceMainContent(container);
  }

  async function renderSourceMode() {
    const element = document.getElementById(SOURCE_DATA_ID);
    if (element) {
      try {
        const inline = JSON.parse(element.textContent || "");
        if (typeof inline === "string" && inline.length > 0) {
          renderSourceText(inline);
          return;
        }
      } catch {
        /* fall through to fetch */
      }
    }

    for (const sourcePath of pageAssetCandidates("typ")) {
      try {
        const response = await fetch(sourcePath);
        if (!response.ok) continue;
        renderSourceText(await response.text());
        return;
      } catch {
        /* try next candidate */
      }
    }
    showMainFallback("Could not load source view.");
  }

  async function renderPdfMode() {
    const isLocal = window.location.protocol === "file:";
    for (const pdfPath of pageAssetCandidates("pdf")) {
      if (!isLocal) {
        try {
          const response = await fetch(pdfPath, { method: "HEAD" });
          if (!response.ok && response.status !== 405) continue;
        } catch {
          /* probe failed; render directly */
        }
      }
      const viewer = document.createElement("iframe");
      viewer.className = "calepin-website-pdf-viewer";
      try {
        viewer.src = new URL(pdfPath, window.location.href).toString();
      } catch {
        viewer.src = pdfPath;
      }
      viewer.setAttribute("title", "Page PDF view");
      viewer.setAttribute("loading", "lazy");
      replaceMainContent(viewer);
      return;
    }
    showMainFallback("Could not load PDF view.");
  }

  function applyView(mode, selects) {
    const value = normalizeView(mode);
    (Array.isArray(selects) ? selects : [selects]).filter(Boolean).forEach((select) => {
      select.value = value;
    });
    updateCurrentUrlParam(VIEW.param, value === VIEW.rendered ? "" : value);
    if (value === VIEW.source) renderSourceMode();
    else if (value === VIEW.pdf) renderPdfMode();
  }

  function initView(selects) {
    const viewSelects = (Array.isArray(selects) ? selects : [selects]).filter(Boolean);
    applyView(readUrlParam(VIEW.param, normalizeView, VIEW.rendered), viewSelects);
    viewSelects.forEach((select) => {
      if (select.dataset.calepinBound) return;
      select.dataset.calepinBound = "true";
      select.addEventListener("change", (event) => {
        const value = normalizeView(event.target.value);
        const url = new URL(window.location.href);
        setUrlParam(url, VIEW.param, value === VIEW.rendered ? "" : value);
        window.location.href = url.toString();
      });
    });
  }

  // ----------------------------------------------------------- nav drawer

  function createNav() {
    const shell = document.querySelector(".calepin-website-shell");
    const backdrop = document.querySelector(".calepin-website-nav-backdrop");
    const toggle = document.querySelector(".calepin-website-nav-toggle");

    function setOpen(isOpen) {
      if (!shell || !toggle) return;
      shell.classList.toggle("nav-open", isOpen);
      document.body.classList.toggle("calepin-website-nav-open", isOpen);
      toggle.setAttribute("aria-expanded", String(isOpen));
      if (backdrop) backdrop.setAttribute("aria-hidden", String(!isOpen));
    }

    const close = () => setOpen(false);

    if (toggle) {
      toggle.addEventListener("click", (event) => {
        event.preventDefault();
        if (isMobile() && shell) setOpen(!shell.classList.contains("nav-open"));
      });
    }
    backdrop?.addEventListener("click", close);
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape") close();
    });
    window.addEventListener("resize", () => {
      if (!isMobile()) close();
    });

    return { close };
  }

  // ------------------------------------------------------ sidebar sections

  function initSidebarSections() {
    const sections = Array.from(
      document.querySelectorAll(".calepin-website-sidebar-section"),
    );
    if (sections.length < 2) return;

    sections.forEach((section) => {
      section.addEventListener("toggle", () => {
        if (!section.open) return;
        sections.forEach((other) => {
          if (other !== section) other.removeAttribute("open");
        });
      });
    });
  }

  // ------------------------------------------------------------- inline svg

  function svgTextFromSource(src) {
    if (!src) return Promise.resolve("");
    const prefix = "data:image/svg+xml";
    if (src.startsWith(prefix)) {
      const payload = src.slice(src.indexOf(",") + 1);
      try {
        return Promise.resolve(
          src.startsWith(`${prefix};base64,`) ? atob(payload) : decodeURIComponent(payload),
        );
      } catch {
        return Promise.resolve(payload);
      }
    }
    return fetch(src)
      .then((response) => response.text())
      .catch(() => "");
  }

  function inlineSvgImage(image) {
    const src = image.getAttribute("src");
    return svgTextFromSource(src).then((text) => {
      if (!text) return;
      const svg = new DOMParser()
        .parseFromString(text, "image/svg+xml")
        .querySelector("svg");
      if (!svg) return;

      svg.removeAttribute("xmlns:xlink");
      svg.removeAttribute("width");
      svg.removeAttribute("height");
      if (image.className) svg.setAttribute("class", image.className);
      if (image.id) svg.setAttribute("id", image.id);
      const style = image.getAttribute("style");
      if (style) svg.setAttribute("style", style);

      const alt = image.getAttribute("alt");
      if (alt) {
        svg.setAttribute("aria-label", alt);
        svg.setAttribute("role", "img");
      }
      const ariaHidden = image.getAttribute("aria-hidden");
      if (ariaHidden !== null) svg.setAttribute("aria-hidden", ariaHidden);

      image.replaceWith(svg);
    });
  }

  function inlineSvgs() {
    document
      .querySelectorAll("img[data-inline-svg]")
      .forEach((image) => inlineSvgImage(image));
  }

  // --------------------------------------------------------- link state

  function urlWithState(href, viewSelect) {
    if (!isInternalPageHref(href)) return href;
    try {
      const url = new URL(href, window.location.href);
      const view = currentView(viewSelect);
      setUrlParam(url, VIEW.param, view === VIEW.rendered ? "" : view);
      return url.toString();
    } catch {
      return href;
    }
  }

  function preserveStateInLinks(viewSelect) {
    document.querySelectorAll("a[href]").forEach((link) => {
      const href = link.getAttribute("href");
      if (!isInternalPageHref(href)) return;
      const next = urlWithState(href, viewSelect);
      if (next && next !== href) link.setAttribute("href", next);
    });
  }

  function initLinkInterception(nav, viewSelect) {
    document.addEventListener("click", (event) => {
      const tocLink = event.target.closest(".calepin-website-toc a");
      if (tocLink) {
        const href = tocLink.getAttribute("href");
        if (href && href.startsWith("#")) {
          let target = null;
          try {
            target = document.querySelector(href);
          } catch {
            target = null;
          }
          if (target) target.scrollIntoView({ behavior: "smooth", block: "start" });
        }
        if (isMobile()) nav.close();
        return;
      }

      const link = event.target.closest("a");
      if (!link || !isPlainLeftClick(event)) return;

      const isSidebar = Boolean(event.target.closest(".calepin-website-sidebar a"));
      if (isSidebar && isMobile()) nav.close();

      const href = link.getAttribute("href");
      const next = urlWithState(href, viewSelect);
      if (next && next !== href) {
        event.preventDefault();
        window.location.href = next;
      }
    });
  }

  // ----------------------------------------------------- dialog lightbox

  function initDialogs() {
    document.querySelectorAll("[data-video-dialog], [data-lightbox-dialog]").forEach((trigger) => {
      const dialogId = trigger.dataset.videoDialog || trigger.dataset.lightboxDialog;
      const dialog = document.getElementById(dialogId);
      if (!dialog || typeof dialog.showModal !== "function") return;
      trigger.addEventListener("click", (event) => {
        event.preventDefault();
        dialog.showModal();
      });
    });

    document.querySelectorAll("dialog.calepin-video-dialog, dialog.calepin-screenshot-dialog").forEach((dialog) => {
      const stop = () => {
        const video = dialog.querySelector("video");
        if (video) video.pause();
      };
      dialog.addEventListener("click", (event) => {
        // Clicking the dialog backdrop (the dialog element itself) closes it.
        if (event.target === dialog) dialog.close();
      });
      dialog.querySelectorAll("[data-close-dialog]").forEach((button) => {
        button.addEventListener("click", () => dialog.close());
      });
      dialog.addEventListener("close", stop);
    });
  }

  // --------------------------------------------------------------- bootstrap

  const viewSelects = Array.from(document.querySelectorAll("[data-calepin-view-mode], #calepin-website-view-mode"));

  initView(viewSelects);
  const nav = createNav();
  initSidebarSections();
  inlineSvgs();
  initDialogs();
  preserveStateInLinks(viewSelects);
  initLinkInterception(nav, viewSelects);

  window.addEventListener("pageshow", (event) => {
    if (!event.persisted) return;
    initView(viewSelects);
    preserveStateInLinks(viewSelects);
  });
})();

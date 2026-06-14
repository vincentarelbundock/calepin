/* Calepin website behaviour, consolidated.
 *
 * One module-local script wires: view switcher (HTML/Source/PDF), the mobile
 * sidebar drawer, inline SVGs, internal link state preservation, and the
 * native <dialog> video lightbox, and the Pagefind navbar search bridge.
 *
 * The Rust theme loader inlines this file into a <script> at the end of <body>,
 * so the DOM already exists when it runs and no DOMContentLoaded guard is
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

  function currentView(select) {
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

  function applyView(mode, select) {
    const value = normalizeView(mode);
    if (select) select.value = value;
    updateCurrentUrlParam(VIEW.param, value === VIEW.rendered ? "" : value);
    if (value === VIEW.source) renderSourceMode();
    else if (value === VIEW.pdf) renderPdfMode();
  }

  function initView(select) {
    applyView(readUrlParam(VIEW.param, normalizeView, VIEW.rendered), select);
    if (select && !select.dataset.calepinBound) {
      select.dataset.calepinBound = "true";
      select.addEventListener("change", (event) => {
        const value = normalizeView(event.target.value);
        const url = new URL(window.location.href);
        setUrlParam(url, VIEW.param, value === VIEW.rendered ? "" : value);
        window.location.href = url.toString();
      });
    }
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

  // --------------------------------------------------------------- search

  const pagefindModal = () => document.querySelector("pagefind-modal");

  function modalSearchInput(modal) {
    if (!modal) return null;
    return modal.querySelector("pagefind-input input, input[type='search'], input");
  }

  function setModalSearchQuery(modal, query) {
    if (!query) return false;
    const input = modalSearchInput(modal);
    if (!input) return false;
    input.value = query;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    return true;
  }

  function focusModalSearch(modal) {
    const input = modalSearchInput(modal);
    if (!input) return false;
    input.focus();
    return true;
  }

  async function openPagefindSearch(query) {
    const modal = pagefindModal();
    if (!modal) return false;

    if (typeof modal.open !== "function" && window.customElements?.whenDefined) {
      try {
        await window.customElements.whenDefined("pagefind-modal");
      } catch {
        /* ignore custom element registration errors */
      }
    }

    if (typeof modal.open !== "function") return false;
    modal.open();

    const sync = () => {
      const hasQuery = setModalSearchQuery(modal, query);
      const hasFocus = focusModalSearch(modal);
      return hasQuery || hasFocus;
    };

    if (!sync()) {
      requestAnimationFrame(() => {
        if (!sync()) window.setTimeout(sync, 50);
      });
    }

    return true;
  }

  function isTextEntryElement(element) {
    if (!element) return false;
    return (
      element.isContentEditable ||
      element.tagName === "INPUT" ||
      element.tagName === "SELECT" ||
      element.tagName === "TEXTAREA"
    );
  }

  const SEARCH_SUGGESTION_LIMIT = 20;
  const SEARCH_SUBRESULT_LIMIT = 3;
  const SEARCH_SUGGESTION_MIN_LENGTH = 2;
  const SEARCH_SUGGESTION_DELAY = 160;
  let pagefindModulePromise = null;

  function pagefindBundlePath() {
    const script = document.querySelector(
      'script[src$="pagefind-component-ui.js"], script[src$="pagefind-ui.js"]',
    );
    if (script?.src) return new URL("./", script.src).toString();
    return new URL("pagefind/", window.location.href).toString();
  }

  function loadPagefindModule() {
    if (!pagefindModulePromise) {
      pagefindModulePromise = import(`${pagefindBundlePath()}pagefind.js`);
    }
    return pagefindModulePromise;
  }

  function plainTextFromHtml(html) {
    const element = document.createElement("div");
    element.innerHTML = html || "";
    return element.textContent?.trim() || "";
  }

  function setSearchResultsOpen(input, results, isOpen) {
    results.hidden = !isOpen;
    input.setAttribute("aria-expanded", String(isOpen));
  }

  function clearSearchResults(input, results) {
    results.innerHTML = "";
    setSearchResultsOpen(input, results, false);
  }

  function renderSearchMessage(input, results, message) {
    results.innerHTML = "";
    const item = document.createElement("p");
    item.className = "calepin-website-search-empty";
    item.textContent = message;
    results.appendChild(item);
    setSearchResultsOpen(input, results, true);
  }

  function renderSearchResults(input, results, items) {
    results.innerHTML = "";
    if (!items.length) {
      renderSearchMessage(input, results, "No results");
      return;
    }

    const basePathPrefix = localCanonicalBasePathPrefix();
    const addSearchLink = (item, className, titleClassName, excerptClassName) => {
      const href = hrefWithoutLocalBasePath(item.meta?.url || item.url || "", basePathPrefix);
      if (!href) return null;

      const link = document.createElement("a");
      link.className = className;
      link.href = href;
      link.setAttribute("role", "option");

      const title = document.createElement("span");
      title.className = titleClassName;
      title.textContent = item.meta?.title || item.title || href;
      link.appendChild(title);

      const excerptText = plainTextFromHtml(item.excerpt || item.content || "");
      if (excerptText) {
        const excerpt = document.createElement("span");
        excerpt.className = excerptClassName;
        excerpt.textContent = excerptText;
        link.appendChild(excerpt);
      }

      return link;
    };

    for (const item of items) {
      const link = addSearchLink(
        item,
        "calepin-website-search-result",
        "calepin-website-search-result-title",
        "calepin-website-search-result-excerpt",
      );
      if (!link) continue;
      results.appendChild(link);

      const subResults = Array.isArray(item.sub_results)
        ? item.sub_results.slice(0, SEARCH_SUBRESULT_LIMIT)
        : [];
      if (!subResults.length) continue;

      const nested = document.createElement("div");
      nested.className = "calepin-website-search-subresults";
      for (const subResult of subResults) {
        const subLink = addSearchLink(
          subResult,
          "calepin-website-search-subresult",
          "calepin-website-search-subresult-title",
          "calepin-website-search-subresult-excerpt",
        );
        if (subLink) nested.appendChild(subLink);
      }
      if (nested.children.length) results.appendChild(nested);
    }

    setSearchResultsOpen(input, results, Boolean(results.children.length));
  }

  function initNavbarSearch() {
    const input = document.querySelector("[data-calepin-search-input]");
    if (!input) return;
    const form = input.closest("[data-calepin-search-form]");
    const results = document.querySelector("[data-calepin-search-results]");

    let opening = false;
    let searchTimer = 0;
    let searchToken = 0;
    const openFromInput = async () => {
      if (opening) return;
      opening = true;
      try {
        const opened = await openPagefindSearch(input.value.trim());
        if (opened) input.value = "";
        if (opened && results) clearSearchResults(input, results);
      } finally {
        opening = false;
      }
    };

    const scheduleSuggestions = () => {
      if (!results) return;
      window.clearTimeout(searchTimer);
      const query = input.value.trim();
      const token = ++searchToken;
      if (query.length < SEARCH_SUGGESTION_MIN_LENGTH) {
        clearSearchResults(input, results);
        return;
      }

      searchTimer = window.setTimeout(async () => {
        try {
          const pagefind = await loadPagefindModule();
          const search = await pagefind.search(query);
          if (token !== searchToken || input.value.trim() !== query) return;
          const items = await Promise.all(
            search.results.slice(0, SEARCH_SUGGESTION_LIMIT).map((result) => result.data()),
          );
          if (token !== searchToken || input.value.trim() !== query) return;
          renderSearchResults(input, results, items);
        } catch {
          if (token === searchToken) renderSearchMessage(input, results, "Search unavailable");
        }
      }, SEARCH_SUGGESTION_DELAY);
    };

    form?.addEventListener("submit", (event) => {
      event.preventDefault();
      openFromInput();
    });

    input.addEventListener("input", scheduleSuggestions);
    input.addEventListener("focus", scheduleSuggestions);
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        openFromInput();
      } else if (event.key === "Escape") {
        if (results) clearSearchResults(input, results);
        input.blur();
      }
    });

    document.addEventListener("click", (event) => {
      if (!results || form?.contains(event.target)) return;
      clearSearchResults(input, results);
    });

    document.addEventListener("keydown", (event) => {
      const modifier = /Mac|iPhone|iPad|iPod/.test(window.navigator.platform)
        ? event.metaKey
        : event.ctrlKey;
      if (
        !modifier ||
        event.altKey ||
        event.shiftKey ||
        event.key.toLowerCase() !== "k" ||
        isTextEntryElement(document.activeElement)
      ) {
        return;
      }

      event.preventDefault();
      input.focus();
    });
  }

  function isLoopbackHost(hostname) {
    return (
      hostname === "localhost" ||
      hostname === "127.0.0.1" ||
      hostname === "0.0.0.0" ||
      hostname === "::1" ||
      hostname === "[::1]"
    );
  }

  function localCanonicalBasePathPrefix() {
    if (!isLoopbackHost(window.location.hostname)) return "";
    const canonical = document.querySelector('link[rel="canonical"]')?.href;
    if (!canonical) return "";

    try {
      const canonicalPath = new URL(canonical).pathname;
      const currentPath = window.location.pathname || "/";
      if (canonicalPath === currentPath) return "";
      if (canonicalPath.endsWith(currentPath)) {
        return canonicalPath.slice(0, -currentPath.length).replace(/\/$/, "");
      }
      if (canonicalPath.endsWith("/index.html")) {
        const indexPath = currentPath.endsWith("/") ? `${currentPath}index.html` : currentPath;
        if (canonicalPath.endsWith(indexPath)) {
          return canonicalPath.slice(0, -indexPath.length).replace(/\/$/, "");
        }
      }
    } catch {
      return "";
    }
    return "";
  }

  function hrefWithoutLocalBasePath(href, basePathPrefix) {
    if (!href || !basePathPrefix) return href;
    try {
      const url = new URL(href, window.location.href);
      if (url.origin !== window.location.origin) return href;
      if (url.pathname !== basePathPrefix && !url.pathname.startsWith(`${basePathPrefix}/`)) {
        return href;
      }
      url.pathname = url.pathname.slice(basePathPrefix.length) || "/";
      return url.toString();
    } catch {
      return href;
    }
  }

  function initPagefindLocalLinks() {
    const basePathPrefix = localCanonicalBasePathPrefix();
    if (!basePathPrefix) return;

    const selector =
      "pagefind-modal a[href], pagefind-searchbox a[href], .calepin-website-search-results a[href]";
    const normalizeLinks = () => {
      document.querySelectorAll(selector).forEach((link) => {
        const next = hrefWithoutLocalBasePath(link.getAttribute("href"), basePathPrefix);
        if (next && next !== link.getAttribute("href")) link.setAttribute("href", next);
      });
    };

    document.addEventListener("click", (event) => {
      const link = event.target.closest(selector);
      if (!link) return;
      const next = hrefWithoutLocalBasePath(link.getAttribute("href"), basePathPrefix);
      if (!next || next === link.getAttribute("href")) return;
      event.preventDefault();
      window.location.href = next;
    });

    normalizeLinks();
    new MutationObserver(normalizeLinks).observe(document.body, {
      childList: true,
      subtree: true,
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

  const viewSelect = document.getElementById("calepin-website-view-mode");

  initView(viewSelect);
  const nav = createNav();
  inlineSvgs();
  initDialogs();
  initNavbarSearch();
  initPagefindLocalLinks();
  preserveStateInLinks(viewSelect);
  initLinkInterception(nav, viewSelect);

  window.addEventListener("pageshow", (event) => {
    if (!event.persisted) return;
    initView(viewSelect);
    preserveStateInLinks(viewSelect);
  });
})();

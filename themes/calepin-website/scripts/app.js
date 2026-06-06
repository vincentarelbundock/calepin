/* Calepin website behaviour, consolidated.
 *
 * One module-local script wires: theme toggle, view switcher (HTML/Source/PDF),
 * the mobile sidebar drawer, copy-to-clipboard buttons, inline SVGs, internal
 * link state preservation, and the native <dialog> video lightbox.
 *
 * The Rust theme loader inlines this file into a <script> at the end of <body>,
 * so the DOM already exists when it runs and no DOMContentLoaded guard is
 * needed. Theme is applied first to keep any flash as short as possible. */
(() => {
  "use strict";

  const THEME = {
    storageKey: "calepin-website-theme",
    param: "theme",
    order: ["", "light", "dark"],
    labels: { "": "Theme: Auto", light: "Theme: Light", dark: "Theme: Dark" },
  };

  const VIEW = {
    storageKey: "calepin-website-view-mode",
    param: "view",
    rendered: "rendered",
    source: "source",
    pdf: "pdf",
  };

  const MOBILE_QUERY = "(max-width: 56rem)";
  const SOURCE_DATA_ID = "calepin-website-source-data";

  const THEME_ICONS = {
    "": `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"></circle><path d="M12 3a9 9 0 0 0 0 18" fill="currentColor" opacity="0.32" stroke="none"></path></svg>`,
    light: `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4"></circle><path d="M12 2v2"></path><path d="M12 20v2"></path><path d="m4.93 4.93 1.41 1.41"></path><path d="m17.66 17.66 1.41 1.41"></path><path d="M2 12h2"></path><path d="M20 12h2"></path><path d="m6.34 17.66-1.41 1.41"></path><path d="m19.07 4.93-1.41 1.41"></path></svg>`,
    dark: `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.99 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 20.99 12.79z"></path></svg>`,
  };

  const COPY_ICON = `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="8" y="2" width="10" height="14" rx="2"></rect><path d="M16 8h-8"></path><path d="M16 11h-8"></path><path d="M16 14h-5"></path><path d="M4 6h-1a1 1 0 0 0-1 1v14a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2v-1"></path></svg>`;
  const COPIED_ICON = `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6L9 17l-5-5"></path></svg>`;

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

  function readStored(key, normalize, fallback = "") {
    try {
      return normalize(localStorage.getItem(key) || "");
    } catch {
      return fallback;
    }
  }

  function writeStored(key, value, fallback = "") {
    try {
      if (value === fallback) {
        localStorage.removeItem(key);
      } else {
        localStorage.setItem(key, value);
      }
    } catch {
      /* ignore storage errors */
    }
  }

  const getMain = () => document.querySelector(".calepin-website-main");

  // ------------------------------------------------------------------ theme

  const normalizeTheme = (value) =>
    value === "light" || value === "dark" ? value : "";

  function currentTheme() {
    return normalizeTheme(document.documentElement.dataset.theme || "");
  }

  function applyTheme(themeName, button) {
    const next = normalizeTheme(themeName);
    if (next) {
      document.documentElement.dataset.theme = next;
    } else {
      delete document.documentElement.dataset.theme;
    }
    updateCurrentUrlParam(THEME.param, next);

    if (button) {
      const label = THEME.labels[next] || THEME.labels[""];
      button.innerHTML = THEME_ICONS[next] || THEME_ICONS[""];
      button.setAttribute("aria-label", label);
      button.setAttribute("title", label);
    }
  }

  function readTheme() {
    const fromUrl = readUrlParam(THEME.param, normalizeTheme);
    if (fromUrl) {
      writeStored(THEME.storageKey, fromUrl);
      return fromUrl;
    }
    return readStored(THEME.storageKey, normalizeTheme, "");
  }

  function initTheme(button) {
    applyTheme(readTheme(), button);
    if (button && !button.dataset.calepinBound) {
      button.dataset.calepinBound = "true";
      button.addEventListener("click", () => {
        const index = THEME.order.indexOf(currentTheme());
        const next = THEME.order[(index + 1) % THEME.order.length];
        applyTheme(next, button);
        writeStored(THEME.storageKey, next);
      });
    }
  }

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
        writeStored(VIEW.storageKey, value, VIEW.rendered);
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
        if (isMobile()) setOpen(!shell.classList.contains("nav-open"));
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

  // ----------------------------------------------------------- copy buttons

  function copyText(text) {
    if (!text) return Promise.resolve();
    if (navigator.clipboard?.writeText) {
      return navigator.clipboard.writeText(text);
    }
    return new Promise((resolve, reject) => {
      const textarea = document.createElement("textarea");
      textarea.value = text;
      textarea.style.position = "fixed";
      textarea.style.opacity = "0";
      document.body.appendChild(textarea);
      textarea.select();
      try {
        if (document.execCommand("copy")) resolve();
        else reject(new Error("copy failed"));
      } catch (error) {
        reject(error);
      } finally {
        document.body.removeChild(textarea);
      }
    });
  }

  function copyTextForBlock(block) {
    const pre = block.matches("pre")
      ? block
      : block.querySelector("pre") || block.querySelector("code");
    if (!pre) return "";
    return (pre.innerText || pre.textContent || "")
      .replace(/\r\n/g, "\n")
      .replace(/ /g, " ");
  }

  const COPY_WRAPPER_SELECTOR =
    ".sourceCode, .cell-output, .cell-output-stdout, .cell-output-stderr";

  function ensureCopyHost(block) {
    if (block.parentElement && block.parentElement.classList.contains("calepin-website-copy-code-host")) {
      return block.parentElement;
    }

    const host = document.createElement("div");
    host.className = "calepin-website-copy-code-host";
    block.parentNode.insertBefore(host, block);
    host.appendChild(block);
    return host;
  }

  function injectCopyButtons() {
    const root = getMain();
    if (!root) return;

    const blocks = new Set();
    root.querySelectorAll("pre").forEach((pre) => {
      blocks.add(pre.closest(COPY_WRAPPER_SELECTOR) || pre);
    });

    blocks.forEach((block) => {
      if (block.dataset.calepinCopyButton === "true") return;

      block.classList.add("calepin-website-copy-code-scroll");

      const host = ensureCopyHost(block);
      if (host.dataset.calepinCopyButton === "true") return;

      host.dataset.calepinCopyButton = "true";
      host.classList.add("calepin-website-copy-code-block");

      const button = document.createElement("button");
      button.type = "button";
      button.className = "calepin-website-copy-code-button";
      button.innerHTML = COPY_ICON;
      button.setAttribute("aria-label", "Copy code block");
      button.title = "Copy code";
      host.appendChild(button);
    });
  }

  function initCopy() {
    injectCopyButtons();

    const root = getMain();
    if (!root || root.dataset.calepinCopyBound === "true") return;
    root.dataset.calepinCopyBound = "true";

    // One delegated listener for every copy button under <main>.
    root.addEventListener("click", async (event) => {
      const button = event.target.closest(".calepin-website-copy-code-button");
      if (!button || !root.contains(button)) return;
      event.preventDefault();

      const block = button.closest(".calepin-website-copy-code-block");
      const text = block ? copyTextForBlock(block) : "";
      if (!text) return;

      try {
        await copyText(text);
        button.innerHTML = COPIED_ICON;
        button.classList.add("calepin-website-copy-code-button--copied");
        button.title = "Copied";
        window.setTimeout(() => {
          button.innerHTML = COPY_ICON;
          button.classList.remove("calepin-website-copy-code-button--copied");
          button.title = "Copy code";
        }, 1100);
      } catch {
        button.classList.add("calepin-website-copy-code-button--error");
        button.title = "Copy failed";
        window.setTimeout(() => {
          button.classList.remove("calepin-website-copy-code-button--error");
        }, 1100);
      }
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

  function urlWithState(href, themeButton, viewSelect) {
    if (!isInternalPageHref(href)) return href;
    try {
      const url = new URL(href, window.location.href);
      setUrlParam(url, THEME.param, currentTheme());
      const view = currentView(viewSelect);
      setUrlParam(url, VIEW.param, view === VIEW.rendered ? "" : view);
      return url.toString();
    } catch {
      return href;
    }
  }

  function preserveStateInLinks(themeButton, viewSelect) {
    document.querySelectorAll("a[href]").forEach((link) => {
      const href = link.getAttribute("href");
      if (!isInternalPageHref(href)) return;
      const next = urlWithState(href, themeButton, viewSelect);
      if (next && next !== href) link.setAttribute("href", next);
    });
  }

  function initLinkInterception(nav, themeButton, viewSelect) {
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
      const next = urlWithState(href, themeButton, viewSelect);
      if (next && next !== href) {
        event.preventDefault();
        window.location.href = next;
      }
    });
  }

  // ----------------------------------------------------- dialog lightbox

  function initDialogs() {
    document.querySelectorAll("[data-video-dialog]").forEach((trigger) => {
      const dialog = document.getElementById(trigger.dataset.videoDialog);
      if (!dialog || typeof dialog.showModal !== "function") return;
      trigger.addEventListener("click", (event) => {
        event.preventDefault();
        dialog.showModal();
      });
    });

    document.querySelectorAll("dialog.calepin-video-dialog").forEach((dialog) => {
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

  const themeButton = document.getElementById("calepin-website-theme-button");
  const viewSelect = document.getElementById("calepin-website-view-mode");

  initTheme(themeButton);
  initView(viewSelect);
  const nav = createNav();
  initCopy();
  inlineSvgs();
  initDialogs();
  preserveStateInLinks(themeButton, viewSelect);
  initLinkInterception(nav, themeButton, viewSelect);

  window.addEventListener("pageshow", (event) => {
    if (!event.persisted) return;
    initTheme(themeButton);
    initView(viewSelect);
    initCopy();
    preserveStateInLinks(themeButton, viewSelect);
  });

  window.addEventListener("storage", (event) => {
    if (event.key === THEME.storageKey) {
      initTheme(themeButton);
      preserveStateInLinks(themeButton, viewSelect);
    } else if (event.key === VIEW.storageKey) {
      initView(viewSelect);
    }
  });
})();

(() => {
  "use strict";

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

  function initMobileSearchTriggers() {
    document.querySelectorAll("[data-calepin-mobile-search-trigger]").forEach((button) => {
      if (button.dataset.calepinSearchBound === "true") return;
      button.dataset.calepinSearchBound = "true";
      button.addEventListener("click", (event) => {
        event.preventDefault();
        openPagefindSearch("");
      });
    });
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

  const hasSearch = !!(
    document.querySelector("pagefind-modal") ||
    document.querySelector("pagefind-searchbox") ||
    document.querySelector("[data-calepin-search-input]") ||
    document.querySelector("[data-calepin-search-results]") ||
    document.querySelector("[data-calepin-mobile-search-trigger]") ||
    document.querySelector(".calepin-website-search-results")
  );
  if (!hasSearch) return;

  initMobileSearchTriggers();
  initNavbarSearch();
  initPagefindLocalLinks();
})();

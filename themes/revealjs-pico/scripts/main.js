(() => {
  "use strict";

  function collectSlidesFromHtml() {
    const slides = document.querySelector(".revealjs-theme .slides");
    if (!slides || slides.dataset.revealjsPrepared === "1") {
      return false;
    }

    const nodes = Array.from(slides.childNodes);
    if (nodes.length === 0) {
      slides.dataset.revealjsPrepared = "1";
      return true;
    }

    const headingLevels = nodes
      .filter((node) => node.nodeType === 1 && /^H[1-6]$/.test(node.tagName))
      .map((node) => Number(node.tagName.slice(1)));
    const horizontalLevel = headingLevels.length > 0 ? Math.min(...headingLevels) : 1;
    const slideLevels = headingLevels.filter((level) => level > horizontalLevel);
    const slideLevel = slideLevels.length > 0 ? Math.min(...slideLevels) : null;
    let currentHorizontal = null;
    let currentSlide = null;
    let pendingSectionContent = [];

    function newFlatSlide(node) {
      currentHorizontal = document.createElement("section");
      currentHorizontal.appendChild(node);
      slides.appendChild(currentHorizontal);
      currentSlide = currentHorizontal;
    }

    function newSection() {
      currentHorizontal = document.createElement("section");
      slides.appendChild(currentHorizontal);
      currentSlide = null;
      pendingSectionContent = [];
    }

    function newSectionSlide(node) {
      if (!currentHorizontal) {
        newSection();
      }
      currentSlide = document.createElement("section");
      for (const pending of pendingSectionContent) {
        currentSlide.appendChild(pending);
      }
      pendingSectionContent = [];
      currentSlide.appendChild(node);
      currentHorizontal.appendChild(currentSlide);
    }

    function appendToCurrent(node) {
      if (currentSlide) {
        currentSlide.appendChild(node);
      } else if (slideLevel === null) {
        newFlatSlide(document.createDocumentFragment());
        currentSlide.appendChild(node);
      } else {
        pendingSectionContent.push(node);
      }
    }

    slides.textContent = "";

    for (const node of nodes) {
      const isElement = node.nodeType === 1;
      const headingLevel = isElement ? Number(node.tagName.slice(1)) : NaN;
      if (slideLevel !== null && headingLevel === horizontalLevel) {
        newSection();
      } else if (slideLevel !== null && headingLevel === slideLevel) {
        newSectionSlide(node);
      } else if (slideLevel === null && headingLevel === horizontalLevel) {
        newFlatSlide(node);
      } else {
        appendToCurrent(node);
      }
    }

    slides.dataset.revealjsPrepared = "1";
    return true;
  }

  const DEFAULT_REVEAL_OPTIONS = {
    hash: true,
    controls: true,
    progress: true,
    controlsTutorial: false,
    slideNumber: "c/t",
    center: false,
    transition: "slide",
    plugins: ["markdown", "highlight", "notes"],
  };

  function resolvePlugins(raw) {
    const pluginMap = {
      markdown: window.RevealMarkdown,
      highlight: window.RevealHighlight,
      notes: window.RevealNotes,
    };

    const names = Array.isArray(raw) ? raw : DEFAULT_REVEAL_OPTIONS.plugins;
    const plugins = [];
    for (const name of names) {
      const plugin = pluginMap[name];
      if (plugin) {
        plugins.push(plugin);
      }
    }
    return plugins;
  }

  function initializeReveal() {
    if (!window.Reveal || typeof window.Reveal.initialize !== "function") {
      return false;
    }

    const config = typeof window.__REVEALJS_OPTIONS === "object" ? window.__REVEALJS_OPTIONS : {};
    const options = {
      ...DEFAULT_REVEAL_OPTIONS,
      ...config,
    };

    if (config && config.plugins) {
      options.plugins = resolvePlugins(config.plugins);
    } else {
      options.plugins = resolvePlugins(DEFAULT_REVEAL_OPTIONS.plugins);
    }

    window.Reveal.initialize(options);
    return true;
  }

  function bootstrap() {
    collectSlidesFromHtml();
    if (!initializeReveal()) {
      requestAnimationFrame(bootstrap);
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", bootstrap, { once: true });
  } else {
    bootstrap();
  }
})();

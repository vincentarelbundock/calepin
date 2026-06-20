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

    function newFlatSlide(node) {
      currentHorizontal = document.createElement("section");
      currentHorizontal.appendChild(node);
      slides.appendChild(currentHorizontal);
      currentSlide = currentHorizontal;
    }

    function newSection(node) {
      currentHorizontal = document.createElement("section");
      slides.appendChild(currentHorizontal);
      currentSlide = document.createElement("section");
      currentSlide.appendChild(node);
      currentHorizontal.appendChild(currentSlide);
    }

    function newSectionSlide(node) {
      if (!currentHorizontal) {
        newSection(document.createElement("div"));
      }
      currentSlide = document.createElement("section");
      currentSlide.appendChild(node);
      currentHorizontal.appendChild(currentSlide);
    }

    function appendToCurrent(node) {
      if (!currentSlide) {
        if (slideLevel === null) {
          newFlatSlide(document.createDocumentFragment());
        } else {
          newSection(document.createElement("div"));
        }
      }
      currentSlide.appendChild(node);
    }

    slides.textContent = "";

    for (const node of nodes) {
      const isElement = node.nodeType === 1;
      const headingLevel = isElement ? Number(node.tagName.slice(1)) : NaN;
      if (slideLevel !== null && headingLevel === horizontalLevel) {
        newSection(node);
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

  function initializeReveal() {
    if (!window.Reveal || typeof window.Reveal.initialize !== "function") {
      return false;
    }

    const plugins = [];
    if (window.RevealMarkdown) plugins.push(window.RevealMarkdown);
    if (window.RevealHighlight) plugins.push(window.RevealHighlight);
    if (window.RevealNotes) plugins.push(window.RevealNotes);

    window.Reveal.initialize({
      hash: true,
      controls: true,
      progress: true,
      controlsTutorial: false,
      slideNumber: "c/t",
      center: false,
      transition: "slide",
      plugins,
    });

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

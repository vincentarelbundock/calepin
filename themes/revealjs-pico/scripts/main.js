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
    const verticalLevel = Math.min(horizontalLevel + 1, 6);

    let currentHorizontal = null;
    let currentVertical = null;

    function newHorizontal(node) {
      currentHorizontal = document.createElement("section");
      currentHorizontal.appendChild(node);
      slides.appendChild(currentHorizontal);
      currentVertical = null;
    }

    function newVertical(node) {
      if (!currentHorizontal) {
        newHorizontal(document.createElement("div"));
      }
      const horizontalChildren = Array.from(currentHorizontal.childNodes);
      const hasHorizontalContent = horizontalChildren.some(
        (child) => !(child.nodeType === 1 && child.tagName.toLowerCase() === "section"),
      );
      if (hasHorizontalContent) {
        const titleSlide = document.createElement("section");
        for (const child of horizontalChildren) {
          titleSlide.appendChild(child);
        }
        currentHorizontal.appendChild(titleSlide);
      }
      currentVertical = document.createElement("section");
      currentVertical.appendChild(node);
      currentHorizontal.appendChild(currentVertical);
    }

    function appendToCurrent(node) {
      if (!currentHorizontal) {
        newHorizontal(document.createDocumentFragment());
      }
      (currentVertical || currentHorizontal).appendChild(node);
    }

    slides.textContent = "";

    for (const node of nodes) {
      const isElement = node.nodeType === 1;
      if (isElement && Number(node.tagName.slice(1)) === horizontalLevel) {
        newHorizontal(node);
      } else if (isElement && Number(node.tagName.slice(1)) === verticalLevel) {
        newVertical(node);
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

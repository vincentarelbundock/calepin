(() => {
  "use strict";

  const selector = window.CalepinCopyCode?.selector ||
    "div.sourceCode, pre.sourceCode, .cell-output, .cell-output-stdout, .cell-output-stderr";
  const buttonClass = window.CalepinCopyCode?.buttonClass || "calepin-copy-code";
  const copiedClass = window.CalepinCopyCode?.copiedClass || "copied";
  const icon = window.CalepinCopyCode?.icon ||
    `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>`;
  const copiedIcon = window.CalepinCopyCode?.copiedIcon ||
    `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17 4 12"></path></svg>`;

  function copyText(text) {
    if (navigator.clipboard?.writeText) {
      return navigator.clipboard.writeText(text);
    }
    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.setAttribute("readonly", "");
    textarea.style.position = "fixed";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    textarea.select();
    try {
      document.execCommand("copy");
    } finally {
      textarea.remove();
    }
    return Promise.resolve();
  }

  function codeText(node) {
    if (!node) return "";
    if (node.nodeType === Node.TEXT_NODE) return node.nodeValue || "";
    if (node.nodeType !== Node.ELEMENT_NODE) return "";
    if (node.tagName === "BR") return "\n";
    return Array.from(node.childNodes).map(codeText).join("");
  }

  document.querySelectorAll(selector).forEach((block) => {
    if (block.dataset.calepinCopyBound === "true" || block.querySelector(`:scope > .${buttonClass}`)) {
      return;
    }
    block.dataset.calepinCopyBound = "true";
    const button = document.createElement("button");
    button.type = "button";
    button.className = buttonClass;
    button.setAttribute("aria-label", "Copy code");
    button.setAttribute("title", "Copy code");
    button.innerHTML = icon;
    let restoreTimeout = null;
    button.addEventListener("click", async () => {
      if (restoreTimeout !== null) {
        window.clearTimeout(restoreTimeout);
        restoreTimeout = null;
      }
      const code = block.querySelector("pre code, code, pre");
      const text = codeText(code);
      try {
        await copyText(text);
        button.classList.add(copiedClass);
        button.innerHTML = copiedIcon;
        restoreTimeout = window.setTimeout(() => {
          button.classList.remove(copiedClass);
          button.innerHTML = icon;
          restoreTimeout = null;
        }, 900);
      } catch {
        button.classList.remove(copiedClass);
        button.innerHTML = icon;
      }
    });
    block.prepend(button);
  });
})();

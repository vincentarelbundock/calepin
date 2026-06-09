(() => {
  const themeButton = document.getElementById("calepin-theme-button");
  const themeStorageKey = "calepin-pico-theme";
  const themeOrder = ["", "light", "dark"];
  const themeLabels = {
    "": "Theme: Auto",
    light: "Theme: Light",
    dark: "Theme: Dark"
  };
  const themeIcons = {
    "": `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"></circle><path d="M12 3a9 9 0 0 0 0 18" fill="currentColor" opacity="0.32" stroke="none"></path></svg>`,
    light: `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4"></circle><path d="M12 2v2"></path><path d="M12 20v2"></path><path d="m4.93 4.93 1.41 1.41"></path><path d="m17.66 17.66 1.41 1.41"></path><path d="M2 12h2"></path><path d="M20 12h2"></path><path d="m6.34 17.66-1.41 1.41"></path><path d="m19.07 4.93-1.41 1.41"></path></svg>`,
    dark: `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.99 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 20.99 12.79z"></path></svg>`
  };
  const copyIcon = `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>`;

  function normalizeTheme(value) {
    return value === "light" || value === "dark" ? value : "";
  }

  function applyTheme(theme) {
    if (theme) {
      document.documentElement.dataset.theme = theme;
    } else {
      delete document.documentElement.dataset.theme;
    }
    if (themeButton) {
      const label = themeLabels[theme] || themeLabels[""];
      themeButton.innerHTML = themeIcons[theme] || themeIcons[""];
      themeButton.setAttribute("aria-label", label);
      themeButton.setAttribute("title", label);
    }
  }

  let storedTheme = "";
  try {
    storedTheme = normalizeTheme(localStorage.getItem("calepin-pico-theme"));
  } catch (error) {
    storedTheme = "";
  }
  applyTheme(storedTheme);

  if (themeButton) {
    themeButton.addEventListener("click", () => {
      const currentTheme = normalizeTheme(document.documentElement.dataset.theme || "");
      const currentIndex = themeOrder.indexOf(currentTheme);
      const theme = themeOrder[(currentIndex + 1) % themeOrder.length];
      applyTheme(theme);
      try {
        if (theme) {
          localStorage.setItem(themeStorageKey, theme);
        } else {
          localStorage.removeItem(themeStorageKey);
        }
      } catch (error) {
      }
    });
  }

  function copyText(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
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
    if (!node) {
      return "";
    }
    if (node.nodeType === Node.TEXT_NODE) {
      return node.nodeValue || "";
    }
    if (node.nodeType !== Node.ELEMENT_NODE) {
      return "";
    }
    if (node.tagName === "BR") {
      return "\n";
    }
    return Array.from(node.childNodes).map(codeText).join("");
  }

  document.querySelectorAll("div.sourceCode, .cell-output").forEach((block) => {
    if (block.querySelector(":scope > .calepin-copy-code")) {
      return;
    }
    const button = document.createElement("button");
    button.type = "button";
    button.className = "calepin-copy-code";
    button.setAttribute("aria-label", "Copy code");
    button.setAttribute("title", "Copy code");
    button.innerHTML = copyIcon;
    button.addEventListener("click", async () => {
      const code = block.querySelector("pre code, code, pre");
      const text = codeText(code);
      try {
        await copyText(text);
        button.classList.add("copied");
        window.setTimeout(() => button.classList.remove("copied"), 900);
      } catch (error) {
        button.classList.remove("copied");
      }
    });
    block.prepend(button);
  });
})();

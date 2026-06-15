(() => {
  "use strict";

  const buttons = document.querySelectorAll("[data-calepin-theme-toggle], #calepin-theme-button");
  if (!buttons.length) return;

  const order = ["", "light", "dark"];
  const labels = { "": "Theme: Auto", light: "Theme: Light", dark: "Theme: Dark" };
  const icons = {
    "": `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"></circle><path d="M12 3a9 9 0 0 0 0 18" fill="currentColor" opacity="0.32" stroke="none"></path></svg>`,
    light: `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4"></circle><path d="M12 2v2"></path><path d="M12 20v2"></path><path d="m4.93 4.93 1.41 1.41"></path><path d="m17.66 17.66 1.41 1.41"></path><path d="M2 12h2"></path><path d="M20 12h2"></path><path d="m6.34 17.66-1.41 1.41"></path><path d="m19.07 4.93-1.41 1.41"></path></svg>`,
    dark: `<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.99 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 20.99 12.79z"></path></svg>`,
  };

  const normalize = (value) => value === "light" || value === "dark" ? value : "";
  const root = document.documentElement;
  const media = window.matchMedia ? window.matchMedia("(prefers-color-scheme: dark)") : null;
  const storageKey = (button) => button.dataset.calepinThemeStorageKey || "calepin-theme";
  const urlParam = (button) => button.dataset.calepinThemeParam || "";
  const systemTheme = () => media && media.matches ? "dark" : "light";

  function readUrl(button) {
    const param = urlParam(button);
    if (!param) return "";
    try {
      return normalize(new URL(window.location.href).searchParams.get(param) || "");
    } catch {
      return "";
    }
  }

  function clearUrl(button) {
    const param = urlParam(button);
    if (!param) return;
    try {
      const url = new URL(window.location.href);
      url.searchParams.delete(param);
      window.history.replaceState({}, "", url.toString());
    } catch {
      /* ignore URL errors */
    }
  }

  function readStored(button) {
    try {
      return normalize(localStorage.getItem(storageKey(button)) || "");
    } catch {
      return "";
    }
  }

  function writeStored(button, value) {
    try {
      if (value) localStorage.setItem(storageKey(button), value);
      else localStorage.removeItem(storageKey(button));
    } catch {
      /* ignore storage errors */
    }
  }

  function readTheme(button) {
    const fromUrl = readUrl(button);
    if (fromUrl) {
      writeStored(button, fromUrl);
      clearUrl(button);
      return fromUrl;
    }
    return readStored(button);
  }

  function applyTheme(value) {
    const mode = normalize(value);
    const theme = mode || systemTheme();
    root.dataset.calepinThemeMode = mode;
    root.dataset.theme = theme;
    root.style.colorScheme = theme;
    buttons.forEach((button) => {
      const label = labels[mode] || labels[""];
      button.innerHTML = icons[mode] || icons[""];
      button.setAttribute("aria-label", label);
      button.setAttribute("title", label);
    });
  }

  applyTheme(readTheme(buttons[0]));

  buttons.forEach((button) => {
    if (button.dataset.calepinThemeBound === "true") return;
    button.dataset.calepinThemeBound = "true";
    button.addEventListener("click", () => {
      const current = normalize(root.dataset.calepinThemeMode || "");
      const next = order[(order.indexOf(current) + 1) % order.length];
      applyTheme(next);
      writeStored(button, next);
    });
  });

  window.addEventListener("pageshow", (event) => {
    if (event.persisted) applyTheme(readTheme(buttons[0]));
  });

  window.addEventListener("storage", (event) => {
    if (event.key === storageKey(buttons[0])) applyTheme(readStored(buttons[0]));
  });

  if (media) {
    const updateSystemTheme = () => {
      if (!normalize(root.dataset.calepinThemeMode || "")) applyTheme("");
    };
    if (media.addEventListener) media.addEventListener("change", updateSystemTheme);
    else if (media.addListener) media.addListener(updateSystemTheme);
  }
})();

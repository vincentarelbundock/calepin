(() => {
  "use strict";

  const currentScript = document.currentScript;
  const storageKey = currentScript?.dataset?.calepinThemeStorageKey || "calepin-website-theme";
  const param = currentScript?.dataset?.calepinThemeParam || "theme";
  const normalize = (value) => (value === "light" || value === "dark" ? value : "");
  const root = document.documentElement;
  const media = window.matchMedia ? window.matchMedia("(prefers-color-scheme: dark)") : null;
  const systemTheme = () => (media && media.matches ? "dark" : "light");

  let mode = "";
  if (param) {
    try {
      mode = normalize(new URL(window.location.href).searchParams.get(param) || "");
    } catch {
      mode = "";
    }
  }

  if (!mode) {
    try {
      mode = normalize(localStorage.getItem(storageKey) || "");
    } catch {
      mode = "";
    }
  }

  const theme = mode || systemTheme();
  root.dataset.calepinThemeMode = mode;
  root.dataset.theme = theme;
  root.style.colorScheme = theme;
})();

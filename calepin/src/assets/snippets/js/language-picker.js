(() => {
  "use strict";

  const selects = document.querySelectorAll("select[data-calepin-language-picker]");
  if (!selects.length) return;

  function escapeHtml(value) {
    return String(value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function languageIcon() {
    return `<svg class="calepin-language-picker-icon" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"></circle><path d="M3 12h18"></path><path d="M12 3a13.5 13.5 0 0 1 0 18"></path><path d="M12 3a13.5 13.5 0 0 0 0 18"></path></svg>`;
  }

  function closeAll(except) {
    document.querySelectorAll(".calepin-language-picker.is-open").forEach((picker) => {
      if (picker === except) return;
      picker.classList.remove("is-open");
      const button = picker.querySelector("button");
      if (button) button.setAttribute("aria-expanded", "false");
    });
  }

  function enhance(select) {
    if (select.dataset.calepinLanguageBound === "true") return;
    select.dataset.calepinLanguageBound = "true";

    const options = Array.from(select.options).filter((option) => option.value);
    if (options.length <= 1) return;

    const selected = options.find((option) => option.selected) || options[0];
    const picker = document.createElement("div");
    picker.className = "calepin-language-picker";

    const button = document.createElement("button");
    button.type = "button";
    button.className = "calepin-language-picker-button outline secondary";
    button.setAttribute("aria-haspopup", "menu");
    button.setAttribute("aria-expanded", "false");
    button.setAttribute("aria-label", select.getAttribute("aria-label") || "Language");
    button.innerHTML = `${languageIcon()}<span>${escapeHtml(selected.textContent)}</span>`;

    const menu = document.createElement("div");
    menu.className = "calepin-language-picker-menu";
    menu.setAttribute("role", "menu");

    options.forEach((option) => {
      const item = document.createElement("button");
      item.type = "button";
      item.setAttribute("role", "menuitem");
      item.dataset.href = option.value;
      item.innerHTML = `${languageIcon()}<span>${escapeHtml(option.textContent)}</span>`;
      if (option.selected) item.setAttribute("aria-current", "true");
      item.addEventListener("click", () => {
        window.location.href = option.value;
      });
      menu.appendChild(item);
    });

    button.addEventListener("click", () => {
      const open = !picker.classList.contains("is-open");
      closeAll(open ? picker : null);
      picker.classList.toggle("is-open", open);
      button.setAttribute("aria-expanded", open ? "true" : "false");
    });

    picker.append(button, menu);
    select.hidden = true;
    select.insertAdjacentElement("afterend", picker);
  }

  selects.forEach(enhance);

  document.addEventListener("click", (event) => {
    if (!event.target.closest(".calepin-language-picker")) closeAll();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") closeAll();
  });
})();

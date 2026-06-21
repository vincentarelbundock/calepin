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

  function closeAll(except) {
    document.querySelectorAll(".calepin-language-picker.is-open").forEach((picker) => {
      if (picker === except) return;
      picker.classList.remove("is-open");
      picker.open = false;
      const summary = picker.querySelector("summary");
      if (summary) summary.setAttribute("aria-expanded", "false");
    });
  }

  function enhance(select) {
    if (select.dataset.calepinLanguageBound === "true") return;
    select.dataset.calepinLanguageBound = "true";

    const options = Array.from(select.options).filter((option) => option.value);
    if (options.length <= 1) return;

    const selected = options.find((option) => option.selected) || options[0];
    const picker = document.createElement("details");
    picker.className = "calepin-language-picker dropdown";

    const summary = document.createElement("summary");
    summary.className = "calepin-language-picker-button";
    summary.setAttribute("role", "button");
    summary.setAttribute("aria-haspopup", "menu");
    summary.setAttribute("aria-expanded", "false");
    summary.setAttribute("aria-label", select.getAttribute("aria-label") || "Language");
    summary.innerHTML = `<span>${escapeHtml(selected.textContent)}</span>`;

    const menu = document.createElement("ul");
    menu.className = "calepin-language-picker-menu";
    menu.setAttribute("role", "menu");

    options.forEach((option) => {
      const li = document.createElement("li");
      const item = document.createElement("a");
      item.href = option.value;
      item.setAttribute("role", "menuitem");
      item.innerHTML = `<span>${escapeHtml(option.textContent)}</span>`;
      if (option.selected) item.setAttribute("aria-current", "true");
      li.appendChild(item);
      menu.appendChild(li);
    });

    summary.addEventListener("click", () => {
      const open = !picker.open;
      closeAll(open ? picker : null);
      picker.classList.toggle("is-open", open);
      summary.setAttribute("aria-expanded", open ? "true" : "false");
    });

    picker.addEventListener("toggle", () => {
      picker.classList.toggle("is-open", picker.open);
      summary.setAttribute("aria-expanded", picker.open ? "true" : "false");
    });

    picker.append(summary, menu);
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

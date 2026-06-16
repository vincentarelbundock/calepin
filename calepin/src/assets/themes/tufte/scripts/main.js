(() => {
  "use strict";

  const button = document.querySelector(".tufte-nav-toggle");
  const menu = document.getElementById("tufte-menu");
  if (!button || !menu) return;

  function setOpen(open) {
    menu.classList.toggle("is-open", open);
    button.setAttribute("aria-expanded", open ? "true" : "false");
  }

  button.addEventListener("click", () => {
    setOpen(!menu.classList.contains("is-open"));
  });

  menu.addEventListener("click", (event) => {
    if (event.target.closest("a")) setOpen(false);
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") setOpen(false);
  });
})();

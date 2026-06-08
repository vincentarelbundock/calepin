(function () {
  const article = document.querySelector(".calepin-tufte");
  if (!article) {
    return;
  }

  const endnotes = article.querySelector('section[role="doc-endnotes"]');
  if (!endnotes) {
    return;
  }

  const notes = new Map(
    Array.from(endnotes.querySelectorAll("li[id]")).map((note) => [note.id, note])
  );

  article.querySelectorAll('a[role="doc-noteref"][href^="#"]').forEach((ref) => {
    const targetId = decodeURIComponent(ref.getAttribute("href").slice(1));
    const source = notes.get(targetId);
    if (!source) {
      return;
    }

    const note = document.createElement("span");
    note.className = "sidenote tufte-generated-sidenote";
    note.dataset.noteRef = ref.textContent.trim();
    note.innerHTML = source.innerHTML;

    const backlink = note.querySelector('[role="doc-backlink"]');
    if (backlink) {
      backlink.remove();
    }

    ref.insertAdjacentElement("afterend", note);
  });

  endnotes.hidden = true;
})();

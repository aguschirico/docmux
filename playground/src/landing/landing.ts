// Copy buttons: `data-copy` holds the text, `data-copy-label` shows feedback.
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-copy]")) {
  const label = button.querySelector<HTMLElement>("[data-copy-label]");
  button.addEventListener("click", async () => {
    const text = button.dataset.copy;
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      if (label) label.textContent = "copied";
    } catch {
      if (label) label.textContent = "press ctrl+c";
    }
    window.setTimeout(() => {
      if (label) label.textContent = "copy";
    }, 1600);
  });
}

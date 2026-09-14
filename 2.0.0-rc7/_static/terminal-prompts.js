(() => {
  const markContinuationPrompts = () => {
    document.querySelectorAll(".highlight-console .gp").forEach((prompt) => {
      if (prompt.textContent === "> ") {
        prompt.classList.add("terminal-prompt--continuation");
      }
    });
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", markContinuationPrompts);
  } else {
    markContinuationPrompts();
  }
})();

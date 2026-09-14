document.addEventListener("DOMContentLoaded", () => {
  document.querySelectorAll('a[href^="https://docs.rs/"]').forEach((link) => {
    link.target = "_blank";
    link.rel = "noopener";
  });
});

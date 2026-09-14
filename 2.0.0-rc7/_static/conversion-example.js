(() => {
  const initialize = () => {
    document.querySelectorAll(".conversion-example").forEach((example) => {
      const select = example.querySelector(".conversion-example__format-select");
      const panels = Array.from(
        example.querySelectorAll(":scope > .conversion-example-panel"),
      );
      const outputs = Array.from(
        example.querySelectorAll(":scope > .conversion-example-output"),
      );
      const actions = example.querySelector(".conversion-example__actions");
      const download = example.querySelector(".conversion-example__download");
      const downloadLabel = download?.querySelector("span");
      const image = example.querySelector(".conversion-example__image");
      const htmlFrame = example.querySelector(".conversion-example__html-frame");

      if (!select || panels.length === 0) {
        return;
      }

      const showSelectedFormat = () => {
        const option = select.selectedOptions[0];
        const formatLabel = option.textContent.trim();
        const file = option.dataset.file;
        const outputKind = option.dataset.preview;
        const selectedClass = `conversion-example-panel-${select.value}`;

        panels.forEach((panel) => {
          const selected = panel.classList.contains(selectedClass);
          panel.hidden = !selected;
          panel.classList.toggle("is-active", selected);
        });

        outputs.forEach((output) => {
          const selected = output.classList.contains(
            `conversion-example-output-${outputKind}`,
          );
          output.hidden = !selected;
          output.classList.toggle("is-active", selected);
        });

        if (outputKind === "image" && image && file) {
          image.setAttribute("src", file);
          image.setAttribute(
            "alt",
            `Horizontal stacked bar chart of barley yield by variety and site rendered as ${formatLabel}`,
          );
        } else if (
          outputKind === "html" &&
          htmlFrame &&
          !htmlFrame.getAttribute("src")
        ) {
          htmlFrame.setAttribute("src", htmlFrame.dataset.src);
        }

        const hasDownload = Boolean(file);
        if (actions) {
          actions.hidden = !hasDownload;
        }
        if (hasDownload && download && downloadLabel) {
          download.setAttribute("href", file);
          download.setAttribute("download", file.split("/").pop());
          downloadLabel.textContent = `Download ${formatLabel}`;
        }
      };

      select.addEventListener("change", showSelectedFormat);
      showSelectedFormat();
      example.classList.add("conversion-example--enhanced");
    });
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initialize);
  } else {
    initialize();
  }
})();

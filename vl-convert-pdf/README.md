## vl-convert-pdf
This crate builds on top of the excellent [svg2pdf](https://github.com/typst/svg2pdf) project (created by the [typst](https://typst.app/) team) to convert SVG images to PDF files with embedded text. svg2pdf supports converting text into geometric paths using the [usvg](https://github.com/RazrFalcon/resvg) library, but it doesn't yet support embedding text (which is required for text selection, text copying, screen readers, etc.).

This project uses svg2pdf to handle everything in the SVG image except text, and then adds an embedded text layer on top using the [pdf-writer](https://github.com/typst/pdf-writer) library (also created by the typst team). The text embedding logic handles TrueType fonts and is heavily inspired by the implementation in the [typst](https://github.com/typst/typst) typesetting project.

In the future, it would be great if the text embedding logic in typst could be extracted and used by svg2pdf (making this crate unnecessary), but in the meantime this crate will maintain an independent implementation of text embedding.

Many thanks to the typst team for their support in https://github.com/typst/svg2pdf/issues/21.

## Example
See examples/pdf_conversion.rs for example usage
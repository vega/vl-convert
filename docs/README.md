# Documentation development

The documentation has shared topic sources in `docs/source/_topics/`. The
generator creates separate Python, CLI, Rust, and server pages from those
sources. Edit the shared source, not a generated interface page.

## Example contract

Keep end-user example inputs in `docs/source/_examples/`. A guide that explains
or displays a conversion result must show the exact input on the same page.
Each interface block must consume that input, although configuration can be in
the location required by the interface.

Use `literalinclude` to show canonical inputs. Do not copy a specification into
each interface block. The `vl-chart` directive must render the same input and
options that the guide describes.

Put each displayed named file in a dropdown whose title is the filename readers
should use. Open the dropdown by default when the file itself is what the page
or section teaches. Leave it collapsed when the file only supports the
surrounding workflow. Keep the code block or `literalinclude` inside the
dropdown so the expanded code has a copy button.

The server needs a complete JSON request body, not only the nested
specification. Define server examples in
`docs/source/_examples/server-requests.yaml`. The request generator combines
the canonical input with request options and writes formatted bodies under
`docs/source/_generated/requests/`. These generated files are ignored by Git.

Conceptual examples and API-reference excerpts do not always need a complete
chart. Introduce these as fragments or state which setup they omit. Do not
present an undefined name or file as a runnable example.

Keep deterministic examples offline. Use network access only in a guide that
explicitly teaches remote data, Google Fonts, or remote plugins.

## Build and check the documentation

Run the generators, a strict Sphinx build, and the link check:

```bash
pixi run docs-generate
pixi run docs-build-strict
pixi run docs-linkcheck
```

Run the documentation tooling style check:

```bash
pixi run ruff check docs/tools docs/source/_ext
```

Use `pixi run docs-serve` for a live preview. Its pre-build commands regenerate
the interface pages and server request bodies after a source change.

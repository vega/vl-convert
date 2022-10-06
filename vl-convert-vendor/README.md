# vl-convert-vendor
Helper crate that downloads multiple versions of Vega-Lite, and their dependencies, using [Deno vendor](https://deno.land/manual@v1.26.0/tools/vendor). It also generates the `vl-convert-rs/src/module_loader/import_map.rs` file which inlines the source code of all the downloaded dependencies using the `include_str!` macro.

This crate relies on the Deno command-line program being available on the system `PATH`.
 
This crate only needs to be run when a new Vega-Lite version is to be added.

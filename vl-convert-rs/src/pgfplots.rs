//! Native, data-coordinate PGFPlots output for a strict Vega-Lite subset.
//!
//! The shared compiler runs in the existing V8 worker. It never renders SVG,
//! executes input expressions, downloads data, or launches a TeX process.

use crate::anyhow::{bail, Result};
use crate::{ValueOrString, VlConverter};

const COMPILER: &str = include_str!("js/native_pgfplots.js");
const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;

/// Options specific to native PGFPlots output.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct PgfplotsOpts {
    /// Emit a complete document rather than a `tikzpicture` fragment.
    pub standalone: bool,
    /// Interpret labels as trusted TeX instead of escaping them.
    ///
    /// Never enable for untrusted input. Generated TeX can execute commands when
    /// subsequently compiled; disabling shell escape is not a complete sandbox.
    pub tex_labels: bool,
}

/// Native PGFPlots source containing editable axes, data coordinates, and text.
#[derive(Debug)]
pub struct PgfplotsOutput {
    /// TeX source. No auxiliary image files or outlined glyphs are generated.
    pub tex: String,
}

impl VlConverter {
    /// Compile a static Cartesian Vega-Lite subset to native PGFPlots.
    ///
    /// Supports inline data, ordinary Cartesian marks, explicit uncertainties,
    /// prepared box plots, heatmaps, layers, and simple multi-view composition.
    /// Unsupported properties return errors, rather than an approximate picture.
    ///
    /// This is an independent semantic backend, not the bundled Vega-Lite-to-Vega
    /// compiler. It does not inherit browser themes, fonts, locales, plugins, or
    /// resource-loading settings. TeX controls typography. See the PGFPlots guide
    /// for the precise supported subset and statistical definitions.
    pub async fn vegalite_to_pgfplots(
        &self,
        spec: impl Into<ValueOrString>,
        opts: PgfplotsOpts,
    ) -> Result<PgfplotsOutput> {
        let json = match spec.into() {
            ValueOrString::JsonString(json) => json,
            ValueOrString::Value(value) => serde_json::to_string(&value)?,
        };
        if json.len() > MAX_INPUT_BYTES {
            bail!("PGFPlots: input exceeds the 8 MiB limit");
        }
        // Serialize a string containing JSON, not JavaScript supplied by the
        // caller. JSON.parse creates data; labels/expressions never become code.
        let source = format!(
            "{COMPILER}\nvlcNativePgfplots.compile(JSON.parse({}), {});",
            serde_json::to_string(&json)?,
            serde_json::to_string(&opts)?,
        );
        self.run_on_worker(move |inner| {
            Box::pin(async move {
                let tex = inner.execute_script_to_string(&source).await?;
                Ok(PgfplotsOutput { tex })
            })
        })
        .await
    }
}

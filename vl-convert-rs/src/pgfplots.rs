//! Native, data-coordinate PGFPlots output for a strict Vega-Lite subset.
//!
//! The existing Vega worker evaluates data and Vega expressions under the same
//! resource policy as image conversion. TeX emission retains axes and original
//! data coordinates; no SVG tracing or TeX subprocess is used.

use crate::anyhow::{bail, Result};
use crate::{LogEntry, ValueOrString, VlConverter, VlVersion};

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
    /// Diagnostics from official compilation, data evaluation, and normalization.
    pub logs: Vec<LogEntry>,
}

impl VlConverter {
    /// Compile a static Cartesian Vega-Lite subset to native PGFPlots.
    ///
    /// Supports inline data, ordinary Cartesian marks, explicit uncertainties,
    /// prepared box plots, heatmaps, layers, and simple multi-view composition.
    /// Unsupported properties return errors, rather than an approximate picture.
    ///
    /// The official compiler/runtime evaluates supported static dataflows. Native
    /// TeX controls typography. Unsupported graphical semantics return an error;
    /// the converter never starts TeX. Resource loads use the configured policy.
    pub async fn vegalite_to_pgfplots(
        &self,
        spec: impl Into<ValueOrString>,
        opts: PgfplotsOpts,
    ) -> Result<PgfplotsOutput> {
        self.vegalite_to_pgfplots_with_version(spec, VlVersion::default(), opts)
            .await
    }

    /// Compile using an explicitly selected bundled Vega-Lite version.
    ///
    /// Resource permissions and execution limits are inherited from this converter.
    pub async fn vegalite_to_pgfplots_with_version(
        &self,
        spec: impl Into<ValueOrString>,
        version: VlVersion,
        opts: PgfplotsOpts,
    ) -> Result<PgfplotsOutput> {
        let json = match spec.into() {
            ValueOrString::JsonString(json) => json,
            ValueOrString::Value(value) => serde_json::to_string(&value)?,
        };
        if json.len() > MAX_INPUT_BYTES {
            bail!("PGFPlots: input exceeds the 8 MiB limit");
        }
        self.run_on_worker(move |inner| {
            Box::pin(async move {
                inner
                    .native_pgfplots(ValueOrString::JsonString(json), version, opts)
                    .await
            })
        })
        .await
    }
}

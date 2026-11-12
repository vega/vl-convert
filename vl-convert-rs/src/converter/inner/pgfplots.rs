//! Official dataflow evaluation followed by native, data-coordinate emission.
use super::super::transfer::JsonArgGuard;
use super::InnerVlConverter;
use crate::anyhow::{anyhow, bail, Result};
use crate::{PgfplotsOpts, PgfplotsOutput, ValueOrString, VlVersion};

const COMPILER: &str = include_str!("../../js/native_pgfplots.js");
const NORMALIZER: &str = include_str!("../../js/native_pgfplots_normalize.js");

impl InnerVlConverter {
    pub(crate) async fn native_pgfplots(
        &mut self,
        spec: ValueOrString,
        version: VlVersion,
        options: PgfplotsOpts,
    ) -> Result<PgfplotsOutput> {
        self.init_vega().await?;
        self.init_vl_version(&version).await?;
        let input = JsonArgGuard::from_spec(&self.transfer_state, spec)?;
        let code = format!(
            r#"
{COMPILER}
{NORMALIZER}
_clearLogMessages();
globalThis.__vlcPgfResult = null;
(async () => {{
    const errors = [];
    try {{
        const input = JSON.parse(op_get_json_arg({input_id}));
        const prepared = await vlcPgfplotsNormalize.prepare(input, {{
            vega, vl: {version:?}, loader: buildLoader(errors), logger: logCollector
        }});
        if (errors.length) throw new Error(errors.join('\n'));
        const figure = vlcPgfplotsNormalize.decorate(vlcNativePgfplots.lower(prepared.spec), vega);
        globalThis.__vlcPgfResult = {{tex: vlcNativePgfplots.emit(figure, {options})}};
    }} catch (error) {{
        globalThis.__vlcPgfResult = {{error: String(error.message || error)}};
    }}
}})();
null;
"#,
            input_id = input.id(),
            options = serde_json::to_string(&options)?,
        );
        // execute_script_to_json drives the event loop. Return null, not the
        // unresolved Promise; collect and clear the request-local result below.
        self.execute_script_to_json(&code).await?;
        let result = self
            .execute_script_to_json(
                "(() => { const r = globalThis.__vlcPgfResult; delete globalThis.__vlcPgfResult; return r; })()",
            )
            .await?;
        self.emit_js_log_messages().await;
        if let Some(error) = result.get("error").and_then(|e| e.as_str()) {
            bail!("{error}");
        }
        let tex = result
            .get("tex")
            .and_then(|s| s.as_str())
            .ok_or_else(|| anyhow!("PGFPlots: normalization did not return TeX"))?
            .to_owned();
        Ok(PgfplotsOutput {
            tex,
            logs: std::mem::take(&mut self.last_log_entries),
        })
    }
}

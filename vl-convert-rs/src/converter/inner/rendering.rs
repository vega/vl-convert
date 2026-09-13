use super::InnerVlConverter;
use crate::image_loading::ImageAccessPolicy;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;

use super::super::transfer::JsonArgGuard;
use super::super::types::*;
use super::super::value_or_string::{apply_spec_overrides, ValueOrString};

impl InnerVlConverter {
    pub async fn vega_to_png(
        &mut self,
        vg_spec: &serde_json::Value,
        vg_opts: VgOpts,
        scale: f32,
        ppi: f32,
    ) -> Result<PngOutput, AnyError> {
        self.init_vega().await?;

        let vg_spec = apply_spec_overrides(
            ValueOrString::Value(vg_spec.clone()),
            &vg_opts.background,
            vg_opts.width,
            vg_opts.height,
        )?
        .to_value()?;

        let format_locale = match vg_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vg_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let config_value = vg_opts.config.unwrap_or(serde_json::Value::Null);

        let spec_arg = JsonArgGuard::from_value(&self.transfer_state, vg_spec)?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config_value)?;

        let code = format!(
            r#"
var canvasPngData;
var errors = [];
_clearLogMessages();
vegaToCanvas(
    JSON.parse(op_get_json_arg({arg_id})),
    JSON.parse(op_get_json_arg({format_locale_id})),
    JSON.parse(op_get_json_arg({time_format_locale_id})),
    {scale},
    JSON.parse(op_get_json_arg({config_id})),
    errors,
).then((canvas) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    canvasPngData = canvas._toPngWithPpi({ppi});
}})
"#,
            arg_id = spec_arg.id(),
            format_locale_id = format_locale_arg.id(),
            time_format_locale_id = time_format_locale_arg.id(),
            config_id = config_arg.id(),
        );
        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        self.emit_js_log_messages().await;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }
        let data = self.execute_script_to_bytes("canvasPngData").await?;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(PngOutput {
            data,
            logs,
            google_fonts: Default::default(),
        })
    }

    pub async fn vegalite_to_png(
        &mut self,
        vl_spec: &serde_json::Value,
        vl_opts: VlOpts,
        scale: f32,
        ppi: f32,
    ) -> Result<PngOutput, AnyError> {
        self.init_vega().await?;
        self.init_vl_version(&vl_opts.vl_version).await?;

        let config = vl_opts.config.clone().unwrap_or(serde_json::Value::Null);

        let format_locale = match vl_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vl_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let spec_arg = JsonArgGuard::from_value(&self.transfer_state, vl_spec.clone())?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config)?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let code = format!(
            r#"
var canvasPngData;
var errors = [];
_clearLogMessages();
vegaLiteToCanvas_{ver_name:?}(
    JSON.parse(op_get_json_arg({spec_arg_id})),
    JSON.parse(op_get_json_arg({config_arg_id})),
    {theme_arg},
    JSON.parse(op_get_json_arg({format_locale_id})),
    JSON.parse(op_get_json_arg({time_format_locale_id})),
    {scale},
    errors,
).then((canvas) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    canvasPngData = canvas._toPngWithPpi({ppi});
}})
"#,
            ver_name = vl_opts.vl_version,
            spec_arg_id = spec_arg.id(),
            config_arg_id = config_arg.id(),
            format_locale_id = format_locale_arg.id(),
            time_format_locale_id = time_format_locale_arg.id(),
        );
        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let access_errors = self
            .execute_script_to_string(
                "Array.isArray(errors) && errors.length > 0 ? errors.join('\\n') : ''",
            )
            .await?;
        self.emit_js_log_messages().await;
        if !access_errors.is_empty() {
            bail!("{access_errors}");
        }
        let data = self.execute_script_to_bytes("canvasPngData").await?;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(PngOutput {
            data,
            logs,
            google_fonts: Default::default(),
        })
    }

    pub(crate) fn svg_to_jpeg_with_worker_options(
        &mut self,
        svg: &str,
        scale: f32,
        quality: Option<u8>,
        policy: &ImageAccessPolicy,
    ) -> Result<Vec<u8>, AnyError> {
        super::super::rendering::svg_to_jpeg_with_options(
            svg,
            scale,
            quality,
            policy,
            &mut self.usvg_options,
        )
    }

    pub(crate) fn svg_to_pdf_with_worker_options(
        &mut self,
        svg: &str,
        policy: &ImageAccessPolicy,
    ) -> Result<Vec<u8>, AnyError> {
        super::super::rendering::svg_to_pdf_with_options(svg, policy, &mut self.usvg_options)
    }

    pub async fn vega_to_jpeg(
        &mut self,
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        scale: f32,
        quality: Option<u8>,
        policy: ImageAccessPolicy,
    ) -> Result<JpegOutput, AnyError> {
        let svg_output = self.vega_to_svg(vg_spec, vg_opts).await?;
        let data =
            self.svg_to_jpeg_with_worker_options(&svg_output.svg, scale, quality, &policy)?;
        Ok(JpegOutput {
            data,
            logs: svg_output.logs,
            google_fonts: svg_output.google_fonts,
        })
    }

    pub async fn vegalite_to_jpeg(
        &mut self,
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        scale: f32,
        quality: Option<u8>,
        policy: ImageAccessPolicy,
    ) -> Result<JpegOutput, AnyError> {
        let svg_output = self.vegalite_to_svg(vl_spec, vl_opts).await?;
        let data =
            self.svg_to_jpeg_with_worker_options(&svg_output.svg, scale, quality, &policy)?;
        Ok(JpegOutput {
            data,
            logs: svg_output.logs,
            google_fonts: svg_output.google_fonts,
        })
    }

    pub async fn vega_to_pdf(
        &mut self,
        vg_spec: ValueOrString,
        vg_opts: VgOpts,
        policy: ImageAccessPolicy,
    ) -> Result<PdfOutput, AnyError> {
        let svg_output = self.vega_to_svg(vg_spec, vg_opts).await?;
        let data = self.svg_to_pdf_with_worker_options(&svg_output.svg, &policy)?;
        Ok(PdfOutput {
            data,
            logs: svg_output.logs,
            google_fonts: svg_output.google_fonts,
        })
    }

    pub async fn vegalite_to_pdf(
        &mut self,
        vl_spec: ValueOrString,
        vl_opts: VlOpts,
        policy: ImageAccessPolicy,
    ) -> Result<PdfOutput, AnyError> {
        let svg_output = self.vegalite_to_svg(vl_spec, vl_opts).await?;
        let data = self.svg_to_pdf_with_worker_options(&svg_output.svg, &policy)?;
        Ok(PdfOutput {
            data,
            logs: svg_output.logs,
            google_fonts: svg_output.google_fonts,
        })
    }
}

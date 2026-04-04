use super::InnerVlConverter;
use deno_core::anyhow::{anyhow, bail};
use deno_core::error::AnyError;

use super::super::transfer::{JsonArgGuard, MsgpackResultGuard};
use super::super::types::*;
use super::super::value_or_string::{apply_spec_overrides, ValueOrString};

impl InnerVlConverter {
    pub async fn vegalite_to_vega(
        &mut self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<VegaOutput, AnyError> {
        self.init_vega().await?;
        self.init_vl_version(&vl_opts.vl_version).await?;
        let config = vl_opts.config.clone().unwrap_or(serde_json::Value::Null);

        let vl_spec = apply_spec_overrides(
            vl_spec.into(),
            &vl_opts.background,
            vl_opts.width,
            vl_opts.height,
        )?;
        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vl_spec)?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config)?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let code = format!(
            r#"
_clearLogMessages();
compileVegaLite_{ver_name:?}(
    JSON.parse(op_get_json_arg({spec_arg_id})),
    JSON.parse(op_get_json_arg({config_arg_id})),
    {theme_arg}
)
"#,
            ver_name = vl_opts.vl_version,
            spec_arg_id = spec_arg.id(),
            config_arg_id = config_arg.id(),
            theme_arg = theme_arg,
        );

        let spec = self.execute_script_to_json(&code).await?;
        self.emit_js_log_messages().await;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(VegaOutput { spec, logs })
    }

    pub async fn vegalite_to_svg(
        &mut self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<SvgOutput, AnyError> {
        self.init_vega().await?;
        self.init_vl_version(&vl_opts.vl_version).await?;

        let vl_spec = apply_spec_overrides(
            vl_spec.into(),
            &vl_opts.background,
            vl_opts.width,
            vl_opts.height,
        )?;
        let config = vl_opts.config.clone().unwrap_or(serde_json::Value::Null);

        let format_locale = match vl_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vl_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vl_spec)?;
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
var svg;
var errors = [];
_clearLogMessages();
vegaLiteToSvg_{ver_name:?}(
    JSON.parse(op_get_json_arg({spec_arg_id})),
    JSON.parse(op_get_json_arg({config_arg_id})),
    {theme_arg},
    JSON.parse(op_get_json_arg({format_locale_id})),
    JSON.parse(op_get_json_arg({time_format_locale_id})),
    errors,
).then((result) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    svg = result;
}});
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
        let svg = self.execute_script_to_string("svg").await?;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(SvgOutput { svg, logs })
    }

    pub async fn vegalite_to_scenegraph_msgpack(
        &mut self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<ScenegraphMsgpackOutput, AnyError> {
        self.init_vega().await?;
        self.init_vl_version(&vl_opts.vl_version).await?;
        let vl_spec = apply_spec_overrides(
            vl_spec.into(),
            &vl_opts.background,
            vl_opts.width,
            vl_opts.height,
        )?;

        let config = vl_opts.config.clone().unwrap_or(serde_json::Value::Null);
        let format_locale = match vl_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vl_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vl_spec)?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config)?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;
        let result = MsgpackResultGuard::new(&self.transfer_state)?;

        let theme_arg = match &vl_opts.theme {
            None => "null".to_string(),
            Some(s) => format!("'{}'", s),
        };

        let code = format!(
            r#"
var errors = [];
_clearLogMessages();
vegaLiteToScenegraph_{ver_name:?}(
    JSON.parse(op_get_json_arg({spec_arg_id})),
    JSON.parse(op_get_json_arg({config_arg_id})),
    {theme_arg},
    JSON.parse(op_get_json_arg({format_locale_id})),
    JSON.parse(op_get_json_arg({time_format_locale_id})),
    errors,
).then((result) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    op_set_msgpack_result({result_id}, msgpack.encode(result));
}})
"#,
            ver_name = vl_opts.vl_version,
            spec_arg_id = spec_arg.id(),
            config_arg_id = config_arg.id(),
            format_locale_id = format_locale_arg.id(),
            time_format_locale_id = time_format_locale_arg.id(),
            result_id = result.id(),
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
        let data = result.take_result()?;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(ScenegraphMsgpackOutput { data, logs })
    }

    pub async fn vegalite_to_scenegraph(
        &mut self,
        vl_spec: impl Into<ValueOrString>,
        vl_opts: VlOpts,
    ) -> Result<ScenegraphOutput, AnyError> {
        let sg_output = self
            .vegalite_to_scenegraph_msgpack(vl_spec, vl_opts)
            .await?;
        let scenegraph: serde_json::Value = rmp_serde::from_slice(&sg_output.data)
            .map_err(|err| anyhow!("Failed to decode MessagePack scenegraph: {err}"))?;
        Ok(ScenegraphOutput {
            scenegraph,
            logs: sg_output.logs,
        })
    }

    pub async fn vega_to_svg(
        &mut self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
    ) -> Result<SvgOutput, AnyError> {
        self.init_vega().await?;

        let vg_spec = apply_spec_overrides(
            vg_spec.into(),
            &vg_opts.background,
            vg_opts.width,
            vg_opts.height,
        )?;

        let format_locale = match vg_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vg_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let config_value = vg_opts.config.unwrap_or(serde_json::Value::Null);

        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vg_spec)?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config_value)?;

        let code = format!(
            r#"
var svg;
var errors = [];
_clearLogMessages();
vegaToSvg(
    JSON.parse(op_get_json_arg({arg_id})),
    JSON.parse(op_get_json_arg({format_locale_id})),
    JSON.parse(op_get_json_arg({time_format_locale_id})),
    JSON.parse(op_get_json_arg({config_id})),
    errors,
).then((result) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    svg = result;
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
        let svg = self.execute_script_to_string("svg").await?;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(SvgOutput { svg, logs })
    }

    pub async fn vega_to_scenegraph_msgpack(
        &mut self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
    ) -> Result<ScenegraphMsgpackOutput, AnyError> {
        self.init_vega().await?;
        let vg_spec = apply_spec_overrides(
            vg_spec.into(),
            &vg_opts.background,
            vg_opts.width,
            vg_opts.height,
        )?;
        let format_locale = match vg_opts.format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let time_format_locale = match vg_opts.time_format_locale {
            None => serde_json::Value::Null,
            Some(fl) => fl.as_object()?,
        };

        let config_value = vg_opts.config.unwrap_or(serde_json::Value::Null);
        let spec_arg = JsonArgGuard::from_spec(&self.transfer_state, vg_spec)?;
        let format_locale_arg = JsonArgGuard::from_value(&self.transfer_state, format_locale)?;
        let time_format_locale_arg =
            JsonArgGuard::from_value(&self.transfer_state, time_format_locale)?;
        let config_arg = JsonArgGuard::from_value(&self.transfer_state, config_value)?;
        let result = MsgpackResultGuard::new(&self.transfer_state)?;

        let code = format!(
            r#"
var errors = [];
_clearLogMessages();
vegaToScenegraph(
    JSON.parse(op_get_json_arg({arg_id})),
    JSON.parse(op_get_json_arg({format_locale_id})),
    JSON.parse(op_get_json_arg({time_format_locale_id})),
    JSON.parse(op_get_json_arg({config_id})),
    errors,
).then((result) => {{
    if (errors != null && errors.length > 0) {{
        throw new Error(`${{errors}}`);
    }}
    op_set_msgpack_result({result_id}, msgpack.encode(result));
}})
"#,
            arg_id = spec_arg.id(),
            format_locale_id = format_locale_arg.id(),
            time_format_locale_id = time_format_locale_arg.id(),
            config_id = config_arg.id(),
            result_id = result.id(),
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
        let data = result.take_result()?;
        let logs = std::mem::take(&mut self.last_log_entries);
        Ok(ScenegraphMsgpackOutput { data, logs })
    }

    pub async fn vega_to_scenegraph(
        &mut self,
        vg_spec: impl Into<ValueOrString>,
        vg_opts: VgOpts,
    ) -> Result<ScenegraphOutput, AnyError> {
        let sg_output = self.vega_to_scenegraph_msgpack(vg_spec, vg_opts).await?;
        let scenegraph: serde_json::Value = rmp_serde::from_slice(&sg_output.data)
            .map_err(|err| anyhow!("Failed to decode MessagePack scenegraph: {err}"))?;
        Ok(ScenegraphOutput {
            scenegraph,
            logs: sg_output.logs,
        })
    }

    pub async fn get_local_tz(&mut self) -> Result<Option<String>, AnyError> {
        let code = "var localTz = Intl.DateTimeFormat().resolvedOptions().timeZone ?? 'undefined';"
            .to_string();
        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let value = self.execute_script_to_string("localTz").await?;
        if value == "undefined" {
            Ok(None)
        } else {
            Ok(Some(value))
        }
    }

    pub async fn get_themes(&mut self) -> Result<serde_json::Value, AnyError> {
        self.init_vega().await?;

        let code = r#"
var themes = Object.assign({}, vegaThemes);
delete themes.version
delete themes.default
"#
        .to_string();

        self.worker.js_runtime.execute_script("ext:<anon>", code)?;
        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        let value = self.execute_script_to_json("themes").await?;
        Ok(value)
    }
}

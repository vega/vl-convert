// mod crate::text;
use vl_convert_common::ops::vl_convert_runtime;

pub static TS_VERSION: &str = "5.8.3";

fn main() {
    {
        let o = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let cli_snapshot_path = o.join("CLI_SNAPSHOT.bin");
        create_cli_snapshot(cli_snapshot_path);
    }
}

fn create_cli_snapshot(snapshot_path: std::path::PathBuf) {
    use deno_runtime::ops::bootstrap::SnapshotOptions;

    let snapshot_options = SnapshotOptions {
        ts_version: TS_VERSION.to_string(),
        v8_version: deno_runtime::deno_core::v8::VERSION_STRING,
        target: std::env::var("TARGET").unwrap(),
    };

    deno_runtime::snapshot::create_runtime_snapshot(
        snapshot_path,
        snapshot_options,
        vec![
            vl_convert_runtime::init(),
        ],
    );
}
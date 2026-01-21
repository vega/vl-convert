// Test using JsRuntime directly from deno_core (without deno_runtime snapshot)
use deno_core::extension;
use deno_core::op2;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use deno_error::JsErrorBox;

// Define our test ops with inline ESM source
extension!(
    test_extension,
    ops = [op_test_echo, op_test_double],
    esm_entry_point = "ext:test_extension/bootstrap.js",
    esm = [
        "ext:test_extension/bootstrap.js" = {
            source = r#"
                import { op_test_echo, op_test_double } from "ext:core/ops";

                // Expose ops on globalThis so user code can access them
                globalThis.testOps = {
                    echo: op_test_echo,
                    double: op_test_double,
                };

                console.log("[bootstrap.js] Test ops exposed on globalThis.testOps");
            "#
        }
    ],
);

#[op2]
#[string]
fn op_test_echo(#[string] msg: String) -> Result<String, JsErrorBox> {
    Ok(format!("Echo: {}", msg))
}

#[op2(fast)]
fn op_test_double(val: f64) -> f64 {
    val * 2.0
}

fn main() {
    println!("Creating JsRuntime with our extension...");

    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![test_extension::init()],
        ..Default::default()
    });

    println!("JsRuntime created successfully");

    // Test if we can access Deno.core.ops
    let result = runtime
        .execute_script(
            "test.js",
            r#"
            const hasDeno = typeof Deno !== 'undefined';
            console.log("Has Deno:", hasDeno);

            const hasCore = hasDeno && typeof Deno.core !== 'undefined';
            console.log("Has Deno.core:", hasCore);

            const hasOps = hasCore && typeof Deno.core.ops !== 'undefined';
            console.log("Has Deno.core.ops:", hasOps);

            if (hasOps) {
                console.log("Ops keys:", Object.keys(Deno.core.ops).sort().join(", "));
                console.log("Has op_test_echo:", 'op_test_echo' in Deno.core.ops);
                console.log("Has op_test_double:", 'op_test_double' in Deno.core.ops);
            }

            JSON.stringify({ hasDeno, hasCore, hasOps })
            "#
            .to_string(),
        )
        .unwrap();

    // Get the result
    deno_core::scope!(scope, &mut runtime);
    let local = deno_core::v8::Local::new(scope, result);
    let result_str = local.to_rust_string_lossy(scope);
    println!("\nResult: {}", result_str);
}

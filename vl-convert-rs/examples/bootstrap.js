// Bootstrap ESM file for test_extension
// This file can import from ext:core/ops because it's an ext: module
import { op_test_echo, op_test_double } from "ext:core/ops";

// Expose ops on globalThis so user code can access them
globalThis.testOps = {
    echo: op_test_echo,
    double: op_test_double,
};

console.log("[bootstrap.js] Test ops exposed on globalThis.testOps");

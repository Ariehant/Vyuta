//! Vyuta debug bridge — Neon (Node N-API) addon wrapping `probe-rs`.
//!
//! Loaded in-process by the VS Code extension host, this addon provides
//! synchronous probe/target queries that back the firmware/debug UI:
//!   * `hello()`        -> build identifier string
//!   * `listProbes()`   -> JSON array of attached debug probes
//!   * `listChipFamilies()` -> JSON array of known chip-family names
//!
//! The actual debug *session* (breakpoints, stepping, registers, RTT) is run
//! by `probe-rs dap-server`, launched from the extension's DAP factory — a far
//! more robust path than hand-rolling a GDB stub through Neon.

use neon::prelude::*;
use probe_rs::probe::list::Lister;

/// Sanity-check export: returns a build identifier string.
fn hello(mut cx: FunctionContext) -> JsResult<JsString> {
    Ok(cx.string(format!(
        "vyuta probe-rs-extension v{} (probe-rs backend)",
        env!("CARGO_PKG_VERSION")
    )))
}

/// Enumerate attached debug probes, returned as a JSON array string.
///
/// On a host with no probe attached this is an empty array `[]` — which is the
/// correct answer, not an error.
fn list_probes(mut cx: FunctionContext) -> JsResult<JsString> {
    let lister = Lister::new();
    let probes = lister.list_all();

    let json: Vec<serde_json::Value> = probes
        .iter()
        .map(|p| {
            serde_json::json!({
                "identifier": p.identifier,
                "vendorId": p.vendor_id,
                "productId": p.product_id,
                "serialNumber": p.serial_number,
            })
        })
        .collect();

    Ok(cx.string(serde_json::Value::Array(json).to_string()))
}

/// List the names of chip families known to probe-rs (for build/flash target
/// pickers), returned as a JSON array string.
fn list_chip_families(mut cx: FunctionContext) -> JsResult<JsString> {
    let registry = probe_rs::config::Registry::from_builtin_families();
    let names: Vec<String> = registry.families().iter().map(|f| f.name.clone()).collect();
    Ok(cx.string(serde_json::Value::from(names).to_string()))
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    cx.export_function("hello", hello)?;
    cx.export_function("listProbes", list_probes)?;
    cx.export_function("listChipFamilies", list_chip_families)?;
    Ok(())
}

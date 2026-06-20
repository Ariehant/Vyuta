//! Vyuta debug bridge — Phase 0 scaffold.
//!
//! This crate is compiled as a Neon (Node N-API) addon and loaded in-process
//! by the VS Code extension host. Phase 2 will wrap `probe-rs` here to
//! enumerate debug probes and drive a GDB/DAP-compatible debug session.
//!
//! For Phase 0 it exposes just enough surface to prove the Neon build pipeline
//! and the in-process Rust ↔ TypeScript call path:
//!   * `hello()`       -> identifying string
//!   * `listProbes()`  -> JSON array of (synthetic) attached probes

use neon::prelude::*;

/// Sanity-check export: returns a build identifier string.
fn hello(mut cx: FunctionContext) -> JsResult<JsString> {
    Ok(cx.string(format!(
        "vyuta probe-rs-extension v{} (Phase 0 stub)",
        env!("CARGO_PKG_VERSION")
    )))
}

/// Enumerate attached debug probes.
///
/// Phase 0: returns a single synthetic probe so the UI/debug-adapter wiring can
/// be developed before real hardware is involved. Phase 2 replaces the body
/// with `probe_rs::probe::list::Lister::new().list_all()`.
fn list_probes(mut cx: FunctionContext) -> JsResult<JsString> {
    let probes = serde_json::json!([
        {
            "identifier": "Synthetic CMSIS-DAP (Phase 0 stub)",
            "vendorId": 0xc251,
            "productId": 0xf001,
            "serialNumber": null,
            "probeType": "synthetic",
            "synthetic": true
        }
    ]);
    Ok(cx.string(probes.to_string()))
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    cx.export_function("hello", hello)?;
    cx.export_function("listProbes", list_probes)?;
    Ok(())
}

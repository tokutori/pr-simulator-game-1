//! Browser I/O binding for the shared application owner.

use wasm_bindgen::JsValue;

mod application_runtime;

pub use application_runtime::{ApplicationRuntimeBridge, ApplicationTextMeasurer};

fn error(cause: impl core::fmt::Display) -> JsValue {
    JsValue::from_str(&cause.to_string())
}

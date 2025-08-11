//! WebAssembly bindings exposing a minimal high-level API.
//!
//! The functions in this module are only compiled when targeting `wasm32`
//! with the `webgpu` feature enabled. They provide a stable surface for the
//! TypeScript wrapper to interact with the engine without copying large device
//! buffers back to the host.

#![cfg(all(target_arch = "wasm32", feature = "webgpu"))]

use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::gpu::device::init_device;

/// Handle to the engine. Internally stores the WebGPU `Device` and `Queue`.
#[wasm_bindgen]
pub struct MycosHandle {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

/// Execution metrics returned from `tick`.
#[wasm_bindgen]
pub struct Metrics {
    rounds: u32,
    effects: u32,
}

#[wasm_bindgen]
impl Metrics {
    /// Number of wavefront rounds executed in the last tick.
    #[wasm_bindgen(getter)]
    pub fn rounds(&self) -> u32 {
        self.rounds
    }

    /// Number of effects applied in the last tick.
    #[wasm_bindgen(getter)]
    pub fn effects(&self) -> u32 {
        self.effects
    }
}

/// Initialize WebGPU and create a new [`MycosHandle`].
#[wasm_bindgen]
pub async fn init(_canvas: Option<HtmlCanvasElement>) -> Result<MycosHandle, JsValue> {
    // For now the canvas is unused as the engine only performs compute work.
    let (device, queue) = init_device().await?;
    Ok(MycosHandle { device, queue })
}

#[wasm_bindgen]
impl MycosHandle {
    /// Load chunk binaries into the engine.
    pub fn load_chunks(&mut self, _chunks: js_sys::Array) {
        // Placeholder: real implementation will parse and upload chunk data.
    }

    /// Load link graph binary describing inter-chunk connections.
    pub fn load_links(&mut self, _links: js_sys::ArrayBuffer) {
        // Placeholder for future implementation.
    }

    /// Set input words for a given chunk.
    ///
    /// `words` is a view into WebAssembly memory, avoiding an extra copy.
    pub fn set_inputs(&mut self, _chunk_id: u32, _words: js_sys::Uint32Array) {}

    /// Execute the engine for up to `max_rounds` wavefront rounds.
    pub fn tick(&mut self, _max_rounds: Option<u32>) -> Metrics {
        // Stub metrics; real values will be produced by the GPU pipeline.
        Metrics {
            rounds: 0,
            effects: 0,
        }
    }

    /// Read output words for a given chunk into `out`.
    pub fn get_outputs(&self, _chunk_id: u32, _out: js_sys::Uint32Array) {}

    /// Select the oscillation handling policy.
    pub fn set_policy(&mut self, _mode: &str) {}
}

use wasm_bindgen::JsValue;

/// Initialize WebGPU and return the device and queue.
///
/// This function is only available when compiling for `wasm32` with the
/// `webgpu` feature enabled. It selects the first available adapter and
/// requests a device/queue pair using WebGPU-compatible limits.
#[cfg(all(target_arch = "wasm32", feature = "webgpu"))]
pub async fn init_device() -> Result<(wgpu::Device, wgpu::Queue), JsValue> {
    // Instance is a lightweight handle in wgpu and doesn't need to be stored.
    let instance = wgpu::Instance::default();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .ok_or_else(|| JsValue::from_str("No suitable GPU adapters found"))?;

    // No optional features are requested for the initial web build.
    let features = wgpu::Features::empty();

    let limits = wgpu::Limits::downlevel_webgl2_defaults();

    let descriptor = wgpu::DeviceDescriptor {
        label: Some("mycos-device"),
        required_features: features,
        required_limits: limits,
    };

    adapter
        .request_device(&descriptor, None)
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

use bevy::prelude::*;
use naga::front::wgsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShaderDiagnostic {
    pub message: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreviewPerformanceMetrics {
    pub validate_ms: f64,
    pub apply_ms: f64,
    pub last_frame_ms: Option<f64>,
    pub average_frame_ms: Option<f64>,
}

impl Default for PreviewPerformanceMetrics {
    fn default() -> Self {
        Self {
            validate_ms: 0.0,
            apply_ms: 0.0,
            last_frame_ms: None,
            average_frame_ms: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PreviewShaderStatus {
    Idle,
    Valid,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShaderValidationResult {
    pub ok: bool,
    pub diagnostics: Vec<ShaderDiagnostic>,
    pub validate_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShaderApplyResult {
    pub ok: bool,
    pub diagnostics: Vec<ShaderDiagnostic>,
    pub status: PreviewShaderStatus,
    pub metrics: PreviewPerformanceMetrics,
}

#[derive(Debug, Resource, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShaderPreviewState {
    pub active_source: String,
    pub last_good_source: String,
    pub status: PreviewShaderStatus,
    pub diagnostics: Vec<ShaderDiagnostic>,
    pub metrics: PreviewPerformanceMetrics,
}

impl Default for ShaderPreviewState {
    fn default() -> Self {
        Self {
            active_source: String::new(),
            last_good_source: String::new(),
            status: PreviewShaderStatus::Idle,
            diagnostics: Vec::new(),
            metrics: PreviewPerformanceMetrics::default(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ShaderPreviewError {
    #[error("WGSL parse failed")]
    ParseFailed(Vec<ShaderDiagnostic>),
    #[error("WGSL validation failed")]
    ValidationFailed(Vec<ShaderDiagnostic>),
}

pub struct ShaderPreviewPlugin;

impl Plugin for ShaderPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShaderPreviewState>()
            .add_systems(Startup, preview_runtime_boot_log);
    }
}

fn preview_runtime_boot_log() {
    info!("sidereal-shader-preview runtime scaffold booted");
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now())
        .unwrap_or(0.0)
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(std::time::Instant::now);
    start.elapsed().as_secs_f64() * 1000.0
}

const PREVIEW_VERTEX_OUTPUT: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
"#;

fn normalize_preview_wgsl(source: &str) -> String {
    let adapted = source.replace(
        "#import bevy_sprite::mesh2d_vertex_output::VertexOutput",
        "",
    );

    if adapted.contains("struct VertexOutput") {
        adapted
    } else {
        format!("{PREVIEW_VERTEX_OUTPUT}\n{adapted}")
    }
}

pub fn validate_wgsl_source(source: &str) -> Result<ShaderValidationResult, ShaderPreviewError> {
    let started_at_ms = now_ms();
    let adapted_source = normalize_preview_wgsl(source);
    let module = wgsl::parse_str(&adapted_source).map_err(|error| {
        ShaderPreviewError::ParseFailed(vec![ShaderDiagnostic {
            message: error.emit_to_string(&adapted_source),
            line: None,
            column: None,
        }])
    })?;

    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    validator.validate(&module).map_err(|error| {
        ShaderPreviewError::ValidationFailed(vec![ShaderDiagnostic {
            message: error.emit_to_string(&adapted_source),
            line: None,
            column: None,
        }])
    })?;

    Ok(ShaderValidationResult {
        ok: true,
        diagnostics: Vec::new(),
        validate_ms: (now_ms() - started_at_ms).max(0.0),
    })
}

pub fn apply_preview_shader_source(
    state: &mut ShaderPreviewState,
    source: &str,
) -> ShaderApplyResult {
    let apply_started_at_ms = now_ms();
    match validate_wgsl_source(source) {
        Ok(validation) => {
            state.active_source = source.to_owned();
            state.last_good_source = source.to_owned();
            state.status = PreviewShaderStatus::Valid;
            state.diagnostics.clear();
            state.metrics.validate_ms = validation.validate_ms;
            state.metrics.apply_ms = (now_ms() - apply_started_at_ms).max(0.0);

            ShaderApplyResult {
                ok: true,
                diagnostics: Vec::new(),
                status: state.status.clone(),
                metrics: state.metrics.clone(),
            }
        }
        Err(ShaderPreviewError::ParseFailed(diagnostics))
        | Err(ShaderPreviewError::ValidationFailed(diagnostics)) => {
            state.status = PreviewShaderStatus::Invalid;
            state.diagnostics = diagnostics.clone();
            state.metrics.apply_ms = (now_ms() - apply_started_at_ms).max(0.0);

            ShaderApplyResult {
                ok: false,
                diagnostics,
                status: state.status.clone(),
                metrics: state.metrics.clone(),
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn validate_shader_source_json(source: &str) -> String {
    let payload = match validate_wgsl_source(source) {
        Ok(result) => result,
        Err(ShaderPreviewError::ParseFailed(diagnostics))
        | Err(ShaderPreviewError::ValidationFailed(diagnostics)) => ShaderValidationResult {
            ok: false,
            diagnostics,
            validate_ms: 0.0,
        },
    };
    serde_json::to_string(&payload).unwrap_or_else(|_| {
        "{\"ok\":false,\"diagnostics\":[{\"message\":\"serialization failure\",\"line\":null,\"column\":null}],\"validate_ms\":0.0}".to_string()
    })
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WASM_PREVIEW_STATE: std::cell::RefCell<ShaderPreviewState> =
        std::cell::RefCell::new(ShaderPreviewState::default());
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn apply_shader_source_json(source: &str) -> String {
    WASM_PREVIEW_STATE.with(|state| {
        let mut state = state.borrow_mut();
        let payload = apply_preview_shader_source(&mut state, source);
        serde_json::to_string(&payload).unwrap_or_else(|_| {
            "{\"ok\":false,\"diagnostics\":[{\"message\":\"serialization failure\",\"line\":null,\"column\":null,\"type\":\"error\"}],\"status\":\"Invalid\",\"metrics\":{\"validate_ms\":0.0,\"apply_ms\":0.0,\"last_frame_ms\":null,\"average_frame_ms\":null}}".to_string()
        })
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn preview_runtime_state_json() -> String {
    WASM_PREVIEW_STATE.with(|state| {
        let state = state.borrow();
        serde_json::to_string(&*state).unwrap_or_else(|_| {
            "{\"active_source\":\"\",\"last_good_source\":\"\",\"status\":\"Invalid\",\"diagnostics\":[{\"message\":\"serialization failure\",\"line\":null,\"column\":null}],\"metrics\":{\"validate_ms\":0.0,\"apply_ms\":0.0,\"last_frame_ms\":null,\"average_frame_ms\":null}}".to_string()
        })
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn boot_preview_runtime() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"sidereal-shader-preview wasm bridge booted".into());
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_SHADER: &str = r#"
@vertex
fn vertex(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let position = positions[vertex_index];
    return vec4<f32>(position, 0.0, 1.0);
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return vec4<f32>(0.25, 0.5, 0.75, 1.0);
}
"#;

    const BEVY_IMPORTED_FRAGMENT_SHADER: &str = r#"
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> spark_params: vec4<f32>;
@group(2) @binding(1) var<uniform> spark_color: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv * 2.0 - 1.0;
    return vec4<f32>(spark_color.rgb * length(uv), spark_params.w);
}
"#;

    #[test]
    fn validates_wgsl_source() {
        let result = validate_wgsl_source(VALID_SHADER).expect("valid shader should parse");
        assert!(result.ok);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn invalid_apply_does_not_overwrite_last_good_source() {
        let mut state = ShaderPreviewState::default();
        let first_apply = apply_preview_shader_source(&mut state, VALID_SHADER);
        assert!(first_apply.ok);

        let invalid_apply = apply_preview_shader_source(&mut state, "@fragment fn broken(");
        assert!(!invalid_apply.ok);
        assert_eq!(state.last_good_source, VALID_SHADER);
        assert_eq!(state.status, PreviewShaderStatus::Invalid);
        assert!(!state.diagnostics.is_empty());
    }

    #[test]
    fn validates_bevy_imported_fragment_shader() {
        let result = validate_wgsl_source(BEVY_IMPORTED_FRAGMENT_SHADER)
            .expect("bevy imported fragment shader should normalize");
        assert!(result.ok);
        assert!(result.diagnostics.is_empty());
    }
}

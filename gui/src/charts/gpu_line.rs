//! GPU-rendered line chart using wgpu.
//!
//! Uses `egui_wgpu::CallbackTrait` to insert custom wgpu render passes into
//! egui's render pipeline. Data is uploaded to a persistent GPU vertex buffer
//! and only re-uploaded when `data_version` changes (new analysis or filter change).
//! Pan/zoom transforms are passed as uniforms, so the buffer stays static during
//! interactive navigation.
//!
//! Ref: egui discussions #3810 (GPU plot), egui_plot issue #18 (optimize).

use bytemuck::{Pod, Zeroable};

/// A single point in the entropy chart, sent to the GPU as a vertex.
/// Position is in data-space coordinates; the vertex shader transforms
/// to clip space using the ViewTransform uniform.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct EntropyVertex {
    /// Data-space x (position number, 1-based)
    pub x: f32,
    /// Data-space y (entropy value)
    pub y: f32,
}

/// View transform uniform: maps data-space coordinates to clip-space [-1, 1].
/// Updated every frame on pan/zoom, but the vertex buffer stays static.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ViewTransform {
    /// Minimum x in data space (left edge of viewport)
    pub x_min: f32,
    /// Range of x in data space (viewport width)
    pub x_range: f32,
    /// Maximum y in data space (top of chart)
    pub y_max: f32,
    /// Padding to align to 16 bytes (wgpu uniform buffer alignment requirement)
    pub _padding: f32,
}

/// GPU resources stored in `egui_wgpu::CallbackResources`.
/// Registered once at app init, retrieved in `prepare()` and `paint()`.
pub struct EntropyGpuResources {
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub point_count: u32,
    /// Tracks data+filter changes. When a new version arrives, vertex data
    /// is re-uploaded in prepare(). Pan/zoom only updates the uniform buffer.
    pub data_version: u64,
}

/// Lightweight callback struct passed with each `egui::PaintCallback`.
/// GPU resources live in CallbackResources, not on this struct.
pub struct EntropyLineCallback {
    pub view_transform: ViewTransform,
    pub new_data_version: u64,
    /// Only `Some` when data changed and needs GPU upload
    pub new_vertices: Option<Vec<EntropyVertex>>,
}

impl egui_wgpu::CallbackTrait for EntropyLineCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources: &mut EntropyGpuResources = callback_resources.get_mut().unwrap();

        // Always update the uniform buffer with the current view transform
        queue.write_buffer(
            &resources.uniform_buffer,
            0,
            bytemuck::bytes_of(&self.view_transform),
        );

        // Re-upload vertex data only when data_version changes
        if let Some(ref vertices) = self.new_vertices {
            if !vertices.is_empty() {
                let byte_data = bytemuck::cast_slice(vertices);

                // wgpu does NOT auto-resize buffers. Recreate if new data is larger.
                if byte_data.len() as u64 > resources.vertex_buffer.size() {
                    resources.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("entropy_vertices"),
                        size: byte_data.len() as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                }
                queue.write_buffer(&resources.vertex_buffer, 0, byte_data);
            }
            resources.point_count = vertices.len() as u32;
            resources.data_version = self.new_data_version;
        }

        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let resources: &EntropyGpuResources = callback_resources.get().unwrap();
        if resources.point_count < 2 {
            return;
        }
        render_pass.set_pipeline(&resources.pipeline);
        render_pass.set_bind_group(0, &resources.bind_group, &[]);
        render_pass.set_vertex_buffer(0, resources.vertex_buffer.slice(..));
        render_pass.draw(0..resources.point_count, 0..1);
    }
}

/// WGSL shader source for the entropy line chart.
/// Vertex shader transforms data-space (position, entropy) to clip-space.
/// Fragment shader outputs a solid color (accent blue).
const SHADER_SOURCE: &str = r"
struct ViewTransform {
    x_min: f32,
    x_range: f32,
    y_max: f32,
    _padding: f32,
}

@group(0) @binding(0) var<uniform> view: ViewTransform;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
}

@vertex
fn vs_main(@location(0) data_pos: vec2f) -> VertexOutput {
    var out: VertexOutput;
    // Map data x to clip-space [-1, 1]
    let nx = (data_pos.x - view.x_min) / view.x_range * 2.0 - 1.0;
    // Map data y to clip-space [-1, 1] (y up)
    let ny = data_pos.y / view.y_max * 2.0 - 1.0;
    out.position = vec4f(nx, ny, 0.0, 1.0);
    // Color: accent blue with slight alpha
    out.color = vec4f(0.145, 0.388, 0.922, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return in.color;
}
";

/// Initialize GPU resources for the entropy chart.
/// Called once during app startup when wgpu_render_state is available.
pub fn init_gpu_resources(render_state: &egui_wgpu::RenderState) {
    let device = &render_state.device;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("entropy_shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("entropy_bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: Some(
                    std::num::NonZeroU64::new(std::mem::size_of::<ViewTransform>() as u64).unwrap(),
                ),
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("entropy_pipeline_layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    // Initial buffers (small default, resized on first data upload)
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("entropy_uniform"),
        size: std::mem::size_of::<ViewTransform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("entropy_vertices"),
        size: 1024, // Small initial, will be resized on first upload
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("entropy_bind_group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("entropy_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<EntropyVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                }],
            }],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: render_state.target_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    render_state
        .renderer
        .write()
        .callback_resources
        .insert(EntropyGpuResources {
            pipeline,
            vertex_buffer,
            uniform_buffer,
            bind_group,
            point_count: 0,
            data_version: 0,
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_size_matches_gpu_layout() {
        assert_eq!(std::mem::size_of::<EntropyVertex>(), 8);
    }

    #[test]
    fn test_view_transform_alignment() {
        // wgpu requires uniform buffers to be 16-byte aligned
        assert_eq!(std::mem::size_of::<ViewTransform>(), 16);
    }

    #[test]
    fn test_vertex_is_pod_and_zeroable() {
        let v = EntropyVertex { x: 1.0, y: 2.0 };
        let bytes = bytemuck::bytes_of(&v);
        assert_eq!(bytes.len(), 8);
    }
}

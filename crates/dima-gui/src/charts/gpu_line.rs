//! The solid (eligible) part of the chart's line, drawn by wgpu through an egui paint callback.
//!
//! Each segment is expanded in the vertex shader into a quad of the requested pixel width, so
//! the GPU line matches the CPU fallback (used when no wgpu device is available, e.g. in tests).
//! Vertex data is uploaded only when its key changes; the key is compared inside the renderer's
//! `prepare`, so an upload is never skipped because a frame was dropped (G14). Missing resources
//! make the callback a no-op instead of a panic (G24).

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

use super::series::ChartKey;

/// One corner of a segment quad.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
pub struct SegmentVertex {
    /// Segment start (x relative to the view's left edge, y in bits).
    pub a: [f32; 2],
    /// Segment end.
    pub b: [f32; 2],
    /// `(end, side)`: end 0 = at `a`, 1 = at `b`; side −1 or +1 of the line.
    pub corner: [f32; 2],
}

/// Uniforms: the data-to-clip mapping, the line width and the colour.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
pub struct LineUniforms {
    /// Data x at the left edge (relative, normally 0).
    pub x_min: f32,
    /// Data span of the view.
    pub x_range: f32,
    /// The plotted value at the top edge, in bits.
    pub y_max: f32,
    /// Half the line width, in physical pixels.
    pub half_width: f32,
    /// Size of the drawing rectangle in physical pixels.
    pub viewport: [f32; 2],
    /// Padding to the 16-byte boundary of `color` (WGSL `vec4<f32>` alignment).
    pub padding: [f32; 2],
    /// Linear RGBA.
    pub color: [f32; 4],
}

/// The six vertices (two triangles) of every segment of `points` (`(x, y)` with x already
/// relative to the view's left edge).
#[must_use]
pub fn segment_vertices(polylines: &[Vec<[f32; 2]>]) -> Vec<SegmentVertex> {
    const CORNERS: [[f32; 2]; 6] = [
        [0.0, -1.0],
        [1.0, -1.0],
        [1.0, 1.0],
        [0.0, -1.0],
        [1.0, 1.0],
        [0.0, 1.0],
    ];
    let segments: usize = polylines.iter().map(|p| p.len().saturating_sub(1)).sum();
    let mut out = Vec::with_capacity(segments * 6);
    for line in polylines {
        for w in line.windows(2) {
            for corner in CORNERS {
                out.push(SegmentVertex {
                    a: w[0],
                    b: w[1],
                    corner,
                });
            }
        }
    }
    out
}

/// Smallest vertex buffer kept allocated.
const MIN_VERTEX_BUFFER_BYTES: u64 = 4096;

/// GPU objects, created once and stored in egui-wgpu's callback resources.
pub struct LineResources {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertex_count: u32,
    /// Key of the vertex data in the buffer.
    uploaded_key: Option<ChartKey>,
}

/// One frame's request: the uniforms, and the vertex data with its key.
pub struct LineCallback {
    /// Mapping, width and colour.
    pub uniforms: LineUniforms,
    /// Identifies `vertices`; a different key triggers an upload.
    pub key: ChartKey,
    /// The quads (shared, so a frame that does not upload copies nothing).
    pub vertices: Arc<Vec<SegmentVertex>>,
}

impl egui_wgpu::CallbackTrait for LineCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(r) = resources.get_mut::<LineResources>() else {
            return Vec::new();
        };
        queue.write_buffer(&r.uniform_buffer, 0, bytemuck::bytes_of(&self.uniforms));
        if r.uploaded_key != Some(self.key) {
            let bytes: &[u8] = bytemuck::cast_slice(self.vertices.as_slice());
            let needed = bytes.len() as u64;
            let current = r.vertex_buffer.size();
            // Grow when too small; shrink when four times too large (hysteresis).
            if needed > current || (current > MIN_VERTEX_BUFFER_BYTES && needed * 4 < current) {
                r.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("dima_line_vertices"),
                    size: needed.max(MIN_VERTEX_BUFFER_BYTES),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            if !bytes.is_empty() {
                queue.write_buffer(&r.vertex_buffer, 0, bytes);
            }
            r.vertex_count = u32::try_from(self.vertices.len()).unwrap_or(0);
            r.uploaded_key = Some(self.key);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(r) = resources.get::<LineResources>() else {
            return;
        };
        if r.vertex_count == 0 || r.uploaded_key != Some(self.key) {
            return;
        }
        pass.set_pipeline(&r.pipeline);
        pass.set_bind_group(0, &r.bind_group, &[]);
        pass.set_vertex_buffer(0, r.vertex_buffer.slice(..));
        pass.draw(0..r.vertex_count, 0..1);
    }
}

const SHADER: &str = r"
struct View {
    x_min: f32,
    x_range: f32,
    y_max: f32,
    half_width: f32,
    viewport: vec2f,
    padding: vec2f,
    color: vec4f,
}

@group(0) @binding(0) var<uniform> view: View;

fn to_pixels(v: vec2f) -> vec2f {
    let clip = vec2f((v.x - view.x_min) / view.x_range * 2.0 - 1.0, v.y / view.y_max * 2.0 - 1.0);
    return clip * view.viewport * 0.5;
}

@vertex
fn vs_main(@location(0) a: vec2f, @location(1) b: vec2f, @location(2) corner: vec2f) -> @builtin(position) vec4f {
    let pa = to_pixels(a);
    let pb = to_pixels(b);
    var dir = pb - pa;
    let len = length(dir);
    if (len < 1e-6) {
        dir = vec2f(1.0, 0.0);
    } else {
        dir = dir / len;
    }
    let normal = vec2f(-dir.y, dir.x);
    let at_b = corner.x > 0.5;
    let base = select(pa, pb, at_b);
    // Extend each end by half the width so consecutive segments join without notches.
    let along = dir * view.half_width * select(-1.0, 1.0, at_b);
    let p = base + normal * view.half_width * corner.y + along;
    return vec4f(p / (view.viewport * 0.5), 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4f {
    return view.color;
}
";

/// Creates the pipeline and buffers. Called once, when the app starts with a wgpu renderer.
pub fn init(render_state: &egui_wgpu::RenderState) {
    let device = &render_state.device;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("dima_line_shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let uniform_size = std::mem::size_of::<LineUniforms>() as u64;
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("dima_line_bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(uniform_size),
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("dima_line_pipeline_layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("dima_line_uniforms"),
        size: uniform_size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("dima_line_vertices"),
        size: MIN_VERTEX_BUFFER_BYTES,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("dima_line_bind_group"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("dima_line_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<SegmentVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![
                    0 => Float32x2,
                    1 => Float32x2,
                    2 => Float32x2
                ],
            })],
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
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..wgpu::PrimitiveState::default()
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
        .insert(LineResources {
            pipeline,
            vertex_buffer,
            uniform_buffer,
            bind_group,
            vertex_count: 0,
            uploaded_key: None,
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_match_the_shader() {
        assert_eq!(std::mem::size_of::<SegmentVertex>(), 24);
        assert_eq!(std::mem::size_of::<LineUniforms>(), 48);
        let u = LineUniforms {
            x_min: 0.0,
            x_range: 1.0,
            y_max: 1.0,
            half_width: 0.75,
            viewport: [100.0, 50.0],
            padding: [0.0; 2],
            color: [0.25, 0.5, 0.75, 1.0],
        };
        let bytes = bytemuck::bytes_of(&u);
        let color: &[f32] = bytemuck::cast_slice(&bytes[32..48]);
        assert_eq!(color, &[0.25, 0.5, 0.75, 1.0], "vec4 at offset 32");
    }

    #[test]
    fn six_vertices_per_segment() {
        let v = segment_vertices(&[vec![[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]], vec![[5.0, 1.0]]]);
        assert_eq!(v.len(), 12, "two segments; a lone point has none");
        assert!(
            v.iter()
                .take(6)
                .all(|c| c.a == [0.0, 0.0] && c.b == [1.0, 1.0])
        );
        assert!(segment_vertices(&[]).is_empty());
    }
}

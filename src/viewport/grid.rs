use eframe::egui;
use eframe::egui_wgpu::{self, RenderState, wgpu};
use eframe::wgpu::util::DeviceExt;
use glam::Mat4;

/// Half-extent of the ground grid in world units (blocks).
const GRID_HALF_EXTENT: i32 = 64;
/// Every n-th line is drawn brighter, like Blender's major grid lines.
const MAJOR_LINE_EVERY: i32 = 8;

const MINOR_COLOR: [f32; 4] = [0.32, 0.32, 0.34, 1.0];
const MAJOR_COLOR: [f32; 4] = [0.45, 0.45, 0.48, 1.0];
const X_AXIS_COLOR: [f32; 4] = [0.85, 0.28, 0.30, 1.0];
const Y_AXIS_COLOR: [f32; 4] = [0.42, 0.75, 0.25, 1.0];
const Z_AXIS_COLOR: [f32; 4] = [0.25, 0.45, 0.90, 1.0];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

/// GPU resources for the grid, stored in egui_wgpu's `CallbackResources`.
pub struct GridRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
}

impl GridRenderer {
    /// Create the pipeline once and register it with the egui_wgpu renderer.
    pub fn init(render_state: &RenderState) {
        let device = &render_state.device;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("grid_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/grid.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("grid_uniforms"),
            size: std::mem::size_of::<Mat4>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("grid_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grid_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let vertices = build_grid_vertices();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("grid_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("grid_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: render_state.target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            // Must match the depth buffer eframe creates (NativeOptions::depth_buffer = 32).
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        render_state
            .renderer
            .write()
            .callback_resources
            .insert(GridRenderer {
                pipeline,
                bind_group,
                uniform_buffer,
                vertex_buffer,
                vertex_count: vertices.len() as u32,
            });
    }
}

/// Per-frame paint callback carrying the camera matrix.
pub struct GridCallback {
    pub view_proj: Mat4,
}

impl egui_wgpu::CallbackTrait for GridCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let renderer: &GridRenderer = callback_resources
            .get()
            .expect("GridRenderer not initialized");
        queue.write_buffer(
            &renderer.uniform_buffer,
            0,
            bytemuck::cast_slice(&self.view_proj.to_cols_array()),
        );
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let renderer: &GridRenderer = callback_resources
            .get()
            .expect("GridRenderer not initialized");
        render_pass.set_pipeline(&renderer.pipeline);
        render_pass.set_bind_group(0, &renderer.bind_group, &[]);
        render_pass.set_vertex_buffer(0, renderer.vertex_buffer.slice(..));
        render_pass.draw(0..renderer.vertex_count, 0..1);
    }
}

fn build_grid_vertices() -> Vec<Vertex> {
    let n = GRID_HALF_EXTENT;
    let extent = n as f32;
    let mut vertices = Vec::new();

    let mut line = |a: [f32; 3], b: [f32; 3], color: [f32; 4]| {
        vertices.push(Vertex { position: a, color });
        vertices.push(Vertex { position: b, color });
    };

    for i in -n..=n {
        if i == 0 {
            // The center lines are drawn as colored axes below.
            continue;
        }
        let color = if i % MAJOR_LINE_EVERY == 0 {
            MAJOR_COLOR
        } else {
            MINOR_COLOR
        };
        let t = i as f32;
        // Lines parallel to Z, then parallel to X.
        line([t, 0.0, -extent], [t, 0.0, extent], color);
        line([-extent, 0.0, t], [extent, 0.0, t], color);
    }

    // World axes: X red, Y green, Z blue.
    line([-extent, 0.0, 0.0], [extent, 0.0, 0.0], X_AXIS_COLOR);
    line([0.0, -extent, 0.0], [0.0, extent, 0.0], Y_AXIS_COLOR);
    line([0.0, 0.0, -extent], [0.0, 0.0, extent], Z_AXIS_COLOR);

    vertices
}

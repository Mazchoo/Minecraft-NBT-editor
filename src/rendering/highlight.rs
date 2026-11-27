use eframe::egui;
use eframe::egui_wgpu::{self, RenderState, wgpu};
use glam::Mat4;

use crate::constants;
use crate::models::Face;

use super::common::face_triangles;
use super::grid_mesh::Vertex;

/// GPU resources for the placement-target face overlay.
pub struct HighlightRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
}

impl HighlightRenderer {
    pub fn init(render_state: &RenderState) {
        let device = &render_state.device;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("highlight_shader"),
            source: wgpu::ShaderSource::Wgsl(constants::GRID_SHADER.into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("highlight_uniforms"),
            size: std::mem::size_of::<Mat4>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("highlight_bind_group_layout"),
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
            label: Some("highlight_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("highlight_vertices"),
            size: (std::mem::size_of::<Vertex>() * 6) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("highlight_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("highlight_pipeline"),
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
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
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
            .insert(HighlightRenderer {
                pipeline,
                bind_group,
                uniform_buffer,
                vertex_buffer,
                vertex_count: 0,
            });
    }
}

pub struct HighlightCallback {
    pub view_proj: Mat4,
    /// Face to overlay, if any.
    pub target: Option<Highlight>,
}

/// A block face to overlay, and the overlay's RGBA colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlight {
    pub cell: glam::IVec3,
    pub face: Face,
    pub color: [f32; 4],
}

impl egui_wgpu::CallbackTrait for HighlightCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let renderer: &mut HighlightRenderer = callback_resources
            .get_mut()
            .expect("HighlightRenderer not initialized");
        queue.write_buffer(
            &renderer.uniform_buffer,
            0,
            bytemuck::cast_slice(&self.view_proj.to_cols_array()),
        );
        match self.target {
            Some(Highlight { cell, face, color }) => {
                let vertices = face_triangles(face, cell, color);
                queue.write_buffer(&renderer.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
                renderer.vertex_count = vertices.len() as u32;
            }
            None => renderer.vertex_count = 0,
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let renderer: &HighlightRenderer = callback_resources
            .get()
            .expect("HighlightRenderer not initialized");
        if renderer.vertex_count == 0 {
            return;
        }
        render_pass.set_pipeline(&renderer.pipeline);
        render_pass.set_bind_group(0, &renderer.bind_group, &[]);
        render_pass.set_vertex_buffer(0, renderer.vertex_buffer.slice(..));
        render_pass.draw(0..renderer.vertex_count, 0..1);
    }
}

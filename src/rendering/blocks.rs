use std::collections::BTreeMap;
use std::sync::Arc;

use eframe::egui;
use eframe::egui_wgpu::{self, RenderState, wgpu};
use eframe::wgpu::util::DeviceExt;
use glam::Mat4;
use image::RgbaImage;

use crate::block_models::ModelLibrary;
use crate::config::Config;
use crate::constants;
use crate::models::Block;
use crate::textures::TextureLibrary;
use crate::world::World;

use super::block_mesh::{BlockMesh, BlockVertex};
use super::visibility::Visibility;

/// Pixels in one texture-array layer.
const TEXELS: usize = (Config::TEXTURE_SIZE * Config::TEXTURE_SIZE) as usize;

/// CPU-side mesh plus packed texture-array pixels, rebuilt when the world changes.
#[derive(Debug, Clone)]
pub struct GpuMesh {
    pub vertices: Vec<BlockVertex>,
    pub indices: Vec<u32>,
    pub texture_data: Vec<u8>,
    pub layer_count: u32,
}

/// GPU resources for textured block meshes, stored in egui_wgpu's `CallbackResources`.
pub struct BlockRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    texture: wgpu::Texture,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl BlockRenderer {
    pub fn init(render_state: &RenderState, textures: &TextureLibrary) {
        let device = &render_state.device;
        let queue = &render_state.queue;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blocks_shader"),
            source: wgpu::ShaderSource::Wgsl(constants::BLOCKS_SHADER.into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("blocks_uniforms"),
            size: std::mem::size_of::<Mat4>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blocks_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blocks_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let missing = textures.missing_image();
        let (texture_data, layer_count) = pack_layers(std::slice::from_ref(&missing));
        let texture = create_texture_array(device, layer_count);
        write_texture_array(queue, &texture, &texture_data, layer_count);
        let bind_group = make_bind_group(
            device,
            &bind_group_layout,
            &uniform_buffer,
            &texture,
            &sampler,
        );

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("blocks_vertices"),
            size: std::mem::size_of::<BlockVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("blocks_indices"),
            size: std::mem::size_of::<u32>() as u64 * 3,
            usage: wgpu::BufferUsages::INDEX,
            mapped_at_creation: false,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blocks_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blocks_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<BlockVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Uint32, 3 => Unorm8x4],
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
                cull_mode: Some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
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
            .insert(BlockRenderer {
                pipeline,
                bind_group,
                bind_group_layout,
                uniform_buffer,
                sampler,
                texture,
                vertex_buffer,
                index_buffer,
                index_count: 0,
            });
    }

    fn upload_mesh(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, mesh: &GpuMesh) {
        self.texture = create_texture_array(device, mesh.layer_count);
        write_texture_array(queue, &self.texture, &mesh.texture_data, mesh.layer_count);
        self.bind_group = make_bind_group(
            device,
            &self.bind_group_layout,
            &self.uniform_buffer,
            &self.texture,
            &self.sampler,
        );

        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            self.index_count = 0;
            return;
        }

        self.vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("blocks_vertices"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("blocks_indices"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.index_count = mesh.indices.len() as u32;
    }
}

pub struct BlockCallback {
    pub view_proj: Mat4,
    pub mesh: Option<Arc<GpuMesh>>,
}

impl egui_wgpu::CallbackTrait for BlockCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let renderer: &mut BlockRenderer = callback_resources
            .get_mut()
            .expect("BlockRenderer not initialized");
        queue.write_buffer(
            &renderer.uniform_buffer,
            0,
            bytemuck::cast_slice(&self.view_proj.to_cols_array()),
        );
        if let Some(mesh) = &self.mesh {
            renderer.upload_mesh(device, queue, mesh);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let renderer: &BlockRenderer = callback_resources
            .get()
            .expect("BlockRenderer not initialized");
        if renderer.index_count == 0 {
            return;
        }
        render_pass.set_pipeline(&renderer.pipeline);
        render_pass.set_bind_group(0, &renderer.bind_group, &[]);
        render_pass.set_vertex_buffer(0, renderer.vertex_buffer.slice(..));
        render_pass.set_index_buffer(renderer.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..renderer.index_count, 0, 0..1);
    }
}

/// Builds a textured mesh for every occupied cell. Layer 0 is always the missing texture.
///
/// Only faces that can be seen from outside the structure are emitted: faces
/// pressed against an occluding neighbour and faces lining sealed cavities are
/// dropped (see [`Visibility`]).
pub fn build_gpu_mesh(world: &World, models: &ModelLibrary, textures: &TextureLibrary) -> GpuMesh {
    let empty = BTreeMap::new();
    let has_alpha = |key: &str| {
        textures
            .get_record(key)
            .is_some_and(|texture| texture.has_alpha())
    };
    // Each distinct block with the sides of its cell it hides completely.
    let mut blocks: BTreeMap<String, (Block, u8)> = BTreeMap::new();
    let mut keys: BTreeMap<String, u32> = BTreeMap::new();
    keys.insert(String::new(), 0);
    let mut next_layer = 1u32;

    for (_, name) in world.iter() {
        if blocks.contains_key(name) {
            continue;
        }
        let block = Block::from_resolved(name, &models.resolve(name, &empty));
        for key in block.texture_keys() {
            keys.entry(key).or_insert_with(|| {
                let layer = next_layer;
                next_layer += 1;
                layer
            });
        }
        let occlusion = block.occlusion_mask(has_alpha);
        blocks.insert(name.to_owned(), (block, occlusion));
    }

    let visibility = Visibility::compute(
        world
            .iter()
            .filter_map(|(cell, name)| blocks.get(name).map(|(_, occlusion)| (cell, *occlusion))),
    );

    let mut mesh = BlockMesh::default();
    for (cell, name) in world.iter() {
        let Some((block, _)) = blocks.get(name) else {
            continue;
        };
        mesh.place_culled(
            block,
            cell,
            |key| keys.get(key).copied().unwrap_or(0),
            |face| visibility.face_visible(cell, face),
        );
    }

    let layer_count = next_layer.max(1);
    let mut layer_images: Vec<Arc<RgbaImage>> = Vec::with_capacity(layer_count as usize);
    layer_images.push(textures.missing_image());
    layer_images.resize(layer_count as usize, textures.missing_image());
    for (key, layer) in &keys {
        if *layer == 0 {
            continue;
        }
        layer_images[*layer as usize] = textures.get(key);
    }
    let (texture_data, layer_count) = pack_layers(&layer_images);

    GpuMesh {
        vertices: mesh.vertices,
        indices: mesh.indices,
        texture_data,
        layer_count,
    }
}

fn create_texture_array(device: &wgpu::Device, layer_count: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("block_texture_array"),
        size: wgpu::Extent3d {
            width: Config::TEXTURE_SIZE,
            height: Config::TEXTURE_SIZE,
            depth_or_array_layers: layer_count.max(1),
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn write_texture_array(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    data: &[u8],
    layer_count: u32,
) {
    let row_pitch = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(row_pitch),
            rows_per_image: Some(Config::TEXTURE_SIZE),
        },
        wgpu::Extent3d {
            width: Config::TEXTURE_SIZE,
            height: Config::TEXTURE_SIZE,
            depth_or_array_layers: layer_count.max(1),
        },
    );
}

fn make_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform_buffer: &wgpu::Buffer,
    texture: &wgpu::Texture,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("block_texture_array_view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("blocks_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn pack_layers(layers: &[Arc<RgbaImage>]) -> (Vec<u8>, u32) {
    let count = layers.len().max(1) as u32;
    let row_pitch = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    let layer_size = row_pitch * Config::TEXTURE_SIZE as usize;
    let mut data = vec![0u8; layer_size * count as usize];
    for (i, img) in layers.iter().enumerate() {
        let pixels = sample_layer(img);
        let base = i * layer_size;
        for y in 0..Config::TEXTURE_SIZE as usize {
            let dst = base + y * row_pitch;
            for x in 0..Config::TEXTURE_SIZE as usize {
                let px = pixels[y * Config::TEXTURE_SIZE as usize + x];
                let o = dst + x * 4;
                data[o..o + 4].copy_from_slice(&px);
            }
        }
    }
    (data, count)
}

fn sample_layer(img: &RgbaImage) -> [[u8; 4]; TEXELS] {
    let w = img.width().max(1);
    let h = img.height().max(1);
    let frame_h = if h >= w { w } else { h };
    let mut out = [[0u8; 4]; TEXELS];
    for y in 0..Config::TEXTURE_SIZE {
        for x in 0..Config::TEXTURE_SIZE {
            let sx = x * w / Config::TEXTURE_SIZE;
            let sy = y * frame_h / Config::TEXTURE_SIZE;
            out[(y * Config::TEXTURE_SIZE + x) as usize] =
                img.get_pixel(sx.min(w - 1), sy.min(frame_h - 1)).0;
        }
    }
    out
}

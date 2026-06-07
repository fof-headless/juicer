//! Native wgpu renderer (Lite mode).
//!
//! Headless / offscreen: renders a scene at a given frame into an RGBA texture,
//! reads it back, and writes a PNG. Used both for viewport preview (single
//! frame) and for video export (frame sequence → encoder).
//!
//! No window, no surface — pure GPU compute-to-image. Metal on macOS.

mod mesh;

use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

use crate::scene::{hex_to_linear, Element, ElementKind, Scene};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 4],
    light_col: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ObjectUniform {
    model: [[f32; 4]; 4],
    normal: [[f32; 4]; 4],
    tint: [f32; 4],
    flags: [f32; 4],
}

struct GpuMesh {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    index_count: u32,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    globals_bgl: wgpu::BindGroupLayout,
    object_bgl: wgpu::BindGroupLayout,
    meshes: HashMap<&'static str, GpuMesh>,
    sampler: wgpu::Sampler,
    white_texture: wgpu::TextureView,
    /// Cache of loaded image textures keyed by path.
    texture_cache: HashMap<String, wgpu::TextureView>,
}

impl Renderer {
    pub fn new() -> Result<Self> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL | wgpu::Backends::VULKAN | wgpu::Backends::DX12,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .context("No suitable GPU adapter found")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("juicer-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .context("Failed to create GPU device")?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("juicer-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let object_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("object-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
                        view_dimension: wgpu::TextureViewDimension::D2,
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("juicer-pl"),
            bind_group_layouts: &[&globals_bgl, &object_bgl],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<mesh::Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 24, shader_location: 2, format: wgpu::VertexFormat::Float32x2 },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("juicer-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[vertex_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None, // double-sided planes
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState { count: 1, ..Default::default() },
            multiview: None,
            cache: None,
        });

        // Upload primitive meshes.
        let mut meshes = HashMap::new();
        for (name, data) in [
            ("plane", mesh::plane()),
            ("cube", mesh::cube()),
            ("sphere", mesh::sphere(48, 24)),
        ] {
            let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(name),
                contents: bytemuck::cast_slice(&data.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(name),
                contents: bytemuck::cast_slice(&data.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            meshes.insert(name, GpuMesh { vbuf, ibuf, index_count: data.indices.len() as u32 });
        }

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("juicer-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let white_texture = make_white_texture(&device, &queue);

        Ok(Self {
            device,
            queue,
            pipeline,
            globals_bgl,
            object_bgl,
            meshes,
            sampler,
            white_texture,
            texture_cache: HashMap::new(),
        })
    }

    /// Render the scene at `frame` and return RGBA8 pixels (width*height*4).
    pub fn render_frame(&mut self, scene: &Scene, frame: f32) -> Result<(Vec<u8>, u32, u32)> {
        let w = scene.render.width;
        let h = scene.render.height;

        // Pre-load any image textures referenced by elements.
        for el in &scene.elements {
            if let Some(path) = &el.image_path {
                if !self.texture_cache.contains_key(path) {
                    if let Ok(view) = load_texture(&self.device, &self.queue, path) {
                        self.texture_cache.insert(path.clone(), view);
                    }
                }
            }
        }

        let color_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("juicer-color"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color_tex.create_view(&Default::default());

        let depth_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("juicer-depth"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_tex.create_view(&Default::default());

        // ── Globals (camera + light) ──
        let (cam_pos, cam_target) = eval_camera(scene, frame);
        let aspect = w as f32 / h as f32;
        let proj = Mat4::perspective_rh(
            scene.camera.fov_deg.to_radians(),
            aspect,
            scene.camera.near,
            scene.camera.far,
        );
        let view = Mat4::look_at_rh(cam_pos, cam_target, Vec3::Y);
        let view_proj = proj * view;

        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            light_dir: [
                scene.light.direction[0],
                scene.light.direction[1],
                scene.light.direction[2],
                scene.light.intensity,
            ],
            light_col: [scene.light.color[0], scene.light.color[1], scene.light.color[2], scene.ambient],
        };
        let globals_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globals"),
            contents: bytemuck::bytes_of(&globals),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let globals_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals-bg"),
            layout: &self.globals_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() }],
        });

        // ── Per-object bind groups ──
        struct Draw {
            mesh: &'static str,
            bg: wgpu::BindGroup,
        }
        let mut draws = Vec::new();

        for el in &scene.elements {
            if !el.visible {
                continue;
            }
            let model = eval_model(el, frame);
            let opacity = eval_opacity(el, frame);
            let normal_mat = model.inverse().transpose();
            let tint = hex_to_linear(&el.color);

            let has_tex = el.image_path.as_ref().map_or(false, |p| self.texture_cache.contains_key(p));
            let obj = ObjectUniform {
                model: model.to_cols_array_2d(),
                normal: normal_mat.to_cols_array_2d(),
                tint: [tint[0], tint[1], tint[2], opacity],
                flags: [if el.unlit { 1.0 } else { 0.0 }, if has_tex { 1.0 } else { 0.0 }, 0.0, 0.0],
            };
            let obj_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("obj"),
                contents: bytemuck::bytes_of(&obj),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let tex_view = el
                .image_path
                .as_ref()
                .and_then(|p| self.texture_cache.get(p))
                .unwrap_or(&self.white_texture);

            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("obj-bg"),
                layout: &self.object_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: obj_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(tex_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });

            let mesh = match el.kind {
                ElementKind::Plane => "plane",
                ElementKind::Box => "cube",
                ElementKind::Sphere => "sphere",
            };
            draws.push(Draw { mesh, bg });
        }

        // ── Encode render pass ──
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let bg = scene.render.background;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("juicer-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: bg[0] as f64,
                            g: bg[1] as f64,
                            b: bg[2] as f64,
                            a: bg[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &globals_bg, &[]);
            for d in &draws {
                let m = &self.meshes[d.mesh];
                pass.set_bind_group(1, &d.bg, &[]);
                pass.set_vertex_buffer(0, m.vbuf.slice(..));
                pass.set_index_buffer(m.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.index_count, 0, 0..1);
            }
        }

        // ── Read back pixels ──
        let bytes_per_row = align_256(w * 4);
        let buffer_size = (bytes_per_row * h) as wgpu::BufferAddress;
        let read_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &color_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &read_buf,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );

        self.queue.submit(Some(encoder.finish()));

        // Map and read.
        let slice = read_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv().context("readback channel closed")?.context("buffer map failed")?;

        let data = slice.get_mapped_range();
        // Strip row padding.
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h {
            let start = (row * bytes_per_row) as usize;
            let end = start + (w * 4) as usize;
            pixels.extend_from_slice(&data[start..end]);
        }
        drop(data);
        read_buf.unmap();

        Ok((pixels, w, h))
    }

    /// Render a single frame and save it as PNG to `path`.
    pub fn render_to_png(&mut self, scene: &Scene, frame: f32, path: &str) -> Result<()> {
        let (pixels, w, h) = self.render_frame(scene, frame)?;
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).context("creating output directory")?;
        }
        image::save_buffer(path, &pixels, w, h, image::ColorType::Rgba8)
            .context("failed to write PNG")?;
        Ok(())
    }

    /// Render a frame and return raw PNG bytes (for embedding in MCP responses).
    pub fn render_to_png_bytes(&mut self, scene: &Scene, frame: f32) -> Result<Vec<u8>> {
        let (pixels, w, h) = self.render_frame(scene, frame)?;
        let mut buf = std::io::Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut buf, &pixels, w, h,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        ).context("encoding PNG bytes")?;
        Ok(buf.into_inner())
    }
}

// ── Animation evaluation helpers ──────────────────────────────────────────────

fn eval_model(el: &Element, frame: f32) -> Mat4 {
    let pos = el
        .track("position")
        .and_then(|t| t.sample(frame))
        .map(|v| v.as_vec3())
        .unwrap_or(el.position);
    let rot = el
        .track("rotation")
        .and_then(|t| t.sample(frame))
        .map(|v| v.as_vec3())
        .unwrap_or(el.rotation);
    let mut scl = el
        .track("scale")
        .and_then(|t| t.sample(frame))
        .map(|v| v.as_vec3())
        .unwrap_or(el.scale);

    // Planes carry width/height into base scale.
    if matches!(el.kind, ElementKind::Plane) {
        scl = [scl[0] * el.width, scl[1] * el.height, scl[2]];
    }

    Mat4::from_translation(Vec3::from(pos))
        * Mat4::from_euler(glam::EulerRot::XYZ, rot[0], rot[1], rot[2])
        * Mat4::from_scale(Vec3::from(scl))
}

fn eval_opacity(el: &Element, frame: f32) -> f32 {
    el.track("opacity")
        .and_then(|t| t.sample(frame))
        .map(|v| v.as_scalar())
        .unwrap_or(el.opacity)
        .clamp(0.0, 1.0)
}

fn eval_camera(scene: &Scene, frame: f32) -> (Vec3, Vec3) {
    let cam = &scene.camera;
    let pos = cam
        .tracks
        .iter()
        .find(|t| t.property == "position")
        .and_then(|t| t.track.sample(frame))
        .map(|v| v.as_vec3())
        .unwrap_or(cam.position);
    let target = cam
        .tracks
        .iter()
        .find(|t| t.property == "target")
        .and_then(|t| t.track.sample(frame))
        .map(|v| v.as_vec3())
        .unwrap_or(cam.target);
    (Vec3::from(pos), Vec3::from(target))
}

// ── Texture helpers ───────────────────────────────────────────────────────────

fn make_white_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("white"),
        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[255, 255, 255, 255],
        wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
    );
    tex.create_view(&Default::default())
}

fn load_texture(device: &wgpu::Device, queue: &wgpu::Queue, path: &str) -> Result<wgpu::TextureView> {
    let img = image::open(path).with_context(|| format!("opening image {path}"))?.to_rgba8();
    let (w, h) = img.dimensions();
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(path),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &img,
        wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    Ok(tex.create_view(&Default::default()))
}

fn align_256(n: u32) -> u32 {
    (n + 255) & !255
}

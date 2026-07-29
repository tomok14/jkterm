use std::sync::Arc;

use wgpu::util::DeviceExt;
use winit::window::Window;
use winit::dpi::PhysicalSize;

use crate::config::Config;
use crate::terminal::Terminal;
use glyphon::{Color, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Attrs, Family, Buffer};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RectUniform {
    transform: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RectVertex {
    position: [f32; 2],
    color: [f32; 4],
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,

    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    text_renderer: TextRenderer,

    cell_width: f64,
    cell_height: f64,
    padding_x: f64,
    padding_y: f64,
    font_size: f32,
    font_family: String,
    format: wgpu::TextureFormat,

    rect_pipeline: wgpu::RenderPipeline,
    rect_vertex_buf: wgpu::Buffer,
    rect_index_buf: wgpu::Buffer,
    rect_uniform_buf: wgpu::Buffer,
    rect_bind_group: wgpu::BindGroup,
}

impl Renderer {
    pub async fn new(window: &Arc<Window>, config: &Config) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let sc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &sc);

        let font_size = config.font_size;
        let cell_height = font_size as f64 * 1.2;
        let cell_width = font_size as f64 * 0.6;

        let mut font_system = FontSystem::new();
        let mut atlas = TextAtlas::new(&device, &queue, format);
        let text_renderer = TextRenderer::new(
            &mut atlas,
            &device,
            wgpu::MultisampleState::default(),
            None,
        );

        // --- Rect pipeline ---
        let rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Rect Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("shaders/rect.wgsl"))),
        });

        let uniform = RectUniform {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };

        let rect_uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Rect Uniform"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Rect BGL"),
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

        let rect_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Rect BG"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: rect_uniform_buf.as_entire_binding(),
            }],
        });

        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Rect PL"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Rect Pipeline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &rect_shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RectVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &rect_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let empty_v: [RectVertex; 0] = [];
        let rect_vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Rect VB"),
            contents: bytemuck::cast_slice(&empty_v),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let empty_i: [u16; 0] = [];
        let rect_index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Rect IB"),
            contents: bytemuck::cast_slice(&empty_i),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            device,
            queue,
            surface,
            config: sc,
            size,
            font_system,
            swash_cache: SwashCache::new(),
            atlas,
            text_renderer,
            cell_width,
            cell_height,
            padding_x: config.padding_x,
            padding_y: config.padding_y,
            font_size,
            font_family: config.font_family.clone(),
            format,
            rect_pipeline,
            rect_vertex_buf,
            rect_index_buf,
            rect_uniform_buf,
            rect_bind_group,
        }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn cell_size(&self) -> (f64, f64) {
        (self.cell_width, self.cell_height)
    }

    pub fn terminal_size(&self) -> (usize, usize) {
        let cols = ((self.size.width as f64 - 2.0 * self.padding_x) / self.cell_width).floor() as usize;
        let rows = ((self.size.height as f64 - 2.0 * self.padding_y) / self.cell_height).floor() as usize;
        (cols.max(1), rows.max(1))
    }

    fn update_transform(&mut self) {
        let w = self.size.width as f32;
        let h = self.size.height as f32;
        let uniform = RectUniform {
            transform: [
                [2.0 / w, 0.0, 0.0, -1.0],
                [0.0, -2.0 / h, 0.0, 1.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        self.queue.write_buffer(&self.rect_uniform_buf, 0, bytemuck::cast_slice(&[uniform]));
    }

    pub fn render(&mut self, terminal: &Terminal) {
        self.update_transform();

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(e) => {
                log::error!("surface error: {e:?}");
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("encoder"),
        });

        // --- Clear ---
        {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.04, g: 0.04, b: 0.06, a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // --- Build rects ---
        let grid = terminal.grid();
        let cursor = terminal.cursor();
        let rows = terminal.rows();
        let cols = terminal.cols();

        let mut rects: Vec<(f32, f32, f32, f32, [f32; 4])> = Vec::new();
        let px = self.padding_x as f32;
        let py = self.padding_y as f32;
        let cw = self.cell_width as f32;
        let ch = self.cell_height as f32;

        for y in 0..rows {
            for x in 0..cols {
                let cell = &grid[y][x];
                let has_bg = cell.bg.r != 0 || cell.bg.g != 0 || cell.bg.b != 0;
                if has_bg {
                    rects.push((
                        px + x as f32 * cw,
                        py + y as f32 * ch,
                        cw, ch,
                        [cell.bg.r as f32 / 255.0, cell.bg.g as f32 / 255.0, cell.bg.b as f32 / 255.0, 1.0],
                    ));
                }
            }
        }

        if cursor.visible && cursor.y < rows && cursor.x < cols {
            rects.push((
                px + cursor.x as f32 * cw,
                py + cursor.y as f32 * ch,
                cw, ch,
                [1.0, 1.0, 1.0, 0.3],
            ));
        }

        if !rects.is_empty() {
            let mut vertices = Vec::with_capacity(rects.len() * 4);
            let mut indices = Vec::with_capacity(rects.len() * 6);

            for (i, (rx, ry, rw, rh, color)) in rects.iter().enumerate() {
                let x1 = *rx; let y1 = *ry;
                let x2 = *rx + *rw; let y2 = *ry + *rh;
                let base = i as u16 * 4;
                vertices.push(RectVertex { position: [x1, y1], color: *color });
                vertices.push(RectVertex { position: [x2, y1], color: *color });
                vertices.push(RectVertex { position: [x2, y2], color: *color });
                vertices.push(RectVertex { position: [x1, y2], color: *color });
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }

            self.queue.write_buffer(&self.rect_vertex_buf, 0, bytemuck::cast_slice(&vertices));
            self.queue.write_buffer(&self.rect_index_buf, 0, bytemuck::cast_slice(&indices));

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rects"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.rect_pipeline);
            pass.set_bind_group(0, &self.rect_bind_group, &[]);
            pass.set_vertex_buffer(0, self.rect_vertex_buf.slice(..));
            pass.set_index_buffer(self.rect_index_buf.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }

        // --- Text ---
        {
            let mut text = String::new();
            for y in 0..rows {
                for x in 0..cols {
                    let cell = &grid[y][x];
                    if cell.blank || cell.is_wide_continuation {
                        text.push(' ');
                    } else {
                        text.push(cell.ch);
                    }
                }
                if y < rows - 1 {
                    text.push('\n');
                }
            }

            if !text.is_empty() {
                let w = self.size.width as f32;
                let h = self.size.height as f32;

                let mut buffer = Buffer::new(
                    &mut self.font_system,
                    Metrics::new(self.font_size, self.cell_height as f32),
                );
                buffer.set_size(&mut self.font_system, w, h);
                buffer.set_text(
                    &mut self.font_system,
                    &text,
                    Attrs::new()
                        .family(resolve_family(&self.font_family))
                        .color(Color::rgb(0xCC, 0xCC, 0xCC)),
                    Shaping::Basic,
                );
                buffer.shape_until_scroll(&mut self.font_system);

                let text_area = TextArea {
                    buffer: &buffer,
                    left: self.padding_x as f32,
                    top: self.padding_y as f32,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: self.size.width as i32,
                        bottom: self.size.height as i32,
                    },
                    default_color: Color::rgb(0xCC, 0xCC, 0xCC),
                };

                let _ = self.text_renderer.prepare(
                    &self.device,
                    &self.queue,
                    &mut self.font_system,
                    &mut self.atlas,
                    Resolution {
                        width: self.size.width,
                        height: self.size.height,
                    },
                    [text_area],
                    &mut self.swash_cache,
                );

                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("text"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                let _ = self.text_renderer.render(&self.atlas, &mut pass);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        self.atlas.trim();
    }
}

fn resolve_family(families: &str) -> Family<'_> {
    families.split(',')
        .next()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|name| match name.to_ascii_lowercase() {
            ref n if n == "monospace" => Family::Monospace,
            ref n if n == "sans-serif" => Family::SansSerif,
            ref n if n == "serif" => Family::Serif,
            _ => Family::Name(name),
        })
        .unwrap_or(Family::Monospace)
}

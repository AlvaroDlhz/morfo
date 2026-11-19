//! GPU renderer for [`morfo`] using wgpu.
//!
//! Every primitive of a frame becomes one instance of a single quad: one
//! instanced draw call per clip region, in the exact paint order of the
//! [`DrawList`]. Rounded rectangles, shadows, strokes and polygons are
//! computed in the fragment shader from signed distances; text comes from a
//! glyph atlas rasterized once with the same font code as the CPU renderer.
//!
//! Blending happens in the encoded color space, like the CPU renderer, so
//! both produce the same picture: use a non-sRGB target format (for example
//! `Bgra8Unorm`) — [`Renderer::pick_format`] chooses one.
//!
//! When nothing changed since the last frame, [`Renderer::changed`] says so
//! and the host can skip acquiring and presenting a surface texture at all.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use morfo::ui::LINE_HEIGHT;
use morfo::{Color, DrawCmd, DrawList, ImageId, Rect, TextStyle, Vec2};
use morfo_fonts::ab_glyph::{point, Font, GlyphId};
use morfo_fonts::{Face, FontSet};

pub use morfo_fonts::{FontMeasure, InvalidFont};
pub use wgpu;

const KIND_RECT: u32 = 0;
const KIND_SHADOW: u32 = 1;
const KIND_LINE: u32 = 2;
const KIND_GLYPH: u32 = 3;
const KIND_POLYGON: u32 = 4;
const KIND_IMAGE: u32 = 5;
const KIND_GRADIENT: u32 = 6;

/// Side of the square glyph atlas, in texels.
const ATLAS: u32 = 2048;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    quad: [f32; 4],
    shape: [f32; 4],
    fill: [f32; 4],
    stroke: [f32; 4],
    params: [f32; 4],
    uv: [f32; 4],
    kind: u32,
    _pad: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    size: [f32; 2],
    scale: f32,
    _pad: f32,
}

/// How far apart, in device pixels, the two passes of faked bold text are.
fn bold_offset(size: f32) -> f32 {
    (size * 0.045).clamp(0.5, 2.0)
}

/// Where a glyph lives in the atlas, and how to place it.
#[derive(Clone, Copy)]
struct Slot {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    left: i32,
    top: i32,
}

/// (glyph, size in 1/8 px, horizontal subpixel in 1/4 px, face)
type GlyphKey = (u16, u32, u8, Face);

/// What a frame was drawn with, to tell whether the next one differs.
#[derive(Default)]
struct Previous {
    cmds: Vec<DrawCmd>,
    text: String,
    points: Vec<Vec2>,
    target: Option<(u32, u32, u32, [u8; 4])>,
}

pub struct Renderer {
    fonts: Arc<FontSet>,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    atlas: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    globals: wgpu::Buffer,
    instances: wgpu::Buffer,
    points: wgpu::Buffer,
    glyphs: HashMap<GlyphKey, Option<Slot>>,
    /// Shelf packer state: next free spot and the current row's height.
    shelf: (u32, u32, u32),
    batch: Vec<Instance>,
    ranges: Vec<Batch>,
    /// The images uploaded.
    images: HashMap<ImageId, GpuImage>,
    image_layout: wgpu::BindGroupLayout,
    image_sampler: wgpu::Sampler,
    /// Bound when no image is: a single white texel.
    no_image: wgpu::BindGroup,
    prev: Previous,
}

impl Renderer {
    /// Creates a renderer drawing into targets of `format`. `font` is the
    /// bytes of a TrueType/OpenType font.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, font: Vec<u8>) -> Result<Renderer, InvalidFont> {
        Ok(Renderer::with_fonts(device, format, FontSet::new(font)?))
    }

    /// Creates a renderer drawing with every face of `fonts`.
    pub fn with_fonts(device: &wgpu::Device, format: wgpu::TextureFormat, fonts: FontSet) -> Renderer {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("morfo"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("morfo"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
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
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("morfo image"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("morfo"),
            bind_group_layouts: &[Some(&layout), Some(&image_layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![
            0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Uint32
        ];
        let premultiplied = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("morfo"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attributes,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState { color: premultiplied, alpha: premultiplied }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("morfo glyphs"),
            size: wgpu::Extent3d { width: ATLAS, height: ATLAS, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas.create_view(&Default::default());
        // Nearest: glyphs are placed on whole device pixels, one texel each.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let globals = buffer(device, "morfo globals", std::mem::size_of::<Globals>() as u64, wgpu::BufferUsages::UNIFORM);
        let instances = buffer(device, "morfo instances", 256 * std::mem::size_of::<Instance>() as u64, wgpu::BufferUsages::VERTEX);
        let points = buffer(device, "morfo points", 1024 * 8, wgpu::BufferUsages::STORAGE);
        let bind_group = bind(device, &layout, &globals, &atlas_view, &sampler, &points);
        // Images are scaled: they are sampled linearly, unlike glyphs.
        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("morfo images"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // Its texel is never read: only image instances sample, and they
        // always have their own image bound.
        let no_image = image_texture(device, &image_layout, &image_sampler, 1, 1).group;
        Renderer {
            fonts: Arc::new(fonts),
            pipeline,
            layout,
            bind_group,
            sampler,
            atlas,
            atlas_view,
            globals,
            instances,
            points,
            glyphs: HashMap::new(),
            shelf: (0, 0, 0),
            batch: Vec::new(),
            ranges: Vec::new(),
            images: HashMap::new(),
            image_layout,
            image_sampler,
            no_image,
            prev: Previous::default(),
        }
    }

    /// A target format among `available` that blends like the CPU renderer
    /// (not sRGB), or the first one.
    pub fn pick_format(available: &[wgpu::TextureFormat]) -> wgpu::TextureFormat {
        available.iter().copied().find(|f| !f.is_srgb()).unwrap_or(available[0])
    }

    /// Changes the fonts (adds a face, a fallback): `f` gets them.
    fn fonts_mut(&mut self, f: impl FnOnce(&mut FontSet) -> Result<(), InvalidFont>) -> Result<(), InvalidFont> {
        f(Arc::make_mut(&mut self.fonts))?;
        self.glyphs.clear();
        self.shelf = (0, 0, 0);
        self.prev.target = None;
        Ok(())
    }

    /// Font used for code. Without one, code is drawn in the main font.
    pub fn set_mono_font(&mut self, font: Vec<u8>) -> Result<(), InvalidFont> {
        self.fonts_mut(|f| f.set_mono(font))
    }

    /// Font for bold text. Without one, bold is drawn twice, a hair apart.
    pub fn set_bold_font(&mut self, font: Vec<u8>) -> Result<(), InvalidFont> {
        self.fonts_mut(|f| f.set_bold(font))
    }

    /// Font for italic text. Without one, italic stays upright.
    pub fn set_italic_font(&mut self, font: Vec<u8>) -> Result<(), InvalidFont> {
        self.fonts_mut(|f| f.set_italic(font))
    }

    /// A font for the characters the others lack (symbols, other scripts),
    /// tried after those added before it.
    pub fn add_fallback_font(&mut self, font: Vec<u8>) -> Result<(), InvalidFont> {
        self.fonts_mut(|f| f.add_fallback(font))
    }

    /// The fonts this renderer draws with.
    pub fn fonts(&self) -> &Arc<FontSet> {
        &self.fonts
    }

    /// Forgets the last frame: the next [`changed`](Self::changed) reports
    /// a change. Call it when the target lost its contents (resized,
    /// uncovered, recreated).
    pub fn invalidate(&mut self) {
        self.prev.target = None;
    }

    /// Text metrics that match what this renderer draws.
    pub fn measure(&self) -> FontMeasure {
        FontMeasure::new(self.fonts.clone())
    }

    /// Whether `list` would draw anything different from the last frame
    /// rendered. When it returns `false`, the host can skip the frame.
    pub fn changed(&self, list: &DrawList, size: (u32, u32), scale: f32, background: Color) -> bool {
        let p = &self.prev;
        p.target != Some((size.0, size.1, scale.to_bits(), background.to_rgba8()))
            || p.cmds.as_slice() != list.commands()
            || p.text != list.text_arena()
            || p.points.as_slice() != list.point_arena()
    }

    /// Draws `list` over `background` into `target`, whose size is `size`
    /// physical pixels; `scale` is the device pixel ratio.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        size: (u32, u32),
        scale: f32,
        background: Color,
        list: &DrawList,
    ) {
        self.upload_images(device, queue, list);
        self.prepare(queue, list, size, scale);

        // Upload: globals, instances and polygon points (growing buffers as needed).
        let logical = [size.0 as f32 / scale, size.1 as f32 / scale];
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&Globals { size: logical, scale, _pad: 0.0 }));
        let bytes: &[u8] = bytemuck::cast_slice(&self.batch);
        if bytes.len() as u64 > self.instances.size() {
            self.instances = buffer(device, "morfo instances", (bytes.len() as u64).next_power_of_two(), wgpu::BufferUsages::VERTEX);
        }
        queue.write_buffer(&self.instances, 0, bytes);
        let pts: Vec<[f32; 2]> = list.point_arena().iter().map(|p| [p.x, p.y]).collect();
        let pbytes: &[u8] = bytemuck::cast_slice(&pts);
        if pbytes.len() as u64 > self.points.size() {
            self.points = buffer(device, "morfo points", (pbytes.len() as u64).next_power_of_two(), wgpu::BufferUsages::STORAGE);
            self.bind_group = bind(device, &self.layout, &self.globals, &self.atlas_view, &self.sampler, &self.points);
        }
        if !pbytes.is_empty() {
            queue.write_buffer(&self.points, 0, pbytes);
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("morfo") });
        {
            let [r, g, b, a] = [background.r, background.g, background.b, background.a].map(|c| c.clamp(0.0, 1.0) as f64);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("morfo"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.instances.slice(..));
            let mut bound = None;
            pass.set_bind_group(1, &self.no_image, &[]);
            for (range, scissor, image) in &self.ranges {
                let [x, y, w, h] = scissor.unwrap_or([0, 0, size.0, size.1]);
                if w == 0 || h == 0 || range.is_empty() {
                    continue;
                }
                if let Some(id) = image.filter(|id| bound != Some(*id)) {
                    if let Some(gpu) = self.images.get(&id) {
                        pass.set_bind_group(1, &gpu.group, &[]);
                        bound = Some(id);
                    }
                }
                pass.set_scissor_rect(x, y, w, h);
                pass.draw(0..6, range.clone());
            }
        }
        queue.submit([encoder.finish()]);

        self.prev.cmds.clear();
        self.prev.cmds.extend_from_slice(list.commands());
        self.prev.text.clear();
        self.prev.text.push_str(list.text_arena());
        self.prev.points.clear();
        self.prev.points.extend_from_slice(list.point_arena());
        self.prev.target = Some((size.0, size.1, scale.to_bits(), background.to_rgba8()));
    }

    /// Uploads the images this frame shows that the GPU does not have yet
    /// (or has an older version of), and forgets those no longer registered.
    fn upload_images(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, list: &DrawList) {
        self.images.retain(|id, _| list.image(*id).is_some());
        for cmd in list.commands() {
            let DrawCmd::Image { image, version, .. } = *cmd else { continue };
            let Some(data) = list.image(image) else { continue };
            let fresh = self.images.get(&image).is_some_and(|g| g.version == version);
            if fresh {
                continue;
            }
            let same_size = self.images.get(&image).is_some_and(|g| g.size == (data.width, data.height));
            if !same_size {
                let gpu = image_texture(device, &self.image_layout, &self.image_sampler, data.width, data.height);
                self.images.insert(image, gpu);
            }
            let gpu = self.images.get_mut(&image).unwrap();
            gpu.version = version;
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &gpu.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &data.rgba,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(data.width * 4), rows_per_image: Some(data.height) },
                wgpu::Extent3d { width: data.width, height: data.height, depth_or_array_layers: 1 },
            );
        }
    }

    /// Turns the draw list into instances, split into ranges by clip and
    /// by the image bound.
    fn prepare(&mut self, queue: &wgpu::Queue, list: &DrawList, size: (u32, u32), scale: f32) {
        self.batch.clear();
        self.ranges.clear();
        let mut clips: Vec<Rect> = Vec::new();
        let mut start = 0u32;
        let mut bound: Option<ImageId> = None;
        let px = 1.0 / scale;
        let scissor = |clips: &[Rect]| {
            clips.last().map(|c| {
                let x0 = (c.x * scale).floor().clamp(0.0, size.0 as f32) as u32;
                let y0 = (c.y * scale).floor().clamp(0.0, size.1 as f32) as u32;
                let x1 = (c.right() * scale).ceil().clamp(0.0, size.0 as f32) as u32;
                let y1 = (c.bottom() * scale).ceil().clamp(0.0, size.1 as f32) as u32;
                [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)]
            })
        };
        for cmd in list.commands() {
            match *cmd {
                DrawCmd::PushClip { rect, .. } => {
                    let end = self.batch.len() as u32;
                    self.ranges.push((start..end, scissor(&clips), bound));
                    start = end;
                    let rect = clips.last().map_or(rect, |c| c.intersect(&rect));
                    clips.push(rect);
                }
                DrawCmd::PopClip => {
                    let end = self.batch.len() as u32;
                    self.ranges.push((start..end, scissor(&clips), bound));
                    start = end;
                    clips.pop();
                }
                DrawCmd::Nop => {}
                DrawCmd::Rect { rect, radius, fill, stroke, stroke_width } => self.batch.push(Instance {
                    quad: grow(rect, px),
                    shape: [rect.x, rect.y, rect.w, rect.h],
                    fill: rgba(fill),
                    stroke: rgba(stroke),
                    params: [radius, stroke_width, 0.0, 0.0],
                    kind: KIND_RECT,
                    ..Instance::zeroed()
                }),
                DrawCmd::Gradient { rect, radius, from, to, start, end } => self.batch.push(Instance {
                    quad: grow(rect, px),
                    shape: [rect.x, rect.y, rect.w, rect.h],
                    fill: rgba(start),
                    stroke: rgba(end),
                    params: [radius, 0.0, 0.0, 0.0],
                    uv: [from.x, from.y, to.x, to.y],
                    kind: KIND_GRADIENT,
                    ..Instance::zeroed()
                }),
                DrawCmd::Shadow { rect, radius, blur, offset, color } => {
                    let r = rect.translate(offset);
                    self.batch.push(Instance {
                        quad: grow(r, 0.65 * blur + px),
                        shape: [r.x, r.y, r.w, r.h],
                        fill: rgba(color),
                        params: [radius, blur, 0.0, 0.0],
                        kind: KIND_SHADOW,
                        ..Instance::zeroed()
                    })
                }
                DrawCmd::Line { from, to, width, color } => {
                    let half = width * 0.5;
                    let bounds = Rect::new(from.x.min(to.x), from.y.min(to.y), (from.x - to.x).abs(), (from.y - to.y).abs());
                    self.batch.push(Instance {
                        quad: grow(bounds, half + px),
                        shape: [from.x, from.y, to.x, to.y],
                        fill: rgba(color),
                        params: [half, 0.0, 0.0, 0.0],
                        kind: KIND_LINE,
                        ..Instance::zeroed()
                    })
                }
                DrawCmd::Polygon { start: first, len, color } => {
                    let pts = list.points(first, len);
                    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                    for p in pts {
                        (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
                    }
                    self.batch.push(Instance {
                        quad: grow(Rect::new(x0, y0, x1 - x0, y1 - y0), px),
                        shape: [first as f32, len as f32, 0.0, 0.0],
                        fill: rgba(color),
                        kind: KIND_POLYGON,
                        ..Instance::zeroed()
                    })
                }
                DrawCmd::Text { pos, size: font_size, color, start: s, len, style } => {
                    self.glyph_instances(queue, list.text(s, len), pos, font_size, color, scale, style);
                }
                DrawCmd::Image { rect, radius, image, uv, tint, .. } => {
                    if !self.images.contains_key(&image) {
                        continue;
                    }
                    // Another image: what came before is drawn with the old one.
                    if bound.is_some_and(|b| b != image) {
                        let end = self.batch.len() as u32;
                        self.ranges.push((start..end, scissor(&clips), bound));
                        start = end;
                    }
                    bound = Some(image);
                    self.batch.push(Instance {
                        quad: grow(rect, px),
                        shape: [rect.x, rect.y, rect.w, rect.h],
                        fill: rgba(tint),
                        params: [radius, 0.0, 0.0, 0.0],
                        uv: [uv.x, uv.y, uv.right(), uv.bottom()],
                        kind: KIND_IMAGE,
                        ..Instance::zeroed()
                    });
                }
                _ => {}
            }
        }
        let end = self.batch.len() as u32;
        self.ranges.push((start..end, scissor(&clips), bound));
    }

    /// Adds the glyphs of one line of text, rasterizing new ones into the atlas.
    #[allow(clippy::too_many_arguments)]
    fn glyph_instances(&mut self, queue: &wgpu::Queue, text: &str, pos: Vec2, size: f32, color: Color, scale: f32, style: TextStyle) {
        let fonts = self.fonts.clone();
        let device_size = size * scale;
        let (x, y) = (pos.x * scale, pos.y * scale);
        let baseline = fonts.baseline(style, y, device_size, LINE_HEIGHT);
        let (_, fake_bold) = fonts.pick(style);
        let passes: &[f32] = if fake_bold { &[0.0, 1.0] } else { &[0.0] };
        let size_key = (device_size * 8.0).round() as u32;
        let fill = rgba(color);
        let mut out = Vec::new();
        fonts.walk(text, device_size, style, x, |face, id, pen| {
            for &k in passes {
                let pen = pen + k * bold_offset(device_size);
                let sub = ((pen - pen.floor()) * 4.0) as u8;
                let key = (id.0, size_key, sub, face);
                if !self.glyphs.contains_key(&key) {
                    let slot = self.rasterize(queue, &fonts, face, id, size_key, sub);
                    self.glyphs.insert(key, slot);
                }
                if let Some(Some(s)) = self.glyphs.get(&key) {
                    let gx = (pen.floor() as i32 + s.left) as f32 / scale;
                    let gy = (baseline + s.top) as f32 / scale;
                    let a = ATLAS as f32;
                    out.push(Instance {
                        quad: [gx, gy, s.w as f32 / scale, s.h as f32 / scale],
                        fill,
                        uv: [s.x as f32 / a, s.y as f32 / a, (s.x + s.w) as f32 / a, (s.y + s.h) as f32 / a],
                        kind: KIND_GLYPH,
                        ..Instance::zeroed()
                    });
                }
            }
        });
        self.batch.extend(out);
    }

    /// Rasterizes one glyph (exactly as the CPU renderer does) into a free
    /// spot of the atlas. When the atlas is full it starts over.
    fn rasterize(&mut self, queue: &wgpu::Queue, fonts: &FontSet, face: Face, id: GlyphId, size_key: u32, sub: u8) -> Option<Slot> {
        let font = fonts.face(face);
        let px = fonts.px_scale(face, size_key as f32 / 8.0);
        let outlined = font.outline_glyph(id.with_scale_and_position(px, point(sub as f32 * 0.25, 0.0)))?;
        let bounds = outlined.px_bounds();
        let (w, h) = (bounds.width() as u32, bounds.height() as u32);
        if w == 0 || h == 0 || w > ATLAS || h > ATLAS {
            return None;
        }
        let mut coverage = vec![0u8; (w * h) as usize];
        outlined.draw(|gx, gy, c| {
            if let Some(p) = coverage.get_mut((gy * w + gx) as usize) {
                *p = (c.clamp(0.0, 1.0) * 255.0) as u8;
            }
        });
        // Shelf packing, with a texel of padding.
        let (mut x, mut y, mut row) = self.shelf;
        if x + w + 1 > ATLAS {
            (x, y, row) = (0, y + row + 1, 0);
        }
        if y + h + 1 > ATLAS {
            // Full: forget every glyph; they are rasterized again as needed.
            self.glyphs.clear();
            (x, y, row) = (0, 0, 0);
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &coverage,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.shelf = (x + w + 1, y, row.max(h));
        Some(Slot { x, y, w, h, left: bounds.min.x as i32, top: bounds.min.y as i32 })
    }
}

fn rgba(c: Color) -> [f32; 4] {
    [c.r, c.g, c.b, c.a]
}

/// `r` as `[x, y, w, h]`, grown by `d` on every side.
fn grow(r: Rect, d: f32) -> [f32; 4] {
    [r.x - d, r.y - d, r.w + 2.0 * d, r.h + 2.0 * d]
}

/// Instances drawn in one call: their range, the scissor of their clip
/// (`None`: the whole target) and the image bound.
type Batch = (Range<u32>, Option<[u32; 4]>, Option<ImageId>);

/// An image's texture on the GPU, and the bind group that shows it.
struct GpuImage {
    version: u32,
    size: (u32, u32),
    texture: wgpu::Texture,
    group: wgpu::BindGroup,
}

fn image_texture(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, sampler: &wgpu::Sampler, w: u32, h: u32) -> GpuImage {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("morfo image"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Not sRGB: colors are blended as the CPU renderer blends them.
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("morfo image"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    });
    GpuImage { version: u32::MAX, size: (w, h), texture, group }
}

fn buffer(device: &wgpu::Device, label: &str, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size.max(16),
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    atlas: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    points: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("morfo"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(atlas) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 3, resource: points.as_entire_binding() },
        ],
    })
}

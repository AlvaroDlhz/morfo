//! Painting frames into a window: with the GPU (wgpu) when there is one,
//! with the CPU (softbuffer) otherwise.

use std::num::NonZeroU32;
use std::sync::Arc;

use morfo::{TextMeasure, Ui};
use morfo_skia::tiny_skia::Pixmap;
use morfo_wgpu::wgpu;
use winit::window::Window;

/// The fonts frames are painted with (TrueType or OpenType bytes).
#[derive(Clone, Default)]
pub struct Fonts {
    /// The interface's font.
    pub main: Vec<u8>,
    /// The font for code; without one, code uses the main font.
    pub mono: Option<Vec<u8>>,
    /// Bold; without one, bold is drawn twice, a hair apart.
    pub bold: Option<Vec<u8>>,
    /// Italic; without one, italic text stays upright.
    pub italic: Option<Vec<u8>>,
    /// Bold italic; without one, the italic made bold.
    pub bold_italic: Option<Vec<u8>>,
    /// Fonts for the characters the others lack (symbols, other scripts),
    /// tried in order.
    pub fallbacks: Vec<Vec<u8>>,
}

impl Fonts {
    /// Just `main`.
    pub fn new(main: Vec<u8>) -> Fonts {
        Fonts { main, ..Fonts::default() }
    }

    /// The system's interface font with its bold and italic, a monospaced
    /// font and fallbacks for symbols and other scripts, found in the usual
    /// places. `MORFO_FONT` and `MORFO_MONO_FONT` name other fonts, and
    /// `MORFO_FALLBACK_FONTS` (paths separated by the system's path
    /// separator) adds fallbacks first, say a CJK font.
    pub fn system() -> Option<Fonts> {
        crate::system::fonts()
    }

    /// The font set these make.
    pub fn font_set(&self) -> Result<morfo_fonts::FontSet, morfo_fonts::InvalidFont> {
        let mut set = morfo_fonts::FontSet::new(self.main.clone())?;
        // A broken optional face is skipped, not fatal.
        if let Some(f) = &self.mono {
            let _ = set.set_mono(f.clone());
        }
        if let Some(f) = &self.bold {
            let _ = set.set_bold(f.clone());
        }
        if let Some(f) = &self.italic {
            let _ = set.set_italic(f.clone());
        }
        if let Some(f) = &self.bold_italic {
            let _ = set.set_bold_italic(f.clone());
        }
        for f in &self.fallbacks {
            let _ = set.add_fallback(f.clone());
        }
        Ok(set)
    }
}

/// Which renderer to paint with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backend {
    /// The GPU, falling back to the CPU when there is no usable one.
    #[default]
    Auto,
    /// Always the CPU.
    Cpu,
}

/// Where a window's frames are painted.
#[allow(clippy::large_enum_variant)]
pub enum Painter {
    Cpu {
        surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
        pixmap: Pixmap,
        renderer: morfo_skia::Renderer,
    },
    Gpu {
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        renderer: morfo_wgpu::Renderer,
    },
}

impl Painter {
    /// A painter for `window`: the GPU if `backend` allows it and there is
    /// one, the CPU otherwise.
    pub fn new(window: &Arc<Window>, backend: Backend, fonts: &Fonts) -> Result<Painter, String> {
        if backend == Backend::Auto {
            match Self::gpu(window, fonts) {
                Ok(p) => return Ok(p),
                Err(e) => eprintln!("morfo: no usable GPU ({e}); painting with the CPU"),
            }
        }
        let context = softbuffer::Context::new(window.clone()).map_err(|e| e.to_string())?;
        let mut surface = softbuffer::Surface::new(&context, window.clone()).map_err(|e| e.to_string())?;
        let size = window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        surface.resize(NonZeroU32::new(w).unwrap(), NonZeroU32::new(h).unwrap()).map_err(|e| e.to_string())?;
        let renderer = morfo_skia::Renderer::with_fonts(fonts.font_set().map_err(|e| e.to_string())?);
        Ok(Painter::Cpu { surface, pixmap: Pixmap::new(w, h).ok_or("empty window")?, renderer })
    }

    fn gpu(window: &Arc<Window>, fonts: &Fonts) -> Result<Painter, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(Box::new(window.clone())));
        let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;
        let options = wgpu::RequestAdapterOptions { compatible_surface: Some(&surface), ..Default::default() };
        let adapter = pollster::block_on(instance.request_adapter(&options)).map_err(|e| e.to_string())?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| e.to_string())?;
        let size = window.inner_size();
        let mut config = surface.get_default_config(&adapter, size.width.max(1), size.height.max(1)).ok_or("surface not supported")?;
        config.format = morfo_wgpu::Renderer::pick_format(&surface.get_capabilities(&adapter).formats);
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);
        let set = fonts.font_set().map_err(|e| e.to_string())?;
        let renderer = morfo_wgpu::Renderer::with_fonts(&device, config.format, set);
        Ok(Painter::Gpu { surface, device, queue, config, renderer })
    }

    /// `"GPU"` or `"CPU"`.
    pub fn name(&self) -> &'static str {
        match self {
            Painter::Cpu { .. } => "CPU",
            Painter::Gpu { .. } => "GPU",
        }
    }

    /// Text metrics that match what this painter draws.
    pub fn measure(&self) -> Box<dyn TextMeasure> {
        match self {
            Painter::Cpu { renderer, .. } => Box::new(renderer.measure()),
            Painter::Gpu { renderer, .. } => Box::new(renderer.measure()),
        }
    }

    /// The window lost its contents (resized, uncovered): repaint it all.
    pub fn invalidate(&mut self) {
        match self {
            Painter::Cpu { renderer, .. } => renderer.invalidate(),
            Painter::Gpu { renderer, .. } => renderer.invalidate(),
        }
    }

    /// Paints the frame `ui` just built, `w`×`h` device pixels at `scale`.
    /// Only what changed is repainted; an identical frame is not even
    /// presented.
    pub fn paint(&mut self, ui: &Ui, w: NonZeroU32, h: NonZeroU32, scale: f32) {
        match self {
            Painter::Cpu { surface, pixmap, renderer } => {
                if pixmap.width() != w.get() || pixmap.height() != h.get() {
                    let Some(p) = Pixmap::new(w.get(), h.get()) else { return };
                    *pixmap = p;
                    if surface.resize(w, h).is_err() {
                        return;
                    }
                }
                let damage = renderer.render(ui.draw_list(), pixmap, scale, ui.background());
                let Ok(mut buffer) = surface.buffer_mut() else { return };
                // Copy only the repainted rows when the window's buffer still
                // holds the previous frame.
                let rows = if buffer.age() == 1 { damage } else { Some((0, h.get())) };
                if let Some((first, end)) = rows {
                    let (from, to) = (first as usize * w.get() as usize, end as usize * w.get() as usize);
                    for (dst, src) in buffer[from..to].iter_mut().zip(pixmap.data().as_chunks::<4>().0[from..to].iter()) {
                        *dst = (src[0] as u32) << 16 | (src[1] as u32) << 8 | src[2] as u32;
                    }
                }
                let _ = buffer.present();
            }
            Painter::Gpu { surface, device, queue, config, renderer } => {
                let (w, h) = (w.get(), h.get());
                if config.width != w || config.height != h {
                    config.width = w;
                    config.height = h;
                    surface.configure(device, config);
                }
                if !renderer.changed(ui.draw_list(), (w, h), scale, ui.background()) {
                    return;
                }
                let (frame, stale) = match surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
                    wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
                    _ => {
                        // Lost, outdated or occluded: try again next frame.
                        surface.configure(device, config);
                        renderer.invalidate();
                        return;
                    }
                };
                let view = frame.texture.create_view(&Default::default());
                renderer.render(device, queue, &view, (w, h), scale, ui.background(), ui.draw_list());
                queue.present(frame);
                if stale {
                    surface.configure(device, config);
                    renderer.invalidate();
                }
            }
        }
    }
}

//! Draws the same interface with the CPU renderer and the GPU renderer and
//! compares the pictures. Skipped (with a note) when there is no GPU or no
//! system font. The two images are left in `target/` for inspection.

use morfo::{Color, Icon, Size, Theme, Ui, Variant, Vec2};

const SIZE: Vec2 = Vec2::new(640.0, 480.0);
const SCALE: f32 = 2.0;

fn font() -> Option<Vec<u8>> {
    [
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ]
    .iter()
    .find_map(|p| std::fs::read(p).ok())
}

fn scene(ui: &mut Ui, t: f64) {
    ui.begin(t, SIZE);
    ui.heading("morfo en la GPU");
    ui.card("card", |ui| {
        ui.row(|ui| {
            for (i, v) in Variant::ALL.into_iter().enumerate() {
                ui.button_with(&format!("b{i}"), "Botón", v, Size::Md);
            }
            ui.icon_button("star", Icon::Star, Variant::Soft, Size::Md);
        });
        ui.row(|ui| {
            let mut on = true;
            ui.toggle("t", &mut on);
            let mut v = 0.6;
            ui.slider("s", &mut v, 200.0);
            let mut stars = 3;
            ui.rating("r", &mut stars, 5);
        });
        ui.row(|ui| {
            let mut k = 0.7;
            ui.knob("k", "Perilla", &mut k);
            let mut c = true;
            ui.checkbox("c", "Casilla marcada", &mut c);
        });
        ui.scroll("list", 60.0, |ui| {
            for i in 0..8 {
                ui.label(&format!("Fila {i}"));
            }
        });
    });
    ui.end();
}

#[test]
fn gpu_matches_cpu() {
    let Some(font) = font() else {
        eprintln!("skipped: no system font");
        return;
    };
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    let Some(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    eprintln!("adapter: {:?}", adapter.get_info().name);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)).unwrap();

    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut gpu = morfo_wgpu::Renderer::new(&device, format, font.clone()).unwrap();
    let mut cpu = morfo_skia::Renderer::new(font).unwrap();
    let mut ui = Ui::new(Theme::dark());
    ui.set_text_measure(Box::new(gpu.measure()));
    let mut t = 0.0;
    for _ in 0..240 {
        t += 1.0 / 60.0;
        scene(&mut ui, t);
    }
    let (w, h) = ((SIZE.x * SCALE) as u32, (SIZE.y * SCALE) as u32);
    let background = ui.background();

    let mut pixmap = tiny_skia::Pixmap::new(w, h).unwrap();
    cpu.render(ui.draw_list(), &mut pixmap, SCALE, background);

    assert!(gpu.changed(ui.draw_list(), (w, h), SCALE, background));
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    gpu.render(&device, &queue, &texture.create_view(&Default::default()), (w, h), SCALE, background, ui.draw_list());
    assert!(!gpu.changed(ui.draw_list(), (w, h), SCALE, background), "an identical frame needs no redraw");

    // Read the GPU picture back.
    let row = (w * 4).div_ceil(256) * 256;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::ImageCopyBuffer { buffer: &readback, layout: wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    readback.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::Maintain::Wait);
    let data = readback.slice(..).get_mapped_range();
    let mut gpu_img = tiny_skia::Pixmap::new(w, h).unwrap();
    for y in 0..h as usize {
        let src = &data[y * row as usize..][..w as usize * 4];
        gpu_img.data_mut()[y * w as usize * 4..][..w as usize * 4].copy_from_slice(src);
    }

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    pixmap.save_png(out.join("morfo-compare-cpu.png")).unwrap();
    gpu_img.save_png(out.join("morfo-compare-gpu.png")).unwrap();

    let (mut total, mut far) = (0u64, 0usize);
    for (a, b) in pixmap.data().chunks(4).zip(gpu_img.data().chunks(4)) {
        let d = (0..3).map(|i| a[i].abs_diff(b[i]) as u64).max().unwrap();
        total += d;
        far += (d > 40) as usize;
    }
    let pixels = (w * h) as usize;
    let mean = total as f64 / pixels as f64;
    eprintln!("mean difference {mean:.3}/255, {far} pixels ({:.3}%) differ by more than 40", far as f64 * 100.0 / pixels as f64);
    // Same picture, up to edge antialiasing.
    assert!(mean < 1.0, "mean difference {mean}");
    assert!((far as f64) < pixels as f64 * 0.003, "{far} pixels differ a lot");
    let _ = Color::WHITE;
}

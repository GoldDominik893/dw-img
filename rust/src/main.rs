#![windows_subsystem = "windows"]

mod format;

use eframe::egui;
use egui::{Color32, Pos2, Rect, Sense, TextureHandle, TextureOptions, Vec2};
use image::{imageops, RgbImage};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct CreateForm {
    input_path: String,
    output_path: String,
    compress: bool,
    dither: bool,
    depth: u32,
    rotation: u32,
    flip_horizontal: bool,
    flip_vertical: bool,
    resize_width: String,
    resize_height: String,
}

impl CreateForm {
    fn new() -> Self {
        Self { depth: 16, ..Self::default() }
    }
}

struct ViewerState {
    source_path: PathBuf,
    decoded: format::DecodedImage,
    texture: TextureHandle,
    zoom: f32,
    scroll_offset: Vec2,
    fit_requested: bool,
}

struct DwxgApp {
    create_open: bool,
    create_form: CreateForm,
    viewer: Option<ViewerState>,
    status: String,
}

impl Default for DwxgApp {
    fn default() -> Self {
        Self {
            create_open: false,
            create_form: CreateForm::new(),
            viewer: None,
            status: "Ready. DWXG uses RGB565 (16-bit) or grayscale (8-bit).".into(),
        }
    }
}

impl DwxgApp {
    fn open_viewer(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("DWXG images", &["dwxg"])
            .pick_file() else { return; };

        match fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| format::decode_dwxg(&bytes)) {
            Ok(decoded) => {
                let color_image = egui::ColorImage::from_rgb(
                    [decoded.width as usize, decoded.height as usize],
                    &decoded.rgb,
                );
                let texture = ctx.load_texture(
                    format!("dwxg:{}", path.display()),
                    color_image,
                    TextureOptions::NEAREST,
                );
                self.status = format!("Opened {} ({} × {} px)", path.display(), decoded.width, decoded.height);
                self.viewer = Some(ViewerState {
                    source_path: path,
                    decoded,
                    texture,
                    zoom: 1.0,
                    scroll_offset: Vec2::ZERO,
                    fit_requested: true,
                });
            }
            Err(error) => self.status = format!("Could not open DWXG: {error}"),
        }
    }

    fn draw_create_window(&mut self, ui: &mut egui::Ui, close: &mut bool) {
        if ui.ctx().input(|input| input.viewport().close_requested()) {
            *close = true;
        }

        ui.heading("Create a DWXG image");
        ui.label("Choose a source image, output path, and conversion options.");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Input image");
            ui.add(egui::TextEdit::singleline(&mut self.create_form.input_path).desired_width(360.0));
            if ui.button("...").on_hover_text("Choose source image").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp", "tif", "tiff"])
                    .pick_file()
                {
                    self.create_form.input_path = path.display().to_string();
                    if self.create_form.output_path.trim().is_empty() {
                        let mut output = path.clone();
                        output.set_extension("dwxg");
                        self.create_form.output_path = output.display().to_string();
                    }
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Output file");
            ui.add(egui::TextEdit::singleline(&mut self.create_form.output_path).desired_width(360.0));
            if ui.button("...").on_hover_text("Choose output path").clicked() {
                let dialog = rfd::FileDialog::new()
                    .add_filter("DWXG images", &["dwxg"])
                    .set_file_name("image.dwxg");
                if let Some(path) = dialog.save_file() {
                    self.create_form.output_path = path.with_extension("dwxg").display().to_string();
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Colour depth");
            egui::ComboBox::from_id_salt("dwxg-colour-depth")
                .selected_text(if self.create_form.depth == 16 { "16-bit RGB565" } else { "8-bit grayscale" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.create_form.depth, 16, "16-bit RGB565");
                    ui.selectable_value(&mut self.create_form.depth, 8, "8-bit grayscale");
                });
        });

        ui.horizontal(|ui| {
            ui.label("Rotate");
            egui::ComboBox::from_id_salt("dwxg-rotation")
                .selected_text(format!("{}°", self.create_form.rotation))
                .show_ui(ui, |ui| {
                    for angle in [0u32, 90, 180, 270] {
                        ui.selectable_value(&mut self.create_form.rotation, angle, format!("{angle}°"));
                    }
                });
            ui.checkbox(&mut self.create_form.flip_horizontal, "Flip horizontal");
            ui.checkbox(&mut self.create_form.flip_vertical, "Flip vertical");
        });

        ui.horizontal(|ui| {
            ui.label("Resize (optional)");
            ui.add(egui::TextEdit::singleline(&mut self.create_form.resize_width).desired_width(65.0).hint_text("width"));
            ui.label("×");
            ui.add(egui::TextEdit::singleline(&mut self.create_form.resize_height).desired_width(65.0).hint_text("height"));
            ui.label("px; leave both empty to keep original size");
        });

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.create_form.compress, "Lossless zlib compression");
            let dither_enabled = self.create_form.depth == 16;
            ui.add_enabled_ui(dither_enabled, |ui| {
                ui.checkbox(&mut self.create_form.dither, "Floyd–Steinberg dithering");
            });
            if !dither_enabled {
                self.create_form.dither = false;
            }
        });

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("Create DWXG").strong()).clicked() {
                match self.create_file() {
                    Ok(path) => {
                        self.status = format!("Created {path}");
                        *close = true;
                    }
                    Err(error) => self.status = format!("Could not create DWXG: {error}"),
                }
            }
            if ui.button("Cancel").clicked() {
                *close = true;
            }
        });
        if self.status.starts_with("Could not create DWXG:") {
            ui.colored_label(Color32::LIGHT_RED, &self.status);
        }
    }

    fn create_file(&self) -> Result<String, String> {
        let input_text = self.create_form.input_path.trim();
        let output_text = self.create_form.output_path.trim();
        if input_text.is_empty() || output_text.is_empty() {
            return Err("Please select both an input image and an output file.".into());
        }
        let input_path = Path::new(input_text);
        let output_path = Path::new(output_text);
        let mut image: RgbImage = image::open(input_path)
            .map_err(|e| format!("Unable to load source image: {e}"))?
            .to_rgb8();

        image = match self.create_form.rotation {
            90 => imageops::rotate90(&image),
            180 => imageops::rotate180(&image),
            270 => imageops::rotate270(&image),
            _ => image,
        };
        if self.create_form.flip_horizontal {
            image = imageops::flip_horizontal(&image);
        }
        if self.create_form.flip_vertical {
            image = imageops::flip_vertical(&image);
        }

        let width_text = self.create_form.resize_width.trim();
        let height_text = self.create_form.resize_height.trim();
        match (width_text.is_empty(), height_text.is_empty()) {
            (true, true) => {}
            (false, false) => {
                let width: u32 = width_text.parse().map_err(|_| "Resize width must be a positive integer".to_string())?;
                let height: u32 = height_text.parse().map_err(|_| "Resize height must be a positive integer".to_string())?;
                if width == 0 || height == 0 || u64::from(width) * u64::from(height) > format::MAX_PIXELS {
                    return Err(format!("Resize dimensions must be positive and total no more than {} pixels", format::MAX_PIXELS));
                }
                image = imageops::resize(&image, width, height, imageops::FilterType::Lanczos3);
            }
            _ => return Err("Enter both resize dimensions, or leave both empty.".into()),
        }

        let bytes = format::encode_dwxg(
            image.width(), image.height(), image.as_raw(), self.create_form.depth,
            self.create_form.compress, self.create_form.dither,
        )?;
        fs::write(output_path, bytes).map_err(|e| format!("Unable to write output file: {e}"))?;
        Ok(output_path.display().to_string())
    }
}

impl eframe::App for DwxgApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("DWXG Studio");
            ui.label("Convert, inspect, and export images in the DWXG format.");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button(egui::RichText::new("＋  Create DWXG…").size(16.0)).clicked() {
                    self.create_open = true;
                }
                if ui.button(egui::RichText::new("▧  View DWXG…").size(16.0)).clicked() {
                    self.open_viewer(&ctx);
                }
            });
            ui.separator();
            ui.label("Features");
            ui.label("• 16-bit RGB565 or 8-bit grayscale output");
            ui.label("• Optional lossless compression and 16-bit dithering");
            ui.label("• Rotate, flip, and resize during conversion");
            ui.label("• Separate viewer window, zoom, scroll, and PNG export");
            ui.add_space(8.0);
            ui.label(egui::RichText::new(&self.status).color(Color32::LIGHT_BLUE));
        });

        if self.create_open {
            let mut close = false;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("dwxg_create_window"),
                egui::ViewportBuilder::default()
                    .with_title("Create DWXG")
                    .with_inner_size([760.0, 390.0])
                    .with_min_inner_size([620.0, 340.0]),
                |ui, _class| self.draw_create_window(ui, &mut close),
            );
            if close {
                self.create_open = false;
            }
        }

        let mut close_viewer_after_frame = false;
        if let Some(viewer) = self.viewer.as_mut() {
            let mut close = false;
            let title = format!("DWXG Viewer — {}", viewer.source_path.file_name().and_then(|s| s.to_str()).unwrap_or("image.dwxg"));
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("dwxg_viewer_window"),
                egui::ViewportBuilder::default()
                    .with_title(title)
                    .with_inner_size([900.0, 650.0])
                    .with_min_inner_size([420.0, 320.0]),
                |ui, _class| draw_viewer(ui, viewer, &mut close),
            );
            close_viewer_after_frame = close;
        }
        if close_viewer_after_frame {
            self.viewer = None;
        }
    }
}

fn draw_viewer(ui: &mut egui::Ui, viewer: &mut ViewerState, close: &mut bool) {
    if ui.ctx().input(|input| input.viewport().close_requested()) {
        *close = true;
    }

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            if ui.button("Fit").clicked() {
                viewer.fit_requested = true;
            }
            if ui.button("100%").clicked() {
                viewer.zoom = 1.0;
                viewer.scroll_offset = Vec2::ZERO;
            }
            ui.label("Zoom");
            ui.add(egui::Slider::new(&mut viewer.zoom, 0.1..=10.0).show_value(true).suffix("×"));
            if ui.button("Save as PNG…").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("PNG image", &["png"]).set_file_name("image.png").save_file() {
                    let image = RgbImage::from_raw(viewer.decoded.width, viewer.decoded.height, viewer.decoded.rgb.clone());
                    match image {
                        Some(image) => match image.save(&path) {
                            Ok(()) => ui.ctx().data_mut(|data| data.insert_temp(egui::Id::new("dwxg-export-note"), format!("Exported {}", path.display()))),
                            Err(error) => ui.ctx().data_mut(|data| data.insert_temp(egui::Id::new("dwxg-export-note"), format!("PNG export failed: {error}"))),
                        },
                        None => ui.ctx().data_mut(|data| data.insert_temp(egui::Id::new("dwxg-export-note"), "Could not construct the PNG image".to_string())),
                    };
                }
            }
        });
        let export_note = ui.ctx().data(|data| data.get_temp::<String>(egui::Id::new("dwxg-export-note")));
        if let Some(note) = export_note {
            ui.label(note);
        }
        ui.label(format!(
            "{} × {} px · {}-bit · {} · {} bytes",
            viewer.decoded.width,
            viewer.decoded.height,
            viewer.decoded.bits_per_pixel,
            if viewer.decoded.compressed { "zlib compressed" } else { "uncompressed" },
            fs::metadata(&viewer.source_path).map(|m| m.len()).unwrap_or(0),
        ));
        ui.separator();

        let available = ui.available_size();
        if viewer.fit_requested {
            let fit_x = (available.x - 20.0).max(1.0) / viewer.decoded.width as f32;
            let fit_y = (available.y - 20.0).max(1.0) / viewer.decoded.height as f32;
            viewer.zoom = fit_x.min(fit_y).min(1.0).clamp(0.05, 10.0);
            viewer.scroll_offset = Vec2::ZERO;
            viewer.fit_requested = false;
        }

        let zoom_before = viewer.zoom;
        let (mut zoom_after, mut anchor) = (zoom_before, None::<(Pos2, Pos2)>);
        let output = egui::ScrollArea::both()
            .id_salt("dwxg-image-scroll")
            .auto_shrink([false, false])
            .scroll_offset(viewer.scroll_offset)
            .show(ui, |content_ui| {
                let image_size = Vec2::new(
                    (viewer.decoded.width as f32 * zoom_before).max(1.0),
                    (viewer.decoded.height as f32 * zoom_before).max(1.0),
                );
                let available = content_ui.available_size();
                let content_size = Vec2::new(available.x.max(image_size.x), available.y.max(image_size.y));
                let (content_rect, _) = content_ui.allocate_exact_size(content_size, Sense::hover());
                let image_rect = Rect::from_center_size(content_rect.center(), image_size);
                content_ui.painter().image(
                    viewer.texture.id(),
                    image_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
                let response = content_ui.interact(image_rect, egui::Id::new("dwxg-image-pointer"), Sense::hover());
                if response.hovered() {
                    let wheel = content_ui.input(|input| input.smooth_scroll_delta.y);
                    if wheel.abs() > 0.0 {
                        zoom_after = (zoom_before * if wheel > 0.0 { 1.1 } else { 1.0 / 1.1 }).clamp(0.1, 10.0);
                        if let Some(pointer) = response.hover_pos() {
                            anchor = Some((pointer, image_rect.min));
                        }
                    }
                }
            });

        viewer.scroll_offset = output.state.offset;
        if (zoom_after - zoom_before).abs() > f32::EPSILON {
            if let Some((pointer, image_min)) = anchor {
                let scale = zoom_after / zoom_before;
                viewer.scroll_offset = (output.state.offset + (pointer - image_min) * (scale - 1.0)).max(Vec2::ZERO);
            }
            viewer.zoom = zoom_after;
            ui.ctx().request_repaint();
        }
    });
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DWXG Studio")
            .with_inner_size([620.0, 330.0])
            .with_min_inner_size([460.0, 260.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DWXG Studio",
        options,
        Box::new(|_cc| Ok(Box::new(DwxgApp::default()))),
    )
}

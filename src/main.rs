use std::error::Error;
use std::fmt;
use image::{ImageBuffer, Rgba};
use eframe::egui::{self, Vec2};
use serde::{Deserialize, Serialize};
use std::fs;

// Add Serialize/Deserialize to our existing structs
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
struct Color {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
struct Permutation {
    mapping: [(usize, usize); 4]
}

#[derive(Copy, Clone, Serialize, Deserialize)]
struct Pixel {
    color: Color,
    perm: Permutation,
}

#[derive(Clone, Serialize, Deserialize)]
struct Pattern {
    pixels: [[Pixel; 2]; 2]
}


impl Permutation {
    fn identity() -> Self {
        Permutation {
            mapping: [(0,0), (0,1), (1,0), (1,1)]
        }
    }
    
    fn rotate_90() -> Self {
        Permutation {
            mapping: [(0,1), (1,1), (0,0), (1,0)]
        }
    }
    
    fn rotate_270() -> Self {
        Permutation {
            mapping: [(1,0), (0,0), (1,1), (0,1)]
        }
    }
    
    fn flip_h() -> Self {
        Permutation {
            mapping: [(0,1), (0,0), (1,1), (1,0)]
        }
    }
    
    fn flip_v() -> Self {
        Permutation {
            mapping: [(1,0), (1,1), (0,0), (0,1)]
        }
    }
    
    fn compose(&self, other: &Permutation) -> Permutation {
        let mut result = [(0,0); 4];
        for i in 0..4 {
            let (y, x) = self.mapping[i];
            let idx = y * 2 + x;
            result[i] = other.mapping[idx];
        }
        Permutation { mapping: result }
    }
    
    fn apply<T: Copy>(&self, grid: [[T; 2]; 2]) -> [[T; 2]; 2] {
        let mut result = [[grid[0][0]; 2]; 2];
        for i in 0..4 {
            let (from_y, from_x) = (i / 2, i % 2);
            let (to_y, to_x) = self.mapping[i];
            result[to_y][to_x] = grid[from_y][from_x];
        }
        result
    }

    fn get_name(&self) -> &'static str {
        if self.mapping == Self::identity().mapping {
            "Identity"
        } else if self.mapping == Self::rotate_90().mapping {
            "Rotate 90°"
        } else if self.mapping == Self::rotate_270().mapping {
            "Rotate 270°"
        } else if self.mapping == Self::flip_h().mapping {
            "Flip H"
        } else if self.mapping == Self::flip_v().mapping {
            "Flip V"
        } else {
            "Custom"
        }
    }
}

impl Color {
    fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color { r, g, b, a }
    }

    fn lerp(&self, other: &Color, t: f32) -> Color {
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    fn to_rgba(&self) -> Rgba<u8> {
        Rgba([
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        ])
    }
}

fn create_base_pattern() -> Pattern {
    Pattern { pixels:
        [
            [
                Pixel {
                    color: Color::new(0.2, 0.4, 0.6, 1.0), // blue
                    perm: Permutation::rotate_90(),
                },
                Pixel {
                    color: Color::new(0.6, 0.4, 0.2, 1.0), // bronze
                    perm: Permutation::flip_h(),
                },
            ],
            [
                Pixel {
                    color: Color::new(0.0, 0.0, 0.0, 1.0), // black
                    perm: Permutation::flip_v(),
                },
                Pixel {
                    color: Color::new(0.0, 0.0, 0.0, 0.0), // transparent
                    perm: Permutation::identity(),
                },
            ],
        ],
    }
}

fn generate_fractal(iterations: u32, pattern: &Pattern, decay: f32) -> Vec<Vec<Color>> {
    let final_size = 1 << iterations;
    let mut result = vec![vec![Pixel {
        color: Color::new(0.0, 0.0, 0.0, 0.0),
        perm: Permutation::identity()
    }; final_size]; final_size];
    
    // Initialize with base pattern
    let base = pattern.pixels;
    for y in 0..2 {
        for x in 0..2 {
            result[y][x] = base[y][x];
        }
    }

    let mut blend = 1.0;
    let mut current_size = 2;
    
    while current_size < final_size {
        blend *= decay;
        let new_size = current_size * 2;

        for y in (0..current_size).rev() {
            for x in (0..current_size).rev() {
                let pixel = result[y][x];
                let alpha = pixel.color.a;
                let color = Color { a: 1.0, ..pixel.color };
                
                let y_start = y * 2;
                let x_start = x * 2;
                
                // Get base pattern and apply current permutation
                let base = pattern.pixels;
                let permuted_base = pixel.perm.apply(base);
                
                let blend_factor = 1.0 - (1.0 - blend) * alpha;
                
                // Place blended region with composed permutations
                for dy in 0..2 {
                    for dx in 0..2 {
                        let base_pixel = permuted_base[dy][dx];
                        let new_perm = if current_size * 2 < final_size {
                            pixel.perm.compose(&base_pixel.perm)
                        } else {
                            Permutation::identity()
                        };
                        
                        result[y_start + dy][x_start + dx] = Pixel {
                            color: color.lerp(&base_pixel.color, blend_factor),
                            perm: new_perm,
                        };
                    }
                }
            }
        }
        
        current_size = new_size;
    }

    // Extract final colors
    result.into_iter()
        .map(|row| row.into_iter().map(|pixel| pixel.color).collect())
        .collect()
}

#[derive(Debug)]
pub enum PatternError {
    FileError(std::io::Error),
    ParseError(serde_json::Error),
    ValidationError(String),
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PatternError::FileError(e) => write!(f, "File error: {}", e),
            PatternError::ParseError(e) => write!(f, "JSON parse error: {}", e),
            PatternError::ValidationError(msg) => write!(f, "Pattern validation error: {}", msg),
        }
    }
}

impl Error for PatternError {}

impl From<std::io::Error> for PatternError {
    fn from(err: std::io::Error) -> PatternError {
        PatternError::FileError(err)
    }
}

impl From<serde_json::Error> for PatternError {
    fn from(err: serde_json::Error) -> PatternError {
        PatternError::ParseError(err)
    }
}

fn validate_pattern(pattern: &Pattern) -> Result<(), PatternError> {
    // Validate color values are in range [0.0, 1.0]
    for row in &pattern.pixels {
        for pixel in row {
            let color = &pixel.color;
            if color.r < 0.0 || color.r > 1.0 ||
               color.g < 0.0 || color.g > 1.0 ||
               color.b < 0.0 || color.b > 1.0 ||
               color.a < 0.0 || color.a > 1.0 {
                return Err(PatternError::ValidationError(
                    "Color values must be between 0.0 and 1.0".to_string()
                ));
            }
        }
    }

    // Validate permutation mappings
    for row in &pattern.pixels {
        for pixel in row {
            let mut used_positions = [[false; 2]; 2];
            
            // Check each mapping in the permutation
            for &(y, x) in &pixel.perm.mapping {
                // Validate coordinates are in range
                if y >= 2 || x >= 2 {
                    return Err(PatternError::ValidationError(
                        "Permutation mapping coordinates must be less than 2".to_string()
                    ));
                }
                
                // Check for duplicate mappings
                if used_positions[y][x] {
                    return Err(PatternError::ValidationError(
                        "Permutation mapping contains duplicate positions".to_string()
                    ));
                }
                
                used_positions[y][x] = true;
            }
            
            // Verify all positions are used
            if !used_positions.iter().all(|row| row.iter().all(|&used| used)) {
                return Err(PatternError::ValidationError(
                    "Permutation mapping must use all positions".to_string()
                ));
            }
        }
    }

    Ok(())
}

fn load_pattern_from_file(path: &str) -> Result<Pattern, PatternError> {
    // Read and parse the JSON file
    let json = fs::read_to_string(path)?;
    let pattern: Pattern = serde_json::from_str(&json)?;
    
    // Validate the pattern
    validate_pattern(&pattern)?;
    
    Ok(pattern)
}

struct FractalApp {
    pattern: Pattern,
    preview_texture: Option<egui::TextureHandle>,
    iterations: u32,
    decay: f32,
    status_message: Option<(String, bool)>, // (message, is_error)
    status_timer: Option<f32>,
    pan_offset: egui::Vec2,
    zoom_level: f32,
    dragging: bool,
}

impl FractalApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            pattern: create_base_pattern(),
            preview_texture: None,
            iterations: 8,
            decay: 0.5,
            status_message: None,
            status_timer: None,
            pan_offset: egui::Vec2::ZERO,
            zoom_level: 1.0,
            dragging: false,
        }
    }
    
    fn save_pattern(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_title("Save Pattern")
            .save_file() {
                match serde_json::to_string_pretty(&self.pattern) {
                    Ok(json) => {
                        match fs::write(&path, json) {
                            Ok(_) => self.update_status(ctx, "Pattern saved successfully", false),
                            Err(e) => self.update_status(ctx, &format!("Failed to save pattern: {}", e), true),
                        }
                    }
                    Err(e) => self.update_status(ctx, &format!("Failed to serialize pattern: {}", e), true),
                }
        }
    }

    fn load_pattern(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_title("Load Pattern")
            .pick_file() {
                match load_pattern_from_file(path.to_str().unwrap_or_default()) {
                    Ok(pattern) => {
                        self.pattern = pattern;
                        self.update_status(ctx, "Pattern loaded successfully", false);
                        self.update_preview(ctx);
                    }
                    Err(e) => {
                        self.update_status(ctx, &format!("Failed to load pattern: {}", e), true);
                    }
                }
        }
    }

    fn reset_view(&mut self) {
        self.zoom_level = 1.0;
        self.pan_offset = egui::Vec2::ZERO;
    }

    fn fit_factor(&self, preview_rect: egui::Rect) -> f32 {
        if let Some(texture) = &self.preview_texture {
            return (preview_rect.size() / texture.size_vec2()).min_elem()
        }
        0.0
    }

    fn handle_zoom(&mut self, zoom_delta: f32, mouse_pos: egui::Pos2, preview_rect: egui::Rect) {
        if let Some(texture) = &self.preview_texture {
            let old_zoom = self.zoom_level;


            // Calculate new zoom level with bounds
            self.zoom_level = (self.zoom_level * (1.0 + zoom_delta * -0.1))
                .clamp(0.5, 20.0/self.fit_factor(preview_rect));
            
            // Calculate the texture size at both zoom levels
            let old_size = texture.size_vec2() * self.fit_factor(preview_rect) * old_zoom;
            let new_size = texture.size_vec2() * self.fit_factor(preview_rect) * self.zoom_level;

            // Calculate normalized mouse position relative to the preview rect
            let preview_size = preview_rect.size();
            let rel_mouse = (mouse_pos - self.pan_offset - preview_rect.min - preview_size / 2.0) / old_size;
            
            // Adjust pan offset to keep the point under cursor stable
            let size_diff = new_size - old_size;
            self.pan_offset -= size_diff * rel_mouse;
            
            // Clamp pan offset to keep image in view
            self.clamp_pan_offset(preview_rect);
        }
    }
    
    fn clamp_pan_offset(&mut self, preview_rect: egui::Rect) {
        if let Some(texture) = &self.preview_texture {
            let preview_size = preview_rect.size();
            let scaled_texture_size = texture.size_vec2() * self.fit_factor(preview_rect) * self.zoom_level;
            
            // Calculate the maximum allowed offset
            let max_offset = (scaled_texture_size - preview_size).abs().max(scaled_texture_size)/2.0;

            // Clamp the offset
            self.pan_offset = self.pan_offset.clamp(-max_offset, max_offset);
        }
    }

    fn update_preview(&mut self, ctx: &egui::Context) {
        let fractal = generate_fractal(self.iterations, &self.pattern, self.decay);
        let size = 1 << self.iterations;
        
        let mut image = image::RgbaImage::new(size as u32, size as u32);
        for (y, row) in fractal.iter().enumerate() {
            for (x, &color) in row.iter().enumerate() {
                image.put_pixel(x as u32, y as u32, color.to_rgba());
            }
        }

        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [size as _, size as _],
            &image.into_raw(),
        );

        let mut tex_options = egui::TextureOptions::default();
        tex_options.magnification = egui::TextureFilter::Nearest;

        self.preview_texture = Some(ctx.load_texture(
            "preview",
            color_image,
            tex_options,
        ));
    }

    fn export_preview(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .set_title("Export Preview")
            .save_file() {
                // Generate the fractal data
                let fractal = generate_fractal(self.iterations, &self.pattern, self.decay);
                let size = 1 << self.iterations;
                
                // Create the image
                let mut image = ImageBuffer::new(size as u32, size as u32);
                for (y, row) in fractal.iter().enumerate() {
                    for (x, &color) in row.iter().enumerate() {
                        image.put_pixel(x as u32, y as u32, color.to_rgba());
                    }
                }

                // Save the image
                match image.save(&path) {
                    Ok(_) => self.update_status(ctx, "Preview exported successfully", false),
                    Err(e) => self.update_status(ctx, &format!("Failed to export preview: {}", e), true),
                }
        }
    }

    fn update_preview_panel(&mut self, ui: &mut egui::Ui) {
        
        if self.preview_texture.is_none() {
            return;
        }
        let (preview_response, painter) = ui.allocate_painter(
            ui.available_size(),
            egui::Sense::drag()
        );
        let preview_rect = preview_response.rect;

        // Handle zooming with scroll wheel
        let zoom_delta = -ui.input(|i| i.smooth_scroll_delta.y / 50.0);
        if zoom_delta != 0.0 && preview_rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) {
            self.handle_zoom(
                zoom_delta,
                ui.input(|i| i.pointer.hover_pos().unwrap_or_default()),
                preview_rect
            );
        }

        // Handle panning
        if preview_response.dragged() {
            self.pan_offset += preview_response.drag_delta();
            self.dragging = true;
            self.clamp_pan_offset(preview_rect);
        } else {
            self.dragging = false;
        }

        // Get texture reference after all mutable operations
        let texture = self.preview_texture.as_ref().unwrap();
        let texture_size = texture.size_vec2();

        // Calculate display rect
        let size = texture_size * self.fit_factor(preview_rect) * self.zoom_level;
        let min_pos = preview_rect.min.to_vec2() + self.pan_offset + (preview_rect.size() - size) * 0.5;
        let rect = egui::Rect::from_min_size(
            min_pos.to_pos2(),
            size
        );

        // Draw the texture
        painter.image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::new(0.0, 0.0), egui::Pos2::new(1.0, 1.0)),
            egui::Color32::WHITE
        );
    }


    fn update_status(&mut self, _ctx: &egui::Context, message: &str, is_error: bool) {
        self.status_message = Some((message.to_string(), is_error));
        self.status_timer = Some(3.0); // Show message for 3 seconds
    }
}

impl eframe::App for FractalApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(timer) = &mut self.status_timer {
            *timer -= ctx.input(|i| i.unstable_dt).min(0.1);
            if *timer <= 0.0 {
                self.status_message = None;
                self.status_timer = None;
            }
        }
        egui::SidePanel::left("controls").show(ctx, |ui| {
            ui.heading("Pattern Controls");
            
            // Iteration control
            ui.add(egui::Slider::new(&mut self.iterations, 4..=11).text("Iterations"));
            ui.add(egui::Slider::new(&mut self.decay, 0.0..=1.0).text("Decay"));
            
            // Pattern editor
            ui.heading("Base Pattern");
            for y in 0..2 {
                for x in 0..2 {
                    ui.group(|ui| {
                        ui.label(format!("Pixel [{}, {}]", y, x));
                        let pixel = &mut self.pattern.pixels[y][x];
                        
                        // Color controls
                        let mut color = [pixel.color.r, pixel.color.g, pixel.color.b, pixel.color.a];
                        if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                            pixel.color.r = color[0];
                            pixel.color.g = color[1];
                            pixel.color.b = color[2];
                            pixel.color.a = color[3];
                        }
                        
                        // Permutation selector
                        let perm_options = ["Identity", "Rotate 90°", "Rotate 270°", "Flip H", "Flip V"];
                        ui.horizontal(|ui| {
                            ui.label("Permutation:");
                            ui.push_id(format!("perm_select_{}_{}", y, x), |ui| {
                                egui::ComboBox::from_label("")
                                    .selected_text(pixel.perm.get_name())
                                    .show_ui(ui, |ui| {
                                        for (idx, name) in perm_options.iter().enumerate() {
                                            if ui.selectable_label(
                                                pixel.perm.get_name() == *name,
                                                *name
                                            ).clicked() {
                                                pixel.perm = match idx {
                                                    0 => Permutation::identity(),
                                                    1 => Permutation::rotate_90(),
                                                    2 => Permutation::rotate_270(),
                                                    3 => Permutation::flip_h(),
                                                    4 => Permutation::flip_v(),
                                                    _ => Permutation::identity(),
                                                };
                                            }
                                        }
                                    });
                            });
                        });
                    });
                }
            }
            
            // Save/Load buttons
            ui.horizontal(|ui| {
                if ui.button("Save Pattern").clicked() {
                    self.save_pattern(ctx);
                }
                if ui.button("Load Pattern").clicked() {
                    self.load_pattern(ctx);
                }
            });

            ui.horizontal(|ui| {
                if ui.button("Update Preview").clicked() {
                    self.update_preview(ctx);
                }
                if ui.button("Reset View").clicked() {
                    self.reset_view();
                }
            });
            if ui.button("Export PNG").clicked() {
                self.export_preview(ui.ctx());
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                if let Some((message, is_error)) = &self.status_message {
                    let color = if *is_error {
                        egui::Color32::from_rgb(255, 0, 0)
                    } else {
                        egui::Color32::from_rgb(0, 255, 0)
                    };
                    ui.colored_label(color, message);
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.update_preview_panel(ui);
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Fractal Generator",
        options,
        Box::new(|cc| Ok(Box::new(FractalApp::new(cc))))
    )
}
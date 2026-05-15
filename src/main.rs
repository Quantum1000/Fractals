use std::error::Error;
use std::fmt;
use image::{ImageBuffer, Rgba};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::fs;

use frac_lang::normalizer::{self, NormalizedFile, NormalizeError};
use frac_lang::evaluator::{self, EvalConfig, RenderTree};

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
    
    fn rotate_180() -> Self {
        Permutation {
            mapping: [(1,1), (1,0), (0,1), (0,0)]
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
        } else if self.mapping == Self::rotate_180().mapping {
            "Rotate 180°"
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

    fn a_lerp(&self, other: &Color, t: f32) -> Color {
        let blend_factor = (1.0 - (1.0 - t) * self.a) * other.a;
        Color {
            r: self.r + (other.r - self.r) * blend_factor,
            g: self.g + (other.g - self.g) * blend_factor,
            b: self.b + (other.b - self.b) * blend_factor,
            a: self.a.max(other.a),
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
                let color = pixel.color;
                
                let y_start = y * 2;
                let x_start = x * 2;
                
                // Get base pattern and apply current permutation
                let base = pattern.pixels;
                let permuted_base = pixel.perm.apply(base);
                
                // Place blended region with composed permutations
                for dy in 0..2 {
                    for dx in 0..2 {
                        let base_pixel = permuted_base[dy][dx];
                        let new_perm = pixel.perm.compose(&base_pixel.perm);
                        result[y_start + dy][x_start + dx] = Pixel {
                            color: color.a_lerp(&base_pixel.color, blend),
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

fn old_generate_fractal(iterations: u32, pattern: &Pattern, decay: f32) -> Vec<Vec<Color>> {
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

#[derive(PartialEq)]
enum AppMode { Classic, Frac }

#[derive(PartialEq, Clone)]
enum FracStopMode { MaxDepth, SizeCutoff }

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
    active_mode: AppMode,
    frac_normalized: Option<NormalizedFile>,
    frac_errors: Vec<NormalizeError>,
    frac_preview_texture: Option<egui::TextureHandle>,
    frac_max_depth: u32,
    frac_output_size: u32,
    frac_stop_mode: FracStopMode,
    frac_min_size: f64,
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
            active_mode: AppMode::Classic,
            frac_normalized: None,
            frac_errors: Vec::new(),
            frac_preview_texture: None,
            frac_max_depth: 6,
            frac_output_size: 512,
            frac_stop_mode: FracStopMode::MaxDepth,
            frac_min_size: 2.0,
        }
    }
    
    fn save_pattern(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_title("Save Pattern")
            .set_directory("patterns")
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
            .set_directory("patterns")
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
        let fractal = old_generate_fractal(self.iterations, &self.pattern, self.decay);
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
            .set_directory("generations")
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


    fn load_frac_file(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Frac", &["frac"])
            .set_title("Load .frac File")
            .set_directory("patterns")
            .pick_file()
        {
            match fs::read_to_string(&path) {
                Err(e) => {
                    self.update_status(ctx, &format!("Failed to read file: {e}"), true);
                }
                Ok(src) => {
                    let (file, parse_errs) = frac_lang::parser::parse(&src);
                    if !parse_errs.is_empty() {
                        self.frac_errors = parse_errs.into_iter()
                            .map(|e| NormalizeError::ParseErrors(vec![e]))
                            .collect();
                        self.frac_normalized = None;
                        self.update_status(ctx, "Parse errors — see error panel", true);
                        return;
                    }
                    match normalizer::normalize(file) {
                        Ok(nf) => {
                            self.frac_errors.clear();
                            self.frac_normalized = Some(nf);
                            self.update_status(ctx, ".frac file loaded and normalized", false);
                            self.update_frac_preview(ctx);
                        }
                        Err(errs) => {
                            self.frac_errors = errs;
                            self.frac_normalized = None;
                            self.update_status(ctx, "Normalization errors — see error panel", true);
                        }
                    }
                }
            }
        }
    }

    fn export_frac_preview(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .set_title("Export Frac PNG")
            .set_directory("generations")
            .save_file()
        {
            let nf = match &self.frac_normalized {
                Some(nf) => nf,
                None => { self.update_status(ctx, "No .frac file loaded", true); return; }
            };

            let root_bbox = {
                use frac_lang::ast::Item;
                let root_name = nf.file.items.iter().find_map(|i| {
                    if let Item::Pattern(p) = &i.node { Some(p.root.0.node.clone()) } else { None }
                });
                root_name.and_then(|name| {
                    nf.file.items.iter().find_map(|i| {
                        if let Item::Tile(t) = &i.node {
                            if t.name.0.node == name { Some(t.canonical.iter()
                                .map(|p| [p.node.x, p.node.y])
                                .collect::<Vec<_>>()) }
                            else { None }
                        } else { None }
                    })
                })
            };

            let pixel_scale = root_bbox.as_deref().and_then(|pts| {
                if pts.is_empty() { return None; }
                let min_x = pts.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
                let max_x = pts.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
                let min_y = pts.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
                let max_y = pts.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
                let world_span = (max_x - min_x).max(max_y - min_y);
                if world_span <= 0.0 { None } else {
                    Some(self.frac_output_size as f64 * 0.98 / world_span)
                }
            }).unwrap_or(1.0);

            let min_size = match self.frac_stop_mode {
                FracStopMode::MaxDepth => None,
                FracStopMode::SizeCutoff => Some(self.frac_min_size / pixel_scale),
            };
            let max_depth = match self.frac_stop_mode {
                FracStopMode::MaxDepth => self.frac_max_depth,
                FracStopMode::SizeCutoff => u32::MAX,
            };
            let cfg = EvalConfig { max_depth, rng_seed: 0, min_size };
            let tree = evaluator::evaluate(nf, &cfg);

            let size = self.frac_output_size;
            let pixels = render_frac_tree(&tree, size as usize, root_bbox.as_deref());
            let mut image = ImageBuffer::new(size, size);
            for (y, row) in pixels.iter().enumerate() {
                for (x, &[r, g, b, a]) in row.iter().enumerate() {
                    image.put_pixel(x as u32, y as u32, Rgba([r, g, b, a]));
                }
            }
            match image.save(&path) {
                Ok(_) => self.update_status(ctx, "Frac PNG exported", false),
                Err(e) => self.update_status(ctx, &format!("Export failed: {}", e), true),
            }
        }
    }

    fn update_frac_preview(&mut self, ctx: &egui::Context) {
        let nf = match &self.frac_normalized {
            Some(nf) => nf,
            None => return,
        };

        // Find the root tile's canonical polygon to get the bounding box.
        let root_bbox = {
            use frac_lang::ast::{Item};
            let root_name = nf.file.items.iter().find_map(|i| {
                if let Item::Pattern(p) = &i.node { Some(p.root.0.node.clone()) } else { None }
            });
            root_name.and_then(|name| {
                nf.file.items.iter().find_map(|i| {
                    if let Item::Tile(t) = &i.node {
                        if t.name.0.node == name { Some(t.canonical.iter()
                            .map(|p| [p.node.x, p.node.y])
                            .collect::<Vec<_>>()) }
                        else { None }
                    } else { None }
                })
            })
        };

        // Convert pixel-space min_size to world-space by dividing by the
        // world-to-pixel scale factor used in render_frac_tree.
        let min_size = match self.frac_stop_mode {
            FracStopMode::MaxDepth => None,
            FracStopMode::SizeCutoff => {
                let pixel_scale = root_bbox.as_deref().and_then(|pts| {
                    if pts.is_empty() { return None; }
                    let min_x = pts.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
                    let max_x = pts.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
                    let min_y = pts.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
                    let max_y = pts.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
                    let world_span = (max_x - min_x).max(max_y - min_y);
                    if world_span <= 0.0 { None } else {
                        Some(self.frac_output_size as f64 * 0.98 / world_span)
                    }
                }).unwrap_or(1.0);
                Some(self.frac_min_size / pixel_scale)
            }
        };
        let max_depth = match self.frac_stop_mode {
            FracStopMode::MaxDepth => self.frac_max_depth,
            FracStopMode::SizeCutoff => u32::MAX,
        };
        let cfg = EvalConfig { max_depth, rng_seed: 0, min_size };
        let tree = evaluator::evaluate(nf, &cfg);

        let size = self.frac_output_size as usize;
        let pixels = render_frac_tree(&tree, size, root_bbox.as_deref());

        let raw: Vec<u8> = pixels.iter()
            .flat_map(|row| row.iter().flat_map(|&[r,g,b,a]| [r,g,b,a]))
            .collect();
        let color_image = egui::ColorImage::from_rgba_unmultiplied([size, size], &raw);
        let mut opts = egui::TextureOptions::default();
        opts.magnification = egui::TextureFilter::Nearest;
        self.frac_preview_texture = Some(ctx.load_texture("frac_preview", color_image, opts));
    }
}

fn point_in_convex_polygon(poly: &[[f32; 2]], px: f32, py: f32) -> bool {
    let n = poly.len();
    let mut sign = 0i32;
    for i in 0..n {
        let j = (i + 1) % n;
        let cross = (poly[j][0] - poly[i][0]) * (py - poly[i][1])
                  - (poly[j][1] - poly[i][1]) * (px - poly[i][0]);
        if cross > 1e-6 {
            if sign < 0 { return false; }
            sign = 1;
        } else if cross < -1e-6 {
            if sign > 0 { return false; }
            sign = -1;
        }
    }
    true
}

fn render_frac_tree(tree: &RenderTree, size: usize, root_poly: Option<&[[f64; 2]]>) -> Vec<Vec<[u8; 4]>> {
    let mut pixels = vec![vec![[0u8, 0u8, 0u8, 255u8]; size]; size];

    let bbox_pts: &[[f64; 2]] = match root_poly {
        Some(p) if !p.is_empty() => p,
        _ => return pixels,
    };
    let min_x = bbox_pts.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let min_y = bbox_pts.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    let max_x = bbox_pts.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
    let max_y = bbox_pts.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);

    let world_w = max_x - min_x;
    let world_h = max_y - min_y;
    if world_w <= 0.0 || world_h <= 0.0 { return pixels; }

    let scale = (size as f64 * 0.98) / world_w.max(world_h);
    let pad = size as f64 * 0.01;
    let off_x = pad - min_x * scale;
    let off_y = pad - min_y * scale;

    for tile in &tree.tiles {
        let poly: Vec<[f32; 2]> = tile.polygon.iter()
            .map(|&[x, y]| [(x * scale + off_x) as f32, (y * scale + off_y) as f32])
            .collect();

        let r = (tile.color[0].clamp(0.0, 1.0) * 255.0) as u8;
        let g = (tile.color[1].clamp(0.0, 1.0) * 255.0) as u8;
        let b = (tile.color[2].clamp(0.0, 1.0) * 255.0) as u8;
        let a = (tile.color[3].clamp(0.0, 1.0) * 255.0) as u8;

        let xs: Vec<f32> = poly.iter().map(|v| v[0]).collect();
        let ys: Vec<f32> = poly.iter().map(|v| v[1]).collect();
        let min_x = xs.iter().cloned().fold(f32::INFINITY, f32::min).max(0.0) as usize;
        let max_x = (xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max).ceil() as usize).min(size);
        let min_y = ys.iter().cloned().fold(f32::INFINITY, f32::min).max(0.0) as usize;
        let max_y = (ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max).ceil() as usize).min(size);

        let mut wrote = false;
        for py in min_y..max_y {
            for px in min_x..max_x {
                if point_in_convex_polygon(&poly, px as f32 + 0.5, py as f32 + 0.5) {
                    pixels[py][px] = [r, g, b, a];
                    wrote = true;
                }
            }
        }
        if !wrote {
            let cx = (xs.iter().sum::<f32>() / xs.len() as f32).floor() as usize;
            let cy = (ys.iter().sum::<f32>() / ys.len() as f32).floor() as usize;
            if cx < size && cy < size {
                pixels[cy][cx] = [r, g, b, a];
            }
        }
    }

    pixels
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

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_mode, AppMode::Classic, "Classic");
                ui.selectable_value(&mut self.active_mode, AppMode::Frac, "Frac");
            });
            ui.separator();

            if self.active_mode == AppMode::Frac {
                if ui.button("Load .frac File").clicked() {
                    self.load_frac_file(ctx);
                }
                ui.horizontal(|ui| {
                    ui.label("Stop by:");
                    ui.selectable_value(&mut self.frac_stop_mode, FracStopMode::MaxDepth, "Max Depth");
                    ui.selectable_value(&mut self.frac_stop_mode, FracStopMode::SizeCutoff, "Size Cutoff");
                });
                match self.frac_stop_mode {
                    FracStopMode::MaxDepth => {
                        ui.add(egui::Slider::new(&mut self.frac_max_depth, 1..=12).text("Max Depth"));
                    }
                    FracStopMode::SizeCutoff => {
                        ui.add(egui::Slider::new(&mut self.frac_min_size, 0.5..=64.0).text("Min Size (px)").logarithmic(true));
                    }
                }
                let sizes = [128u32, 256, 512, 1024];
                ui.horizontal(|ui| {
                    ui.label("Size:");
                    for &s in &sizes {
                        ui.selectable_value(&mut self.frac_output_size, s, s.to_string());
                    }
                });
                if ui.button("Update Preview").clicked() {
                    self.update_frac_preview(ctx);
                }
                if ui.button("Export PNG").clicked() {
                    self.export_frac_preview(ctx);
                }
                if !self.frac_errors.is_empty() {
                    ui.separator();
                    ui.colored_label(egui::Color32::RED, "Errors:");
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for e in &self.frac_errors {
                            ui.label(e.to_string());
                        }
                    });
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    if let Some((message, is_error)) = &self.status_message {
                        let color = if *is_error { egui::Color32::RED } else { egui::Color32::GREEN };
                        ui.colored_label(color, message);
                    }
                });
                return;
            }

            // Classic mode controls
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
                        let perm_options = ["Identity", "Rotate 90°", "Rotate 180°", "Rotate 270°", "Flip H", "Flip V"];
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
                                                    2 => Permutation::rotate_180(),
                                                    3 => Permutation::rotate_270(),
                                                    4 => Permutation::flip_h(),
                                                    5 => Permutation::flip_v(),
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
            let show_texture = |ui: &mut egui::Ui, texture: &egui::TextureHandle| {
                let available = ui.available_size();
                let tex_size = texture.size_vec2();
                let scale = (available / tex_size).min_elem();
                let display_size = tex_size * scale;
                ui.centered_and_justified(|ui| {
                    ui.image((texture.id(), display_size));
                });
            };
            match self.active_mode {
                AppMode::Frac => {
                    if let Some(texture) = &self.frac_preview_texture {
                        show_texture(ui, texture);
                    } else {
                        ui.centered_and_justified(|ui| { ui.label("Load a .frac file to preview"); });
                    }
                }
                AppMode::Classic => {
                    self.update_preview_panel(ui);
                }
            }
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



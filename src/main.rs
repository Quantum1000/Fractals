use image::{ImageBuffer, Rgba};

#[derive(Copy, Clone, Debug)]
struct Color {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

#[derive(Copy, Clone, Debug)]
struct Permutation {
    mapping: [(usize, usize); 4]
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

#[derive(Copy, Clone)]
struct Pixel {
    color: Color,
    perm: Permutation,
}

fn create_base_pattern() -> [[Pixel; 2]; 2] {
    [
        [
            Pixel {
                color: Color::new(0.2, 0.4, 0.6, 1.0),
                perm: Permutation::identity(),
            },
            Pixel {
                color: Color::new(0.0, 0.0, 0.0, 0.1),
                perm: Permutation::flip_h(),
            },
        ],
        [
            Pixel {
                color: Color::new(0.6, 0.4, 0.2, 1.0),
                perm: Permutation::rotate_270(),
            },
            Pixel {
                color: Color::new(0.0, 0.0, 0.0, 1.0),
                perm: Permutation::rotate_90(),
            },
        ],
    ]
}

fn generate_fractal(iterations: u32) -> Vec<Vec<Color>> {
    let final_size = 1 << iterations;
    let mut result = vec![vec![Pixel {
        color: Color::new(0.0, 0.0, 0.0, 0.0),
        perm: Permutation::identity()
    }; final_size]; final_size];
    
    // Initialize with base pattern
    let base = create_base_pattern();
    for y in 0..2 {
        for x in 0..2 {
            result[y][x] = base[y][x];
        }
    }

    let mut blend = 1.0;
    let mut current_size = 2;
    
    while current_size < final_size {
        blend *= 0.5;
        let new_size = current_size * 2;

        for y in (0..current_size).rev() {
            for x in (0..current_size).rev() {
                let pixel = result[y][x];
                let alpha = pixel.color.a;
                let color = Color { a: 1.0, ..pixel.color };
                
                let y_start = y * 2;
                let x_start = x * 2;
                
                // Get base pattern and apply current permutation
                let base = create_base_pattern();
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

fn main() {
    let iterations = 11;
    let fractal = generate_fractal(iterations);
    
    let size = 1 << iterations;
    let mut img = ImageBuffer::new(size as u32, size as u32);
    
    for (y, row) in fractal.iter().enumerate() {
        for (x, &color) in row.iter().enumerate() {
            img.put_pixel(x as u32, y as u32, color.to_rgba());
        }
    }

    img.save("fractal.png").unwrap();
}
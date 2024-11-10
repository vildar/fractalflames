#![allow(dead_code)]

use bincode;
use mpi::traits::*;
use plotters::prelude::*;
use rand::distributions::{Distribution, WeightedIndex};
use rand::Rng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
struct HistogramEntry {
    key: (i32, i32),
    value: ((f64, f64, f64), u32),
}

fn color_map(value: f64) -> (f64, f64, f64) {
    // Ensure the value is clamped between 0 and 1
    let value = value.clamp(0.0, 1.0);

    // Define the colors at the start and end of the range
    let start_color = (0.0, 0.0, 1.0); // Blue
    let end_color = (1.0, 0.0, 0.0); // Red

    // Interpolate between the start and end colors
    let r = start_color.0 + value * (end_color.0 - start_color.0);
    let g = start_color.1 + value * (end_color.1 - start_color.1);
    let b = start_color.2 + value * (end_color.2 - start_color.2);

    (r, g, b)
}

enum Variation {
    Linear,
    Sinusoidal,
    Spherical,
    Swirl,
    Horseshoe,
    Popcorn,
}

struct PostTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl PostTransform {
    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.b * y + self.c,
            self.d * x + self.e * y + self.f,
        )
    }
}

struct AffineTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
    weight: f64,
    variation: Variation,
    color: (f64, f64, f64),
}

impl AffineTransform {
    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let (x, y) = (
            self.a * x + self.b * y + self.c,
            self.d * x + self.e * y + self.f,
        );
        let r = (x * x + y * y).sqrt();

        let (x, y) = match self.variation {
            Variation::Linear => (x, y),
            Variation::Sinusoidal => (x.sin(), y.sin()),
            Variation::Spherical => (x / (r * r), y / (r * r)),
            Variation::Swirl => (x * r.sin() - y * r.cos(), x * r.cos() + y * r.sin()),
            Variation::Horseshoe => ((x - y) / r, (x + y) / r),
            Variation::Popcorn => (
                x + self.c * (3.0 * y).tan().sin(),
                y + self.f * (3.0 * x).tan().sin(),
            ),
        };
        (x, y)
    }
}

struct IFS {
    transforms: Vec<AffineTransform>,
}

impl IFS {
    fn chaos_game(&self, iterations: u32, rank: usize, size: usize) -> Vec<((f64, f64), usize)> {
        let mut rng = rand::thread_rng();
        let mut x = rng.gen_range(-1.0..1.0);
        let mut y = rng.gen_range(-1.0..1.0);
        let mut points = Vec::new();

        let weights: Vec<f64> = self.transforms.iter().map(|t| t.weight).collect();
        let dist = WeightedIndex::new(&weights).unwrap();

        // Each process will handle its portion of iterations
        let local_iterations = iterations / size as u32;
        let start_iteration = rank as u32 * local_iterations;

        for i in start_iteration..(start_iteration + local_iterations) {
            let transform_index = dist.sample(&mut rng);
            let transform = &self.transforms[transform_index];
            (x, y) = transform.apply(x, y);

            if i >= 20 {
                points.push(((x, y), transform_index));
            }
        }
        points
    }

    fn update_coord(
        &self,
        points: Vec<((f64, f64), usize)>,
        post_transform: &PostTransform,
    ) -> Vec<((f64, f64), usize)> {
        points
            .into_par_iter() // Parallelize with Rayon
            .map(|((x, y), index)| (post_transform.apply(x, y), index))
            .collect()
    }

    fn transform_to_pixels(
        &self,
        points: Vec<((f64, f64), usize)>,
        width: u32,
        height: u32,
    ) -> Vec<((i32, i32), usize)> {
        let min_x = points
            .iter()
            .map(|((x, _), _)| *x)
            .fold(f64::INFINITY, f64::min);
        let max_x = points
            .iter()
            .map(|((x, _), _)| *x)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = points
            .iter()
            .map(|((_, y), _)| *y)
            .fold(f64::INFINITY, f64::min);
        let max_y = points
            .iter()
            .map(|((_, y), _)| *y)
            .fold(f64::NEG_INFINITY, f64::max);

        points
            .into_par_iter() // Parallelize with Rayon
            .map(|((x, y), index)| {
                let pixel_x = ((x - min_x) / (max_x - min_x) * (width as f64)).round() as i32;
                let pixel_y = ((y - min_y) / (max_y - min_y) * (height as f64)).round() as i32;
                ((pixel_x, height as i32 - pixel_y), index)
            })
            .collect()
    }

    fn create_histogram(
        &self,
        pixel_points: &[((i32, i32), usize)],
    ) -> HashMap<(i32, i32), ((f64, f64, f64), u32)> {
        pixel_points
            .par_iter()
            .fold(HashMap::new, |mut local_histogram, &((x, y), index)| {
                let transform_color = self.transforms[index].color;
                let entry = local_histogram
                    .entry((x, y))
                    .or_insert((transform_color, 0));
                entry.1 += 1;

                if entry.1 > 1 {
                    entry.0 .0 = (entry.0 .0 + transform_color.0) / 2.0;
                    entry.0 .1 = (entry.0 .1 + transform_color.1) / 2.0;
                    entry.0 .2 = (entry.0 .2 + transform_color.2) / 2.0;
                }

                local_histogram
            })
            .reduce(
                || HashMap::new(),
                |mut acc, local_histogram| {
                    for (key, (color, alpha)) in local_histogram {
                        let entry = acc.entry(key).or_insert((color, 0));
                        entry.1 += alpha;

                        entry.0 .0 = (entry.0 .0 + color.0) / 2.0;
                        entry.0 .1 = (entry.0 .1 + color.1) / 2.0;
                        entry.0 .2 = (entry.0 .2 + color.2) / 2.0;
                    }
                    acc
                },
            )
    }
}

fn plot_points(
    histogram: HashMap<(i32, i32), ((f64, f64, f64), u32)>,
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let root =
        BitMapBackend::new("fractal_flames_colored_white.png", (width, height)).into_drawing_area();
    root.fill(&WHITE)?;

    let max_alpha = histogram
        .values()
        .map(|&(_, alpha)| alpha)
        .max()
        .unwrap_or(1) as f64;

    for (&(x, y), &((r, g, b), alpha)) in &histogram {
        let intensity = (alpha as f64).ln_1p() / (max_alpha.ln_1p());
        let color = RGBColor((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
        root.draw_pixel((x, y), &color.mix(intensity))?;
    }

    root.present()?;
    Ok(())
}

fn print_histogram(histogram: &HashMap<(i32, i32), ((f64, f64, f64), u32)>) {
    for ((x, y), ((r, g, b), alpha)) in histogram {
        println!(
            "Pixel ({}, {}): Color ({:.2}, {:.2}, {:.2}), Alpha: {}",
            x, y, r, g, b, alpha
        );
    }
}

fn main() {
    let universe = mpi::initialize().unwrap();
    let world = universe.world();
    let rank = world.rank();
    let size = world.size();

    let transform1 = AffineTransform {
        a: -0.870,
        b: -0.100,
        c: -0.930,
        d: -0.350,
        e: 0.500,
        f: -0.500,
        weight: 0.370,
        variation: Variation::Linear,
        color: color_map(0.1),
    };

    let transform2 = AffineTransform {
        a: 0.590,
        b: -0.620,
        c: -0.800,
        d: -0.110,
        e: 0.100,
        f: -0.900,
        weight: 0.570,
        variation: Variation::Linear,
        color: color_map(0.3),
    };

    let transform3 = AffineTransform {
        a: -0.056,
        b: 0.310,
        c: 0.920,
        d: 0.170,
        e: 0.000,
        f: -0.100,
        weight: 0.022,
        variation: Variation::Linear,
        color: color_map(0.5),
    };

    let transform4 = AffineTransform {
        a: 0.910,
        b: -0.190,
        c: 0.330,
        d: 0.240,
        e: -0.600,
        f: 0.900,
        weight: 0.058,
        variation: Variation::Linear,
        color: color_map(0.7),
    };

    let ifs = IFS {
        transforms: vec![transform1, transform2, transform3, transform4],
    };

    let points = ifs.chaos_game(1 << 27, rank as usize, size as usize);
    let min_x = points
        .iter()
        .map(|((x, _), _)| *x)
        .fold(f64::INFINITY, f64::min);
    let min_y = points
        .iter()
        .map(|((_, y), _)| *y)
        .fold(f64::INFINITY, f64::min);

    let post_transform = PostTransform {
        a: 1.0,
        b: 0.0,
        c: min_x.abs(),
        d: 0.0,
        e: 1.0,
        f: min_y.abs(),
    };

    let points = ifs.update_coord(points, &post_transform);

    let width = 1600;
    let height = 1200;
    let pixel_points = ifs.transform_to_pixels(points, width, height);

    let local_histogram = ifs.create_histogram(&pixel_points);

    // Serialize `local_histogram` to a Vec<u8>
    let local_histogram_data: Vec<HistogramEntry> = local_histogram
        .into_iter()
        .map(|(key, value)| HistogramEntry { key, value })
        .collect();
    let serialized_local_histogram = bincode::serialize(&local_histogram_data).unwrap();

    let local_size = serialized_local_histogram.len();
    println!(
        "Process {} local serialized histogram size: {}",
        rank, local_size
    );

    let mut sizes = vec![0usize; size as usize];
    world.all_gather_into(&local_size, &mut sizes[..]);

    if rank == 0 {
        println!("All sizes gathered: {:?}", sizes);
    }

    // Calculate total size
    let total_size: usize = sizes.iter().sum();
    // Gather all serialized data from processes
    // let mut serialized_global_histogram =
    //     vec![0u8; serialized_local_histogram.len() * size as usize];
    let mut serialized_global_histogram = vec![0u8; total_size];
    println!(
        "Process {} local serialized histogram size: {}",
        rank, local_size
    );
    println!("Total size to gather: {}", total_size);

    world.all_gather_into(
        &serialized_local_histogram[..],
        &mut serialized_global_histogram[..],
    );

    // world.all_gather_into(
    //     &serialized_local_histogram[..],
    //     &mut serialized_global_histogram[..],
    // );

    // Deserialize received data into `global_histogram`
    // let mut global_histogram = HashMap::new();
    // for chunk in serialized_global_histogram.chunks(serialized_local_histogram.len()) {
    //     let histogram_entries: Vec<HistogramEntry> = bincode::deserialize(chunk).unwrap();
    //     for entry in histogram_entries {
    //         global_histogram.entry(entry.key).or_insert(entry.value);
    //     }
    // }
    let mut global_histogram = HashMap::new();
    let mut offset = 0;

    for &size in &sizes {
        let chunk = &serialized_global_histogram[offset..offset + size];
        let histogram_entries: Vec<HistogramEntry> = bincode::deserialize(chunk).unwrap();
        for entry in histogram_entries {
            global_histogram.entry(entry.key).or_insert(entry.value);
        }
        offset += size;
    }
    if rank == 0 {
        println!("Sizes array: {:?}", sizes);
    }
    if rank == 0 {
        if let Err(e) = plot_points(global_histogram, width, height) {
            eprintln!("Error plotting points: {}", e);
        }
    }
}

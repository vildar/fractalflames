#![allow(dead_code)]

use plotters::prelude::*;
use rand::Rng;
use rand::distributions::{Distribution, WeightedIndex};

enum Variation {
    Linear,
    Sinusoidal,
    Spherical,
    Swirl,
    Horseshoe,
    Popcorn,
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
}

impl AffineTransform {
    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let (new_x, new_y) = (
            self.a * x + self.b * y + self.c,
            self.d * x + self.e * y + self.f,
        );
        let r = (new_x * new_x + new_y * new_y).sqrt();

        match self.variation {
            Variation::Linear => (new_x, new_y),
            Variation::Sinusoidal => (new_x.sin(), new_y.sin()),
            Variation::Spherical => (new_x / (r * r), new_y / (r * r)),
            Variation::Swirl => (
                new_x * r.sin() - new_y * r.cos(),
                new_x * r.cos() + new_y * r.sin(),
            ),
            Variation::Horseshoe => (
                (new_x - new_y) / r,
                (new_x + new_y) / r,
            ),
            Variation::Popcorn => (
                new_x + self.c * (3.0 * new_y).tan().sin(),
                new_y + self.f * (3.0 * new_x).tan().sin(),
            ),
        }
    }
}

struct IFS {
    transforms: Vec<AffineTransform>,
}

impl IFS {
    fn chaos_game(&self, iterations: u32) -> Vec<(f64, f64)> {
        let mut rng = rand::thread_rng();
        let mut x = rng.gen_range(-1.0..1.0);
        let mut y = rng.gen_range(-1.0..1.0);
        let mut points = Vec::new();

        let weights: Vec<f64> = self.transforms.iter().map(|t| t.weight).collect();
        let dist = WeightedIndex::new(&weights).unwrap();

        for i in 0..iterations {
            let transform = &self.transforms[dist.sample(&mut rng)];
            let (new_x, new_y) = transform.apply(x, y);
            x = new_x;
            y = new_y;

            if i >= 20 {
                points.push((x, y));
            }
        }

        points
    }
}

fn plot_points(points: Vec<(f64, f64)>, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new("fractal_flames.png", (width, height)).into_drawing_area();
    root.fill(&BLACK)?;

    let min_x = points.iter().map(|&(x, _)| x).fold(f64::INFINITY, f64::min);
    let max_x = points.iter().map(|&(x, _)| x).fold(f64::NEG_INFINITY, f64::max);
    let min_y = points.iter().map(|&(_, y)| y).fold(f64::INFINITY, f64::min);
    let max_y = points.iter().map(|&(_, y)| y).fold(f64::NEG_INFINITY, f64::max);

    let mut chart = ChartBuilder::on(&root)
        .build_cartesian_2d(min_x-0.5..max_x+0.5, min_y-0.5..max_y+0.5)?;

    chart.configure_mesh().disable_mesh().draw()?;

    for &(x, y) in &points {
        chart.draw_series(PointSeries::of_element(
            [(x, y)],
            1,
            &WHITE,
            &|c, s, st| {
                return EmptyElement::at(c) + Circle::new((0, 0), s, st.filled());
            },
        ))?;
    }

    root.present()?;
    Ok(())
}

fn main() {
    let transform1 = AffineTransform {
        a: -0.870,
        b: -0.100,
        c: -0.930,
        d: -0.350,
        e: 0.500,
        f: -0.500,
        weight: 0.370,
        variation: Variation::Linear,
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
    };

    let ifs = IFS {
        transforms: vec![transform1, transform2, transform3, transform4],
    };

    let points = ifs.chaos_game(1<<22);
    let width = 800;
    let height = 600;
    if let Err(e) = plot_points(points, width, height) {
        eprintln!("Error plotting points: {}", e);
    }
}

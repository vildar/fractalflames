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
            Variation::Swirl => (
                x * r.sin() - y * r.cos(),
                x * r.cos() + y * r.sin(),
            ),
            Variation::Horseshoe => (
                (x - y) / r,
                (x + y) / r,
            ),
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
    fn chaos_game(&self, iterations: u32) -> Vec<(f64, f64)> {
        let mut rng = rand::thread_rng();
        let mut x = rng.gen_range(-1.0..1.0);
        let mut y = rng.gen_range(-1.0..1.0);
        let mut points = Vec::new();

        let weights: Vec<f64> = self.transforms.iter().map(|t| t.weight).collect();
        let dist = WeightedIndex::new(&weights).unwrap();

        for i in 0..iterations {
            let transform = &self.transforms[dist.sample(&mut rng)];
            (x, y) = transform.apply(x, y);

            if i >= 20 {
                points.push((x, y));
            }
        }
        points
    }

    fn update_coord(&self, points: Vec<(f64, f64)>, post_transform: &PostTransform) -> Vec<(f64, f64)> {
        points.into_iter()
            .map(|(x, y)| post_transform.apply(x, y))
            .collect()
    }
}

fn plot_points(points: Vec<(f64, f64)>, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new("fractal_flames2.png", (width, height)).into_drawing_area();
    root.fill(&BLACK)?;

    //let min_x = points.iter().map(|(x, _)| *x).fold(f64::INFINITY, f64::min);
    let max_x = points.iter().map(|(x, _)| *x).fold(f64::NEG_INFINITY, f64::max);
    //let min_y = points.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
    let max_y = points.iter().map(|(_, y)| *y).fold(f64::NEG_INFINITY, f64::max);

    let mut chart = ChartBuilder::on(&root)
        .build_cartesian_2d(0.0..max_x, 0.0..max_y)?;

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
    let min_x = points.iter().map(|&(x, _)| x).fold(f64::INFINITY, f64::min);
    let min_y = points.iter().map(|&(_, y)| y).fold(f64::INFINITY, f64::min);

    let post_transform = PostTransform {
        a: 1.0,
        b: 0.0,
        c: min_x.abs(),
        d: 0.0,
        e: 1.0,
        f: min_y.abs(),
    };

    let points = ifs.update_coord(points, &post_transform);

    let width = 800;
    let height = 600;
    if let Err(e) = plot_points(points, width, height) {
        eprintln!("Error plotting points: {}", e);
    }
}

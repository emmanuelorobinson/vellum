// ported-from: visioncortex/vtracer cmdapp/src/converter.rs 8acb6bd911e160a55cd8a5503cfac54550a429f8
//! Colour tracing over `visioncortex`, adapted from vtracer 0.6.5 (MIT; `notices/vtracer-MIT.txt`).
//!
//! Kept: vtracer's colour mode with stacked layers, its default thresholds and its SVG output. Changed: the
//! key colour for transparent pixels is chosen deterministically (vtracer draws random candidates), and
//! the binary mode, presets, cutout layering and file I/O are left out, because only the colour path is
//! used. Depending on `visioncortex` directly drops vtracer's command-line dependencies (clap 2, image 0.23).
use std::fmt::Write;
use visioncortex::color_clusters::{HIERARCHICAL_MAX, KeyingAction, Runner, RunnerConfig};
use visioncortex::{Color, ColorImage, PathSimplifyMode, PointF64};

/// vtracer's defaults (`Config::default`), with speckle filtering turned off as Vellum used it.
const FILTER_SPECKLE: usize = 0;
const COLOR_PRECISION: i32 = 6;
const LAYER_DIFFERENCE: i32 = 16;
const CORNER_THRESHOLD_DEG: i32 = 60;
const LENGTH_THRESHOLD: f64 = 4.0;
const SPLICE_THRESHOLD_DEG: i32 = 45;
const MAX_ITERATIONS: usize = 10;
const PATH_PRECISION: u32 = 2;
/// A share of transparent pixels on five scanlines above which transparency is keyed out.
const KEYING_THRESHOLD: f32 = 0.2;

fn rad(deg: i32) -> f64 {
    f64::from(deg) / 180.0 * std::f64::consts::PI
}

fn has_colour(img: &ColorImage, c: Color) -> bool {
    (0..img.height).any(|y| {
        (0..img.width).any(|x| {
            let p = img.get_pixel(x, y);
            p.r == c.r && p.g == c.g && p.b == c.b
        })
    })
}

/// The first colour the image does not use: six saturated colours, then a fixed walk over the cube.
fn unused_colour(img: &ColorImage) -> Option<Color> {
    let special = [
        (255, 0, 0),
        (0, 255, 0),
        (0, 0, 255),
        (255, 255, 0),
        (0, 255, 255),
        (255, 0, 255),
    ];
    let walk = (1u32..4096).map(|i| {
        let i = i.wrapping_mul(2_654_435_761);
        ((i >> 24) as u8, (i >> 16) as u8, (i >> 8) as u8)
    });
    special
        .into_iter()
        .chain(walk)
        .map(|(r, g, b)| Color::new(r, g, b))
        .find(|c| !has_colour(img, *c))
}

fn should_key(img: &ColorImage) -> bool {
    if img.width == 0 || img.height == 0 {
        return false;
    }
    let threshold = ((img.width * 2) as f32 * KEYING_THRESHOLD) as usize;
    let mut transparent = 0;
    for y in [
        0,
        img.height / 4,
        img.height / 2,
        3 * img.height / 4,
        img.height - 1,
    ] {
        for x in 0..img.width {
            if img.get_pixel(x, y).a == 0 {
                transparent += 1;
            }
            if transparent >= threshold {
                return true;
            }
        }
    }
    false
}

/// Traces RGBA pixels into an SVG document, one filled path per colour cluster, back to front.
pub(crate) fn trace(mut img: ColorImage) -> Result<String, ()> {
    let (width, height) = (img.width, img.height);
    let key_color = if should_key(&img) {
        let key = unused_colour(&img).ok_or(())?;
        for y in 0..height {
            for x in 0..width {
                if img.get_pixel(x, y).a == 0 {
                    img.set_pixel(x, y, &key);
                }
            }
        }
        key
    } else {
        // All zeroes: visioncortex applies no keying.
        Color::default()
    };
    let clusters = Runner::new(
        RunnerConfig {
            diagonal: LAYER_DIFFERENCE == 0,
            hierarchical: HIERARCHICAL_MAX,
            batch_size: 25600,
            good_min_area: FILTER_SPECKLE * FILTER_SPECKLE,
            good_max_area: width * height,
            is_same_color_a: 8 - COLOR_PRECISION,
            is_same_color_b: 1,
            deepen_diff: LAYER_DIFFERENCE,
            hollow_neighbours: 1,
            key_color,
            keying_action: KeyingAction::Discard,
        },
        img,
    )
    .run();
    let view = clusters.view();
    let mut svg = String::new();
    let _ = writeln!(svg, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    let _ = writeln!(
        svg,
        r#"<svg version="1.1" xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}">"#
    );
    for &index in view.clusters_output.iter().rev() {
        let cluster = view.get_cluster(index);
        let path = cluster.to_compound_path(
            &view,
            false,
            PathSimplifyMode::Spline,
            rad(CORNER_THRESHOLD_DEG),
            LENGTH_THRESHOLD,
            MAX_ITERATIONS,
            rad(SPLICE_THRESHOLD_DEG),
        );
        let (d, offset) = path.to_svg_string(true, PointF64::default(), Some(PATH_PRECISION));
        let _ = writeln!(
            svg,
            r#"<path d="{d}" fill="{}" transform="translate({},{})"/>"#,
            cluster.residue_color().to_hex_string(),
            offset.x,
            offset.y
        );
    }
    svg.push_str("</svg>\n");
    Ok(svg)
}

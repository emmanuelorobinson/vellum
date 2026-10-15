//! TEST DATA: checked recording is explicit; ordinary runs compare the committed native golden corpus.
#[path = "support/render_scenes.rs"]
mod render_scenes;
use std::{fs, path::Path};
use vellum_render_skia::{Page, Renderer, Resources};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let record = std::env::args().any(|a| a == "--record");
    let root = Path::new("fixtures/rendering");
    let out = Path::new("target/renderer-goldens");
    fs::create_dir_all(out)?;
    let renderer = Renderer::new()?;
    let mut names = vec![];
    for (name, list) in render_scenes::scenes() {
        let bytes = list.to_bytes()?;
        let scene = renderer.prepare_buffer(&bytes, &Resources::default())?;
        let page = Page {
            width: 256,
            height: 256,
            scene: &scene,
        };
        let rgba = renderer.rgba(&page)?;
        fs::write(out.join(format!("{name}.bin")), &bytes)?;
        fs::write(out.join(format!("{name}.rgba")), &rgba)?;
        fs::write(out.join(format!("{name}.png")), renderer.png(&page)?)?;
        fs::write(
            out.join(format!("{name}.basic.json")),
            renderer.basic_plan(&bytes, &Resources::default())?,
        )?;
        let basic = renderer.prepare_basic(&bytes, &Resources::default())?;
        fs::write(
            out.join(format!("{name}.basic-native.rgba")),
            renderer.rgba(&Page {
                width: 256,
                height: 256,
                scene: &basic,
            })?,
        )?;
        if record {
            fs::write(root.join(format!("{name}.rgba")), &rgba)?;
        } else {
            let expected = fs::read(root.join(format!("{name}.rgba")))?;
            let different = rgba
                .as_chunks::<4>()
                .0
                .iter()
                .zip(expected.as_chunks::<4>().0.iter())
                .filter(|(a, b)| pixel_delta(a, b) > 2)
                .count();
            if rgba.len() != expected.len() || different > 0 {
                return Err(format!(
                    "native golden {name}: {different} pixels differ; artifacts in {}",
                    out.display()
                )
                .into());
            }
        }
        names.push(name);
    }
    let legacy = Path::new("target/renderer-fixtures");
    let name = "text-images-strokes";
    let rgba = fs::read(legacy.join("native.rgba"))?;
    if record {
        fs::write(root.join(format!("{name}.rgba")), &rgba)?;
    } else {
        let expected = fs::read(root.join(format!("{name}.rgba")))?;
        if expected.len() != rgba.len()
            || expected
                .as_chunks::<4>()
                .0
                .iter()
                .zip(rgba.as_chunks::<4>().0.iter())
                .any(|(a, b)| pixel_delta(a, b) > 2)
        {
            return Err("text/image native golden changed".into());
        }
    }
    fs::write(out.join(format!("{name}.rgba")), rgba)?;
    for (from, to) in [
        ("scene.bin", "text-images-strokes.bin"),
        ("native.png", "text-images-strokes.png"),
        ("basic-native.rgba", "text-images-strokes.basic-native.rgba"),
        ("image.png", "image.png"),
        ("font-0.ttf", "font-0.ttf"),
        ("font-1.ttf", "font-1.ttf"),
    ] {
        fs::copy(legacy.join(from), out.join(to))?;
    }
    names.push(name.into());
    for i in 0..60 {
        let pan = i as f32 * 0.3;
        let zoom = 0.4 + i as f32 * 0.001;
        fs::write(
            out.join(format!("performance-{i}.bin")),
            render_scenes::many_nodes(5000, pan, zoom).to_bytes()?,
        )?;
    }
    for i in 0..30 {
        let mut l = render_scenes::many_nodes(5000, 0., 1.);
        l.commands[0] =
            vellum_types::display::DrawCmd::Transform([1., 0., 0., 1., i as f32 * 0.5, 0.]);
        fs::write(out.join(format!("drag-{i}.bin")), l.to_bytes()?)?;
    }
    for i in 0..30 {
        fs::write(
            out.join(format!("batch200-{i}.bin")),
            render_scenes::many_nodes(200, i as f32 * 0.1, 1.).to_bytes()?,
        )?;
    }
    fs::write(
        out.join("manifest.json"),
        format!(
            "[{}]",
            names
                .iter()
                .map(|n| format!("\"{n}\""))
                .collect::<Vec<_>>()
                .join(",")
        ),
    )?;
    Ok(())
}

fn pixel_delta(a: &[u8; 4], b: &[u8; 4]) -> u8 {
    let premul = |p: &[u8; 4], c: usize| ((u16::from(p[c]) * u16::from(p[3]) + 127) / 255) as u8;
    (0..3)
        .map(|c| premul(a, c).abs_diff(premul(b, c)))
        .max()
        .unwrap_or(0)
        .max(a[3].abs_diff(b[3]))
}

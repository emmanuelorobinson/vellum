//! The native path: the same display list and the same skia-safe drawing code as Renderer B, on a
//! CPU raster surface.
//!
//!   skia-native fixtures <out-dir>                       fixture-NN.native.png for all 20 fixtures
//!   skia-native document <nodes> <out.png> <font.ttf>    a generated document at 1280 x 800
//!   skia-native text-fixtures <fonts-dir> <out-dir>      text-NN.native.png (Parley layout, the path
//!       every renderer shares) and text-NN.paragraph.png (Skia's SkParagraph laying out the same text),
//!       plus text-layout.json comparing the two layouts' line counts
//!   skia-native compare <results.json> <artifact-dir> <channel> <fraction> <name>=<expected.png>,<actual.png>...
//!       pixel-diffs each pair with vellum-testkit, writes diff artifacts and a JSON summary;
//!       exits 1 if any pair fails the tolerance

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use skia_safe::textlayout::{
    FontCollection, ParagraphBuilder, ParagraphStyle, TextAlign as SkAlign, TextDirection,
    TextStyle, TypefaceFontProvider,
};
use skia_safe::{
    AlphaType, Canvas, Color, Color4f, ColorType, ImageInfo, Paint, Typeface, surfaces,
};
use spike_scene::{
    CANVAS_BACKGROUND, DisplayList, Document, FIXTURE_COUNT, FIXTURE_SIZE, FONT_FILES,
    TEXT_FIXTURE_COUNT, TextAlign, TextEngine, fixture, generate, text_fixture, text_fixture_spec,
};
use spike_skia_draw::{SkiaScene, make_image, make_typeface};
use vellum_testkit::pixel_diff::{PixelDiffError, Rgba8Image, Tolerance, assert_images_match};

fn render(doc: &Document, width: i32, height: i32, font: Option<&[u8]>) -> Rgba8Image {
    render_with(doc, width, height, font, Vec::new())
}

fn render_with(
    doc: &Document,
    width: i32,
    height: i32,
    font: Option<&[u8]>,
    glyph_typefaces: Vec<Typeface>,
) -> Rgba8Image {
    let images = doc.images.iter().map(make_image).collect();
    let typeface = font.and_then(make_typeface);
    let mut scene = SkiaScene::new(DisplayList::build(doc), images, typeface)
        .with_glyph_typefaces(glyph_typefaces);
    paint_to_image(width, height, |canvas| scene.draw(canvas))
}

fn paint_to_image(width: i32, height: i32, draw: impl FnOnce(&Canvas)) -> Rgba8Image {
    let mut surface = surfaces::raster_n32_premul((width, height)).expect("raster surface");
    let canvas = surface.canvas();
    let [r, g, b] = CANVAS_BACKGROUND;
    canvas.clear(Color::from_rgb(r, g, b));
    draw(canvas);
    let info = ImageInfo::new(
        (width, height),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    assert!(
        surface.read_pixels(&info, &mut rgba, width as usize * 4, (0, 0)),
        "read pixels"
    );
    Rgba8Image::new(width as u32, height as u32, rgba).expect("buffer size")
}

fn fixtures(out: &Path) -> Result<(), String> {
    for n in 1..=FIXTURE_COUNT {
        let size = spike_scene::FIXTURE_SIZE as i32;
        let image = render(&fixture(n), size, size, None);
        image
            .write_png(&out.join(format!("fixture-{n:02}.native.png")))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn read_fonts(dir: &Path) -> Result<Vec<Vec<u8>>, String> {
    FONT_FILES
        .iter()
        .map(|f| {
            std::fs::read(dir.join(f))
                .map_err(|e| format!("{f}: {e}; run node scripts/fetch-fonts.mjs"))
        })
        .collect()
}

/// True when the first strong character is right-to-left (Hebrew or Arabic), as Parley decides the
/// base direction; SkParagraph needs it set explicitly.
fn starts_rtl(text: &str) -> bool {
    text.chars()
        .find(|c| c.is_alphabetic())
        .is_some_and(|c| ('\u{0590}'..='\u{08FF}').contains(&c))
}

fn text_fixtures(fonts_dir: &Path, out: &Path) -> Result<(), String> {
    let fonts = read_fonts(fonts_dir)?;
    skia_safe::icu::init();
    let typefaces: Vec<Typeface> = fonts
        .iter()
        .zip(FONT_FILES)
        .map(|(f, name)| make_typeface(f).ok_or(format!("{name}: Skia cannot read it")))
        .collect::<Result<_, _>>()?;
    let mut engine = TextEngine::new(&fonts);
    let mut provider = TypefaceFontProvider::new();
    for (typeface, family) in typefaces.iter().zip(engine.families().to_vec()) {
        provider.register_typeface(typeface.clone(), Some(family.as_str()));
    }
    let mut collection = FontCollection::new();
    // As the asset font manager: with the default-font-manager slot every paragraph came out empty.
    collection.set_asset_font_manager(Some(provider.into()));
    collection.disable_font_fallback();
    let families: Vec<String> = engine.families().to_vec();
    let size = FIXTURE_SIZE as i32;
    let mut json = String::from("[\n");
    for n in 1..=TEXT_FIXTURE_COUNT {
        let spec = text_fixture_spec(n);
        let doc = text_fixture(n, &mut engine);
        let parley_lines = engine.last_line_count();
        render_with(&doc, size, size, None, typefaces.clone())
            .write_png(&out.join(format!("text-{n:02}.native.png")))
            .map_err(|e| e.to_string())?;

        let mut style = ParagraphStyle::new();
        style.set_text_align(match spec.align {
            TextAlign::Start => SkAlign::Start,
            TextAlign::Center => SkAlign::Center,
            TextAlign::End => SkAlign::End,
        });
        style.set_text_direction(if starts_rtl(spec.text) {
            TextDirection::RTL
        } else {
            TextDirection::LTR
        });
        let mut text_style = TextStyle::new();
        text_style.set_font_families(&families);
        text_style.set_font_size(spec.size);
        text_style.set_height((spec.size * 1.4).round() / spec.size);
        text_style.set_height_override(true);
        let mut paint = Paint::new(Color4f::new(0.07, 0.09, 0.15, 1.0), None);
        paint.set_anti_alias(true);
        text_style.set_foreground_paint(&paint);
        let mut builder = ParagraphBuilder::new(&style, collection.clone());
        builder.push_style(&text_style);
        builder.add_text(spec.text);
        let mut paragraph = builder.build();
        paragraph.layout(spec.width);
        paint_to_image(size, size, |canvas| {
            canvas.clear(Color::WHITE);
            paragraph.paint(canvas, (16.0, 16.0));
        })
        .write_png(&out.join(format!("text-{n:02}.paragraph.png")))
        .map_err(|e| e.to_string())?;
        let _ = writeln!(
            json,
            "  {{\"fixture\": {n}, \"name\": \"{}\", \"parleyLines\": {}, \"paragraphLines\": {}}}{}",
            spec.name,
            parley_lines,
            paragraph.line_number(),
            if n < TEXT_FIXTURE_COUNT { "," } else { "" }
        );
    }
    json.push_str("]\n");
    std::fs::write(out.join("text-layout.json"), json).map_err(|e| e.to_string())
}

fn compare(args: &[String]) -> Result<bool, String> {
    let [results, artifacts, channel, fraction, pairs @ ..] = args else {
        return Err("compare <results.json> <artifact-dir> <channel> <fraction> <name>=<expected>,<actual>...".into());
    };
    let tolerance = Tolerance {
        channel: channel.parse().map_err(|e| format!("channel: {e}"))?,
        max_differing_fraction: fraction.parse().map_err(|e| format!("fraction: {e}"))?,
    };
    let artifacts = PathBuf::from(artifacts);
    let mut all_passed = true;
    let mut json = String::from("[\n");
    for (i, pair) in pairs.iter().enumerate() {
        let (name, files) = pair.split_once('=').ok_or(format!("bad pair {pair}"))?;
        let (expected, actual) = files.split_once(',').ok_or(format!("bad pair {pair}"))?;
        let expected = Rgba8Image::read_png(Path::new(expected)).map_err(|e| e.to_string())?;
        let actual = Rgba8Image::read_png(Path::new(actual)).map_err(|e| e.to_string())?;
        let (report, diff) =
            match assert_images_match(name, &expected, &actual, &tolerance, &artifacts) {
                Ok(report) => (report, String::new()),
                Err(PixelDiffError::Mismatch { report, diff }) => {
                    (*report, diff.display().to_string())
                }
                Err(e) => return Err(e.to_string()),
            };
        all_passed &= report.passed;
        let _ = writeln!(
            json,
            "  {{\"name\": \"{name}\", \"passed\": {}, \"differing_pixels\": {}, \"total_pixels\": {}, \
             \"differing_fraction\": {:.6}, \"max_channel_delta\": {}, \"p95_pixel_delta\": {}, \
             \"mean_pixel_delta\": {:.4}, \"diff\": \"{diff}\"}}{}",
            report.passed,
            report.differing_pixels,
            report.total_pixels,
            report.differing_fraction(),
            report.max_channel_delta,
            report.p95_pixel_delta,
            report.mean_pixel_delta,
            if i + 1 < pairs.len() { "," } else { "" },
        );
    }
    json.push_str("]\n");
    std::fs::write(results, json).map_err(|e| format!("{results}: {e}"))?;
    Ok(all_passed)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("fixtures") if args.len() == 2 => fixtures(Path::new(&args[1])).map(|()| true),
        Some("document") if args.len() == 4 => (|| {
            let nodes: usize = args[1].parse().map_err(|e| format!("nodes: {e}"))?;
            let font = std::fs::read(&args[3]).map_err(|e| format!("{}: {e}", args[3]))?;
            render(&generate(nodes), 1280, 800, Some(&font))
                .write_png(Path::new(&args[2]))
                .map_err(|e| e.to_string())
                .map(|()| true)
        })(),
        Some("text-fixtures") if args.len() == 3 => {
            text_fixtures(Path::new(&args[1]), Path::new(&args[2])).map(|()| true)
        }
        Some("compare") => compare(&args[1..]),
        _ => Err("usage: skia-native fixtures <out-dir> | document <nodes> <out.png> <font.ttf> | compare ...".into()),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(message) => {
            eprintln!("skia-native: {message}");
            ExitCode::from(2)
        }
    }
}

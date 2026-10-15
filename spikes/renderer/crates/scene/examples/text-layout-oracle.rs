//! TEST DATA: serialize Prompt 5's positioned glyph oracle without a renderer or GPU.
//! Run from scripts/gen-text-fixtures.mjs. Requires the five checksum-verified spike fonts.
use spike_scene::{FONT_FILES, Node, TEXT_FIXTURE_COUNT, TextBlock, TextEngine, text_fixture_spec};
fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fonts");
    let fonts: Vec<_> = FONT_FILES
        .iter()
        .map(|name| {
            std::fs::read(root.join(name)).expect("run pnpm spike:fonts in spikes/renderer/web")
        })
        .collect();
    let mut engine = TextEngine::new(&fonts);
    println!("[");
    for n in 1..=TEXT_FIXTURE_COUNT {
        let f = text_fixture_spec(n);
        let b = TextBlock {
            text: f.text,
            x: 0.0,
            y: 0.0,
            size: f.size,
            line_height: (f.size * 1.4).round(),
            width: f.width,
            align: f.align,
            color: spike_scene::Color::rgba(0.0, 0.0, 0.0, 1.0),
        };
        let nodes = engine.layout(&b);
        print!(
            "{{\"name\":{:?},\"text\":{},\"size\":{},\"lineHeight\":{},\"width\":{},\"align\":{:?},\"lines\":{},\"runs\":[",
            f.name,
            json_string(f.text),
            f.size,
            b.line_height,
            f.width,
            format!("{:?}", f.align),
            engine.last_line_count()
        );
        for (i, node) in nodes.iter().enumerate() {
            let Node::Glyphs {
                font, size, glyphs, ..
            } = node
            else {
                panic!()
            };
            if i > 0 {
                print!(",");
            }
            print!("{{\"font\":{font},\"size\":{size},\"glyphs\":[");
            for (j, (id, x, y)) in glyphs.iter().enumerate() {
                if j > 0 {
                    print!(",");
                }
                print!("[{id},{x},{y}]");
            }
            print!("]}}");
        }
        println!("]}}{}", if n < TEXT_FIXTURE_COUNT { "," } else { "" });
    }
    println!("]");
}

fn json_string(text: &str) -> String {
    let mut s = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            c if c.is_control() => s.push_str(&format!("\\u{:04x}", c as u32)),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

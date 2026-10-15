//! TEST DATA: wire ABI validation, exact integer identities, malformed input and deterministic bytes.
use proptest::prelude::*;
use vellum_types::{BlendMode, display::*};
#[test]
fn integer_font_and_glyph_ids_are_not_lossy_floats() {
    let list = DisplayList {
        commands: vec![
            DrawCmd::Save,
            DrawCmd::GlyphRun {
                glyphs: 0,
                paint: 1,
                stroke: None,
            },
            DrawCmd::Restore,
        ],
        resources: vec![
            Resource::Glyphs(RenderGlyphRun {
                font: u64::MAX,
                size: 12.25,
                glyphs: vec![RenderGlyph {
                    id: u32::MAX,
                    cluster: u32::MAX,
                    position: [0.125, -0.25],
                    advance: 0.375,
                }],
                normalized_coords: vec![-1234, 16384],
                embolden: true,
                skew: Some(12.),
                underline: Some([1., 0.5]),
                strikethrough: None,
                link: None,
            }),
            Resource::Paint(RenderPaint::Solid([1, 2, 3, 4])),
        ],
    };
    let bytes = list.to_bytes().unwrap();
    assert_eq!(DisplayList::from_bytes(&bytes).unwrap(), list);
    assert_eq!(
        bytes,
        DisplayList::from_bytes(&bytes).unwrap().to_bytes().unwrap()
    );
}
#[test]
fn refuses_invalid_resource_kinds_and_state_and_nonfinite_scalars() {
    let mut list = DisplayList {
        commands: vec![DrawCmd::Restore],
        ..Default::default()
    };
    assert_eq!(list.to_bytes(), Err(BufferError::Unbalanced));
    list.commands = vec![DrawCmd::Save];
    assert_eq!(list.to_bytes(), Err(BufferError::Unbalanced));
    list.commands = vec![DrawCmd::Clip { shape: 0 }];
    list.resources = vec![Resource::Paint(RenderPaint::Solid([0; 4]))];
    assert_eq!(list.to_bytes(), Err(BufferError::InvalidResource));
    list.commands = vec![
        DrawCmd::Layer {
            opacity: f32::NAN,
            blend: BlendMode::Normal,
        },
        DrawCmd::Restore,
    ];
    assert_eq!(list.to_bytes(), Err(BufferError::InvalidNumber));
    list.commands.clear();
    list.resources = vec![Resource::Shape(Shape::Line {
        width: f32::INFINITY,
    })];
    assert_eq!(list.to_bytes(), Err(BufferError::InvalidNumber));
}
#[test]
fn byte_fixture_pins_the_header_and_command_opcodes() {
    let list = DisplayList {
        commands: vec![DrawCmd::Save, DrawCmd::Restore],
        resources: vec![],
    };
    assert_eq!(
        list.to_bytes().unwrap(),
        [
            b"VDL\0".as_slice(),
            &1_u32.to_le_bytes(),
            &2_u32.to_le_bytes(),
            &0_u32.to_le_bytes(),
            &[0, 1]
        ]
        .concat()
    );
    let mut bytes = list.to_bytes().unwrap();
    bytes.push(0);
    assert_eq!(DisplayList::from_bytes(&bytes), Err(BufferError::Malformed));
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn arbitrary_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(),0..2048)) {
        let _ = DisplayList::from_bytes(&bytes);
    }
    #[test]
    fn finite_transforms_roundtrip_exactly(m in prop::array::uniform6(-1e8_f32..1e8_f32)) {
        let list=DisplayList{commands:vec![DrawCmd::Save,DrawCmd::Transform(m),DrawCmd::Restore],resources:vec![]};
        let bytes=list.to_bytes().unwrap();
        prop_assert_eq!(DisplayList::from_bytes(&bytes).unwrap(),list);
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn mutated_structured_buffers_never_panic(edits in prop::collection::vec((any::<usize>(),any::<u8>()),1..16)) {
        let list=DisplayList{commands:vec![DrawCmd::Save,DrawCmd::Clip{shape:0},DrawCmd::Restore],resources:vec![Resource::Shape(Shape::Rect{size:[10.,20.],radii:[1.,2.,3.,4.]})]};
        let mut bytes=list.to_bytes().unwrap();
        let len=bytes.len();
        for (at,value) in edits{bytes[at%len]=value;}
        let _ = DisplayList::from_bytes(&bytes);
    }
}

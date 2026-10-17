//! TEST DATA aside, these read the shipped sets themselves: their names, counts and drawings are the product.
use vellum_core::ports::{IconLibrary, IconStyle};
use vellum_icons::{Art, Catalog, Library, Record, SetInfo};

fn catalog() -> Catalog {
    Catalog::builtin()
}

#[test]
fn every_shipped_set_passes_the_library_contract() {
    let catalog = catalog();
    for (id, name, word) in [
        ("lucide", "house", "home"),
        ("feather", "home", "house"),
        ("phosphor", "house", "buildings"),
        ("material-symbols-outlined", "home", "building"),
        ("material-symbols-rounded", "home", "building"),
        ("material-symbols-sharp", "home", "building"),
    ] {
        let lib = catalog.library(id).unwrap_or_else(|| panic!("{id}"));
        vellum_contracts::suites::import::icon_library(lib, name, word);
    }
}

#[test]
fn the_sets_hold_what_they_claim() {
    let c = catalog();
    let sizes: Vec<_> = c
        .ids()
        .iter()
        .map(|id| (id.clone(), c.library(id).unwrap().len()))
        .collect();
    assert_eq!(
        sizes,
        [
            ("lucide".to_owned(), 1891),
            ("material-symbols-outlined".to_owned(), 700),
            ("material-symbols-rounded".to_owned(), 700),
            ("material-symbols-sharp".to_owned(), 700),
            ("phosphor".to_owned(), 1512),
            ("feather".to_owned(), 287),
        ]
    );
    let styles: Vec<_> = c
        .ids()
        .iter()
        .map(|id| c.library(id).unwrap().style())
        .collect();
    assert_eq!(
        styles,
        [
            IconStyle::Stroke,
            IconStyle::Fill,
            IconStyle::Fill,
            IconStyle::Fill,
            IconStyle::Fill,
            IconStyle::Stroke
        ]
    );
}

#[test]
fn material_symbols_alone_means_outlined() {
    let c = catalog();
    assert_eq!(
        c.library("material-symbols").unwrap().id(),
        "material-symbols-outlined"
    );
    assert!(c.library("nope").is_none());
}

#[test]
fn stroke_sets_turn_weight_into_line_width() {
    let lucide = catalog();
    let lucide = lucide.library("lucide").unwrap();
    let widths: Vec<f64> = [100, 300, 400, 700, 1000, 0]
        .iter()
        .map(|w| {
            lucide
                .glyph("star", *w, false)
                .unwrap()
                .stroke
                .unwrap()
                .width
        })
        .collect();
    assert_eq!(widths, [0.5, 1.5, 2.0, 3.5, 3.5, 0.5]);
    let g = lucide.glyph("arrow-right", 400, true).unwrap();
    assert!(!g.fill, "Lucide has no filled variant");
    assert_eq!(
        g.path, "M5 12h14M12 5l7 7-7 7",
        "separate path elements join with absolute moves"
    );
    assert_eq!(g.view_box, [0., 0., 24., 24.]);
    assert!(g.stroke.unwrap().round);
}

#[test]
fn phosphor_has_four_weights_and_a_fill() {
    let c = catalog();
    let p = c.library("phosphor").unwrap();
    assert_eq!(p.weights(), [100, 300, 400, 700]);
    let thin = p.glyph("house", 100, false).unwrap();
    let bold = p.glyph("house", 700, false).unwrap();
    let filled = p.glyph("house", 400, true).unwrap();
    assert!(thin.path != bold.path && bold.path != filled.path);
    assert!(filled.fill && !thin.fill);
    assert_eq!(
        (
            thin.weight,
            bold.weight,
            p.glyph("house", 250, false).unwrap().weight
        ),
        (100, 700, 300)
    );
    assert!(thin.stroke.is_none());
    assert_eq!(thin.view_box, [0., 0., 256., 256.]);
}

#[test]
fn material_symbols_have_one_weight_and_a_fill_in_their_own_view_box() {
    let c = catalog();
    let m = c.library("material-symbols-rounded").unwrap();
    assert_eq!(m.weights(), [400]);
    let outline = m.glyph("home", 700, false).unwrap();
    let solid = m.glyph("home", 100, true).unwrap();
    assert_eq!((outline.weight, solid.weight), (400, 400));
    assert!(solid.fill && !outline.fill && outline.path != solid.path);
    assert_eq!(outline.view_box, [0., -960., 960., 960.]);
}

#[test]
fn search_finds_by_name_part_and_tag_and_ranks_exact_names_first() {
    let c = catalog();
    let lucide = c.library("lucide").unwrap();
    let first = |q: &str| {
        lucide
            .search(q, 5)
            .into_iter()
            .map(|i| i.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(first("arrow-right")[0], "arrow-right");
    assert_eq!(first("Arrow Right")[0], "arrow-right");
    assert!(first("magnifier").contains(&"search".to_owned()));
    assert_eq!(lucide.search("", 3).len(), 3);
    let a = lucide.search("arrow", 50);
    assert_eq!(
        a,
        lucide.search("arrow", 50),
        "the same query gives the same order"
    );
    assert!(
        a.iter()
            .all(|i| i.name.contains("arrow") || i.tags.iter().any(|t| t.contains("arrow")))
    );
}

#[test]
fn a_library_can_be_made_from_records_and_added_to_the_catalog() {
    let info = SetInfo {
        id: "project".into(),
        label: "Project icons".into(),
        style: IconStyle::Fill,
        view_box: [0., 0., 16., 16.],
        weights: vec![400],
        fill: false,
    };
    let lib = Library::from_records(
        info,
        vec![
            Record {
                name: "logo".into(),
                tags: vec!["brand".into()],
                art: Art::One("M0 0h16v16z".into()),
            },
            Record {
                name: "logo".into(),
                tags: vec![],
                art: Art::One("M1 1z".into()),
            },
        ],
    );
    assert_eq!(lib.len(), 1, "a name is kept once");
    let c = Catalog::builtin().with(lib);
    assert_eq!(c.ids().last().map(String::as_str), Some("project"));
    assert_eq!(
        c.library("project")
            .unwrap()
            .glyph("logo", 400, false)
            .unwrap()
            .path,
        "M0 0h16v16z"
    );
    assert_eq!(c.library("lucide").unwrap().id(), "lucide");
}

#[test]
fn damaged_data_is_an_error_not_a_panic() {
    for bad in ["", "{}", "{\"id\":\"x\"", "{\"id\":\"x\"},\"icons\":[1]}"] {
        assert!(Library::from_json(bad).is_err(), "{bad}");
    }
}

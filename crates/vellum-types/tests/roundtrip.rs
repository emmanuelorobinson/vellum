//! Property tests (guide Prompt 7 FINAL): any node the types can hold prints to JSON, parses back to
//! an equal node, and prints the same bytes again. Unknown fields are refused wherever they land.

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::TestRunner;
use serde_json::Value as Json;
use vellum_types::*;

#[path = "common/strategies.rs"]
mod strategies;

use strategies::*;

// --- properties ---------------------------------------------------------------------------------

/// The node strategies are large tuples of nested strategies; building one in a debug build needs more
/// than the 2 MiB a test thread gets. The properties run on a thread with room.
fn on_a_big_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(test)
        .expect("spawn")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
}

fn runner() -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases: 128,
        ..ProptestConfig::default()
    })
}

#[test]
fn a_node_survives_a_trip_through_json() {
    on_a_big_stack(|| {
        runner()
            .run(&arb_node(), |node| {
                let text = serde_json::to_string(&node).expect("serialise");
                let back: Node = serde_json::from_str(&text).expect("deserialise");
                prop_assert_eq!(&back, &node);
                prop_assert_eq!(serde_json::to_string(&back).expect("serialise again"), text);
                Ok(())
            })
            .unwrap();
    });
}

#[test]
fn a_node_survives_pretty_printing() {
    on_a_big_stack(|| {
        runner()
            .run(&arb_node(), |node| {
                let text = serde_json::to_string_pretty(&node).expect("serialise");
                let back: Node = serde_json::from_str(&text).expect("deserialise");
                prop_assert_eq!(back, node);
                Ok(())
            })
            .unwrap();
    });
}

#[test]
fn an_unknown_field_is_refused_wherever_it_lands() {
    on_a_big_stack(|| {
        runner()
            .run(&(arb_node(), "zz[a-z]{1,6}"), |(node, key)| {
                let mut json = serde_json::to_value(&node).expect("serialise");
                json.as_object_mut()
                    .expect("object")
                    .insert(key.clone(), Json::Null);
                prop_assert!(serde_json::from_value::<Node>(json).is_err());

                // `metadata` is the one place an unknown key is welcome.
                let mut json = serde_json::to_value(&node).expect("serialise");
                json.as_object_mut()
                    .expect("object")
                    .entry("metadata")
                    .or_insert_with(|| Json::Object(Default::default()))
                    .as_object_mut()
                    .expect("metadata is an object")
                    .insert(key, Json::Null);
                prop_assert!(serde_json::from_value::<Node>(json).is_ok());
                Ok(())
            })
            .unwrap();
    });
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn an_id_survives_a_trip_and_never_holds_a_slash(text in arb_id_text(), other in "\\PC{0,8}") {
        let id = NodeId::new(text.clone()).expect("valid");
        let back: NodeId = serde_json::from_str(&serde_json::to_string(&id).expect("ser")).expect("de");
        prop_assert_eq!(back, id);
        let with_slash = format!("{text}/{other}");
        prop_assert!(NodeId::new(with_slash).is_err());
    }

    #[test]
    fn a_node_id_path_prints_and_parses_back(ids in vec(arb_id_text(), 1..6)) {
        let text = ids.join("/");
        let path: NodeIdPath = text.parse().expect("parse");
        prop_assert_eq!(path.to_string(), text);
        prop_assert_eq!(path.len(), ids.len());
        let back: NodeIdPath = serde_json::from_str(&serde_json::to_string(&path).expect("ser")).expect("de");
        prop_assert_eq!(back, path);
    }

    #[test]
    fn a_colour_prints_and_parses_back(color in arb_color()) {
        let text = color.to_string();
        prop_assert!(text.len() == 7 || text.len() == 9);
        prop_assert_eq!(text.parse::<Color>().expect("parse"), color);
        prop_assert_eq!(text.to_uppercase(), text.clone());
        prop_assert_eq!(text.to_lowercase().parse::<Color>().expect("parse lower case"), color);
    }

    #[test]
    fn a_size_survives_a_trip_through_json(size in arb_size()) {
        let back: Size = serde_json::from_str(&serde_json::to_string(&size).expect("ser")).expect("de");
        prop_assert_eq!(back, size);
    }
}

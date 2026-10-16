//! TEST DATA. Every content port accepts its fake and rejects an implementation with fabricated bodies.
use vellum_contracts::{
    fakes::{MemorySkills, MemoryStyles},
    suites,
};
use vellum_core::ports::*;

fn skills() -> MemorySkills {
    MemorySkills(vec![(
        SkillMeta {
            path: "test.md".into(),
            name: "Test".into(),
            description: "TEST DATA".into(),
            group: SkillGroup::User,
        },
        "TEST DATA body".into(),
    )])
}
fn styles() -> MemoryStyles {
    MemoryStyles(vec![VisualStyle {
        name: "test".into(),
        description: "TEST DATA".into(),
        palette: [("text".into(), "#000000".into())].into(),
        typography: StyleType {
            body: "TEST DATA Sans".into(),
            heading: "TEST DATA Sans".into(),
            mono: "TEST DATA Mono".into(),
            base_size: 16.0,
            scale: 1.25,
        },
        radius: 4.0,
        density: StyleDensity {
            gap: 8.0,
            padding: 16.0,
            control_height: 40.0,
        },
    }])
}
#[test]
fn content_fakes_pass_their_suites() {
    suites::content::skills(&skills());
    suites::content::styles(&styles());
}
struct BrokenSkills;
impl SkillSource for BrokenSkills {
    fn list(&self) -> Result<Vec<SkillMeta>, ContentError> {
        skills().list()
    }
    fn read(&self, _: &str) -> Result<String, ContentError> {
        Ok(String::new())
    }
}
#[test]
#[should_panic(expected = "body is empty")]
fn skill_suite_rejects_empty_bodies() {
    suites::content::skills(&BrokenSkills);
}
struct BrokenStyles;
impl StyleLibrary for BrokenStyles {
    fn list(&self) -> Result<Vec<StyleMeta>, ContentError> {
        styles().list()
    }
    fn read(&self, name: &str) -> Result<VisualStyle, ContentError> {
        let mut s = styles().read(name)?;
        s.radius = f64::NAN;
        Ok(s)
    }
}
#[test]
#[should_panic(expected = "radius is invalid")]
fn style_suite_rejects_nonfinite_sizes() {
    suites::content::styles(&BrokenStyles);
}

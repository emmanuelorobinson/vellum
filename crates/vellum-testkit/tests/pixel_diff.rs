//! The pixel-diff helper (guide Prompt 4): golden images compare within a tolerance, and a failed
//! comparison leaves a diff image behind as an artifact.

use std::path::PathBuf;

use vellum_testkit::pixel_diff::{
    PixelDiffError, Rgba8Image, Tolerance, assert_images_match, compare,
};

fn solid(width: u32, height: u32, px: [u8; 4]) -> Rgba8Image {
    Rgba8Image::new(width, height, px.repeat((width * height) as usize)).unwrap()
}

fn with_pixel(mut image: Rgba8Image, x: u32, y: u32, px: [u8; 4]) -> Rgba8Image {
    image.set(x, y, px);
    image
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vellum-pixel-diff-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn identical_images_have_no_difference() {
    let a = solid(8, 8, [10, 20, 30, 255]);
    let comparison = compare(&a, &a.clone(), &Tolerance::EXACT).unwrap();
    assert_eq!(comparison.report.differing_pixels, 0);
    assert_eq!(comparison.report.max_channel_delta, 0);
    assert_eq!(comparison.report.total_pixels, 64);
    assert!(comparison.report.passed);
}

#[test]
fn a_difference_within_the_channel_tolerance_passes() {
    let a = solid(4, 4, [100, 100, 100, 255]);
    let b = with_pixel(a.clone(), 1, 1, [103, 98, 100, 255]);
    let tolerance = Tolerance {
        channel: 3,
        max_differing_fraction: 0.0,
    };
    let report = compare(&a, &b, &tolerance).unwrap().report;
    assert_eq!(report.max_channel_delta, 3);
    assert_eq!(report.differing_pixels, 0);
    assert!(report.passed);
}

#[test]
fn a_difference_beyond_the_channel_tolerance_counts_and_fails() {
    let a = solid(4, 4, [100, 100, 100, 255]);
    let b = with_pixel(a.clone(), 2, 3, [140, 100, 100, 255]);
    let report = compare(
        &a,
        &b,
        &Tolerance {
            channel: 3,
            max_differing_fraction: 0.0,
        },
    )
    .unwrap()
    .report;
    assert_eq!(report.differing_pixels, 1);
    assert_eq!(report.max_channel_delta, 40);
    assert!((report.differing_fraction() - 1.0 / 16.0).abs() < 1e-12);
    assert!(!report.passed);
}

#[test]
fn the_differing_fraction_tolerance_admits_a_few_pixels() {
    let a = solid(10, 10, [0, 0, 0, 255]);
    let b = with_pixel(a.clone(), 0, 0, [255, 255, 255, 255]);
    let tolerance = Tolerance {
        channel: 0,
        max_differing_fraction: 0.01,
    };
    assert!(compare(&a, &b, &tolerance).unwrap().report.passed);
    let c = with_pixel(b, 1, 0, [255, 255, 255, 255]);
    assert!(!compare(&a, &c, &tolerance).unwrap().report.passed);
}

#[test]
fn p95_reports_the_tail_of_per_pixel_deltas() {
    // 100 pixels: 94 identical, 6 off by 50. The 95th percentile per-pixel delta is 50.
    let a = solid(10, 10, [0, 0, 0, 255]);
    let mut b = a.clone();
    for x in 0..6 {
        b.set(x, 9, [50, 0, 0, 255]);
    }
    let report = compare(&a, &b, &Tolerance::EXACT).unwrap().report;
    assert_eq!(report.p95_pixel_delta, 50);
    let mut c = a.clone();
    for x in 0..4 {
        c.set(x, 9, [50, 0, 0, 255]);
    }
    assert_eq!(
        compare(&a, &c, &Tolerance::EXACT)
            .unwrap()
            .report
            .p95_pixel_delta,
        0
    );
}

#[test]
fn size_mismatch_is_an_error_not_a_pass() {
    let err = compare(
        &solid(4, 4, [0; 4]),
        &solid(4, 5, [0; 4]),
        &Tolerance::EXACT,
    )
    .unwrap_err();
    assert!(matches!(err, PixelDiffError::SizeMismatch { .. }));
}

#[test]
fn buffers_of_the_wrong_length_are_rejected() {
    assert!(matches!(
        Rgba8Image::new(2, 2, vec![0; 15]),
        Err(PixelDiffError::BadLength { .. })
    ));
}

#[test]
fn the_diff_image_marks_differing_pixels() {
    let a = solid(3, 1, [200, 200, 200, 255]);
    let b = with_pixel(a.clone(), 1, 0, [0, 0, 0, 255]);
    let diff = compare(&a, &b, &Tolerance::EXACT).unwrap().diff;
    assert_eq!((diff.width(), diff.height()), (3, 1));
    assert_eq!(diff.get(1, 0), Tolerance::DIFF_MARK);
    assert_ne!(diff.get(0, 0), Tolerance::DIFF_MARK);
}

#[test]
fn png_round_trips() {
    let dir = scratch("png");
    std::fs::create_dir_all(&dir).unwrap();
    let image = with_pixel(solid(5, 3, [1, 2, 3, 255]), 4, 2, [9, 8, 7, 128]);
    let path = dir.join("a.png");
    image.write_png(&path).unwrap();
    assert_eq!(Rgba8Image::read_png(&path).unwrap(), image);
}

#[test]
fn a_failed_assertion_writes_expected_actual_and_diff_artifacts() {
    let dir = scratch("artifacts");
    let a = solid(4, 4, [0, 0, 0, 255]);
    let b = with_pixel(a.clone(), 0, 0, [255, 0, 0, 255]);
    let err = assert_images_match("fixture-01", &a, &b, &Tolerance::EXACT, &dir).unwrap_err();
    let PixelDiffError::Mismatch { report, diff } = err else {
        panic!("expected a mismatch, got {err:?}")
    };
    assert_eq!(report.differing_pixels, 1);
    assert_eq!(diff, dir.join("fixture-01.diff.png"));
    for suffix in ["expected", "actual", "diff"] {
        assert!(
            dir.join(format!("fixture-01.{suffix}.png")).is_file(),
            "{suffix} artifact missing"
        );
    }
}

#[test]
fn a_passing_assertion_writes_nothing() {
    let dir = scratch("pass");
    let a = solid(4, 4, [0, 0, 0, 255]);
    let report = assert_images_match("same", &a, &a.clone(), &Tolerance::EXACT, &dir).unwrap();
    assert!(report.passed);
    assert!(!dir.exists());
}

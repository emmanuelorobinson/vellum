use vellum_core::ports::*;
pub fn image_ops(ops: &dyn ImageOps, valid: &[u8]) {
    assert!(ops.remove_background(&[]).is_err());
    assert!(ops.trace(&[]).is_err());
    let first = ops
        .remove_background(valid)
        .expect("configured local model");
    assert_eq!(first.mime, "image/png");
    assert!(!first.bytes.is_empty());
    assert_eq!(first, ops.remove_background(valid).unwrap());
    let svg = ops.trace(valid).unwrap();
    assert!(svg.contains("<svg"));
    assert_eq!(svg, ops.trace(valid).unwrap());
}
pub fn segmentation(model: &dyn SegmentationModel) {
    assert!(model.mask(&[], 0, 0).is_err());
    let r = model.mask(&[128; 4 * 4 * 4], 4, 4).unwrap();
    assert_eq!(r.len(), 16);
    assert!(r.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
}
pub fn generator(provider: &dyn ImageGenerator) {
    assert!(!provider.provider().is_empty());
    assert!(provider.origin().starts_with("https://"));
    assert!(
        provider
            .generate(&GenerateImage {
                prompt: String::new(),
                model: String::new()
            })
            .is_err()
    );
    let r = provider
        .generate(&GenerateImage {
            prompt: "TEST DATA".into(),
            model: "fixture".into(),
        })
        .unwrap();
    assert!(r.mime.starts_with("image/"));
    assert!(!r.bytes.is_empty());
}
pub fn figma(source: &dyn FigmaSource) {
    assert!(
        source
            .import_file("test", &|_| Err(ImageWorkError::PermissionDenied))
            .is_err()
    );
    let r = source.import_file("test", &|_| Ok(())).unwrap();
    assert!(!r.nodes.is_empty());
}

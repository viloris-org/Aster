use engine_assets::{GltfImporter, ImportOptions, PngImporter};
use std::path::Path;

#[test]
fn png_valid_fixture_imports_successfully() {
    let fixture_path = Path::new("tests/fixtures/valid.png");
    let options = ImportOptions::default();

    let result = PngImporter::import(fixture_path, &options);

    assert!(result.is_ok(), "valid PNG should import successfully");
    let outcome = result.unwrap();
    assert!(
        outcome.diagnostics.is_empty(),
        "valid PNG should have no diagnostics: {:?}",
        outcome.diagnostics
    );
}

#[test]
fn png_invalid_fixture_does_not_panic() {
    let fixture_path = Path::new("tests/fixtures/invalid.png");
    let options = ImportOptions::default();

    let result = PngImporter::import(fixture_path, &options);

    // Should not panic, but should produce diagnostics
    assert!(result.is_ok(), "importer should not panic on invalid PNG");
    let outcome = result.unwrap();
    assert!(
        !outcome.diagnostics.is_empty(),
        "invalid PNG should produce at least one diagnostic"
    );
}

#[test]
fn gltf_valid_fixture_imports_successfully() {
    let fixture_path = Path::new("tests/fixtures/valid.gltf");

    let result = GltfImporter::import(fixture_path);

    assert!(result.is_ok(), "valid glTF should import successfully");
    let outcome = result.unwrap();
    assert!(
        outcome.diagnostics.is_empty(),
        "valid glTF should have no diagnostics: {:?}",
        outcome.diagnostics
    );
}

#[test]
fn gltf_invalid_fixture_does_not_panic() {
    let fixture_path = Path::new("tests/fixtures/invalid.gltf");

    let result = GltfImporter::import(fixture_path);

    // Should not panic, but should produce diagnostics or error
    assert!(result.is_ok(), "importer should not panic on invalid glTF");
    let outcome = result.unwrap();
    assert!(
        !outcome.diagnostics.is_empty(),
        "invalid glTF should produce at least one diagnostic"
    );
}

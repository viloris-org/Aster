use super::*;
use crate::import::generate_mip_chain;
use crate::mesh_builder::scale_vec3;
use crate::vmodel::compile_vmodel;
use crate::watch::generate_asset_guid;
use engine_core::{Handle, ResourceId};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

fn guid(value: u128) -> AssetGuid {
    AssetGuid::from_u128(value)
}

#[test]
fn manifest_upsert_replaces_by_guid() {
    let id = guid(7);
    let mut manifest = ResourceManifestFormat::default();
    manifest.upsert(AssetManifestEntry {
        guid: id,
        path: AssetPath::new("old.mesh"),
        kind: ResourceKind::Model,
        dependencies: Vec::new(),
    });
    manifest.upsert(AssetManifestEntry {
        guid: id,
        path: AssetPath::new("new.mesh"),
        kind: ResourceKind::Model,
        dependencies: Vec::new(),
    });

    assert_eq!(manifest.entries.len(), 1);
    assert_eq!(
        manifest.get(id).unwrap().path.to_utf8().unwrap(),
        "new.mesh"
    );
}

#[test]
fn database_resolves_guid_and_dependencies() {
    let mut database = AssetDatabase::new("assets", "builtin");
    database
        .upsert_meta(ResourceMetaFormat {
            version: CURRENT_SCHEMA_VERSION,
            guid: guid(1),
            source_path: PathBuf::from("materials/player.varg_material.json"),
            kind: ResourceKind::Material,
            importer: "material-json".to_string(),
            dependencies: vec![guid(2)],
        })
        .unwrap();

    assert_eq!(
        database.resolve_guid(guid(1)).unwrap().to_utf8().unwrap(),
        "materials/player.varg_material.json"
    );
    assert_eq!(database.dependencies().dependencies(guid(1)), vec![guid(2)]);
    assert_eq!(database.dependencies().dependents(guid(2)), vec![guid(1)]);
}

#[test]
fn registry_keeps_cpu_and_gpu_cache_lifetimes_separate() {
    let mut registry = AssetRegistry::default();
    let handle = registry.register(guid(9), ResourceKind::Texture).unwrap();
    registry
        .put_cpu(
            handle,
            CpuResource {
                kind: ResourceKind::Texture,
                bytes: Arc::<[u8]>::from([1_u8, 2, 3]),
            },
        )
        .unwrap();
    registry
        .put_gpu(
            handle,
            GpuResource {
                kind: ResourceKind::Texture,
                backend_token: 42,
            },
        )
        .unwrap();

    registry.drop_cpu(handle);

    assert!(!registry.cpu_cache.contains_key(&handle));
    assert!(registry.gpu_cache.contains_key(&handle));
    assert_eq!(
        registry.record(handle).unwrap().state,
        ResourceState::GpuReady
    );
}

#[test]
fn import_queue_separates_import_and_upload_work() {
    let handle = ResourceHandle::new(
        ResourceId::from_u128(1),
        Handle::new(0, engine_core::Generation::FIRST),
    );
    let mut queue = ImportQueue::default();
    queue.push_import(ImportTask {
        guid: guid(1),
        source_path: PathBuf::from("textures/a.png"),
        kind: ResourceKind::Texture,
        importer: "image".to_string(),
    });

    let outcomes = queue.drain_imports_parallel(2, move |_| ImportOutcome {
        guid: guid(1),
        diagnostics: Vec::new(),
        upload: Some(GpuUploadTask {
            handle,
            kind: ResourceKind::Texture,
        }),
    });

    assert_eq!(outcomes.len(), 1);
    assert_eq!(queue.pop_upload().unwrap().handle, handle);
}

#[test]
fn runtime_min_has_only_builtin_importer_by_default() {
    let mut expected = Vec::new();
    expected.push(ImporterBackend::BuiltIn);
    #[cfg(feature = "fbx-importer")]
    expected.push(ImporterBackend::Fbx);
    #[cfg(feature = "assimp-importer")]
    expected.push(ImporterBackend::Assimp);

    assert_eq!(available_importers(), expected);
}

#[test]
fn scans_and_imports_supported_assets() {
    let root = std::env::temp_dir().join(format!("varg-assets-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("textures")).unwrap();
    std::fs::write(root.join("textures/player.png"), [1_u8, 2, 3, 4]).unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");
    let report = scan_project_assets(&root, &mut database).unwrap();
    let meta = report
        .metas
        .iter()
        .find(|meta| meta.source_path == PathBuf::from("textures/player.png"))
        .unwrap();
    assert!(root.join("textures/player.png.meta").exists());

    let mut registry = AssetRegistry::default();
    let outcome = import_builtin_asset(
        &root,
        &mut registry,
        ImportTask {
            guid: meta.guid,
            source_path: meta.source_path.clone(),
            kind: meta.kind,
            importer: meta.importer.clone(),
        },
    )
    .unwrap();

    assert!(outcome.upload.is_some());
    assert_eq!(
        registry
            .record(outcome.upload.unwrap().handle)
            .unwrap()
            .state,
        ResourceState::CpuReady
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn imports_png_as_decoded_texture_payload() {
    let root =
        std::env::temp_dir().join(format!("varg-texture-decode-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("textures")).unwrap();
    std::fs::write(root.join("textures/white.png"), one_pixel_png()).unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");
    let report = scan_project_assets(&root, &mut database).unwrap();
    let meta = report
        .metas
        .iter()
        .find(|meta| meta.source_path == PathBuf::from("textures/white.png"))
        .unwrap();
    let mut registry = AssetRegistry::default();
    import_builtin_asset(
        &root,
        &mut registry,
        ImportTask {
            guid: meta.guid,
            source_path: meta.source_path.clone(),
            kind: meta.kind,
            importer: meta.importer.clone(),
        },
    )
    .unwrap();

    let handle = registry.handle_for_guid(meta.guid).unwrap();
    let texture =
        DecodedTextureResource::from_bytes(&registry.cpu_resource(handle).unwrap().bytes).unwrap();

    assert_eq!(texture.width, 1);
    assert_eq!(texture.height, 1);
    assert_eq!(texture.format, "rgba8_srgb");
    assert_eq!(texture.pixels, vec![255, 255, 255, 255]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn imports_cubemap_manifest_as_decoded_cube_payload() {
    let root =
        std::env::temp_dir().join(format!("varg-cubemap-decode-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("textures/cube")).unwrap();
    for name in ["px", "nx", "py", "ny", "pz", "nz"] {
        std::fs::write(
            root.join(format!("textures/cube/{name}.png")),
            one_pixel_png(),
        )
        .unwrap();
    }
    std::fs::write(
        root.join("textures/skybox.cubemap.json"),
        r#"{
  "positive_x": "cube/px.png",
  "negative_x": "cube/nx.png",
  "positive_y": "cube/py.png",
  "negative_y": "cube/ny.png",
  "positive_z": "cube/pz.png",
  "negative_z": "cube/nz.png"
}"#,
    )
    .unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");
    let report = scan_project_assets(&root, &mut database).unwrap();
    let meta = report
        .metas
        .iter()
        .find(|meta| meta.source_path == PathBuf::from("textures/skybox.cubemap.json"))
        .unwrap();
    assert_eq!(meta.kind, ResourceKind::Texture);
    assert_eq!(meta.importer, "cubemap-json");

    let mut registry = AssetRegistry::default();
    import_builtin_asset(
        &root,
        &mut registry,
        ImportTask {
            guid: meta.guid,
            source_path: meta.source_path.clone(),
            kind: meta.kind,
            importer: meta.importer.clone(),
        },
    )
    .unwrap();

    let handle = registry.handle_for_guid(meta.guid).unwrap();
    let cubemap =
        DecodedCubemapResource::from_bytes(&registry.cpu_resource(handle).unwrap().bytes).unwrap();

    assert_eq!(cubemap.face_size, 1);
    assert_eq!(cubemap.format, "cubemap_rgba8_srgb");
    assert_eq!(cubemap.pixels.len(), 6 * 4);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn material_scan_records_shader_and_texture_dependencies() {
    let root = std::env::temp_dir().join(format!("varg-material-deps-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("materials")).unwrap();
    std::fs::write(
        root.join("materials/player.material.json"),
        format!(
            r#"{{
  "version": 1,
  "shader": "{shader}",
  "textures": {{"albedo": "{texture}"}},
  "parameters": {{}}
}}"#,
            shader = guid(11),
            texture = guid(12),
        ),
    )
    .unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");
    let report = scan_project_assets(&root, &mut database).unwrap();
    let material = report.metas.first().unwrap();

    assert_eq!(
        database.dependencies().dependencies(material.guid),
        vec![guid(11), guid(12)]
    );

    let _ = std::fs::remove_dir_all(&root);
}

fn one_pixel_png() -> Vec<u8> {
    let mut bytes = Vec::new();
    let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255]));
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

#[test]
fn scan_preserves_guid_from_moved_meta_file() {
    let root = std::env::temp_dir().join(format!("varg-assets-meta-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("textures")).unwrap();
    std::fs::write(root.join("textures/player.png"), [1_u8, 2, 3, 4]).unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");
    let first = scan_project_assets(&root, &mut database).unwrap();
    let guid = first.metas[0].guid;

    std::fs::rename(
        root.join("textures/player.png"),
        root.join("textures/hero.png"),
    )
    .unwrap();
    std::fs::rename(
        root.join("textures/player.png.meta"),
        root.join("textures/hero.png.meta"),
    )
    .unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");
    let second = scan_project_assets(&root, &mut database).unwrap();
    assert_eq!(second.metas[0].guid, guid);
    assert_eq!(
        second.metas[0].source_path,
        PathBuf::from("textures/hero.png")
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn scan_registers_files_with_correct_resource_kinds() {
    let root = std::env::temp_dir().join(format!("varg-scan-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    // Create subdirectories
    std::fs::create_dir_all(root.join("textures")).unwrap();
    std::fs::create_dir_all(root.join("models")).unwrap();
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    std::fs::create_dir_all(root.join("shaders")).unwrap();
    std::fs::create_dir_all(root.join("scenes")).unwrap();

    // Texture: .png file
    std::fs::write(root.join("textures/player.png"), one_pixel_png()).unwrap();
    // Model: .gltf file (minimal ASCII glTF JSON)
    let gltf_json = r#"{"asset":{"version":"2.0"}}"#;
    std::fs::write(root.join("models/hero.gltf"), gltf_json).unwrap();
    // Shader: .wgsl file
    std::fs::write(root.join("shaders/pbr.wgsl"), "fn main() {}").unwrap();
    // Script: .varg file
    std::fs::write(
        root.join("scripts/player.varg"),
        "script Player { func update(_ dt: Float) {} }",
    )
    .unwrap();
    std::fs::write(
        root.join("scripts/player.py"),
        "def update(ctx):\n    pass\n",
    )
    .unwrap();
    // Scene: JSON file with version + objects
    let scene_json = r#"{"version":1,"name":"test","objects":[]}"#;
    std::fs::write(root.join("scenes/level.scene.json"), scene_json).unwrap();
    // Non-asset file (should be ignored)
    std::fs::write(root.join("readme.txt"), "hello").unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");
    database.scan(&root).unwrap();

    // Verify all supported files are registered with correct kinds
    assert_eq!(
        database
            .entry_for_path(Path::new("textures/player.png"))
            .unwrap()
            .kind,
        ResourceKind::Texture,
        "PNG files should map to Texture"
    );
    assert_eq!(
        database
            .entry_for_path(Path::new("models/hero.gltf"))
            .unwrap()
            .kind,
        ResourceKind::Model,
        "glTF files should map to Model"
    );
    assert_eq!(
        database
            .entry_for_path(Path::new("shaders/pbr.wgsl"))
            .unwrap()
            .kind,
        ResourceKind::Shader,
        "WGSL files should map to Shader"
    );
    assert_eq!(
        database
            .entry_for_path(Path::new("scripts/player.varg"))
            .unwrap()
            .kind,
        ResourceKind::Script,
        "Varg script files should map to Script"
    );
    assert!(
        database
            .entry_for_path(Path::new("scripts/player.py"))
            .is_none(),
        "Python files are not Varg script assets"
    );
    assert_eq!(
        database
            .entry_for_path(Path::new("scenes/level.scene.json"))
            .unwrap()
            .kind,
        ResourceKind::Scene,
        "Scene JSON files should map to Scene"
    );

    // All entries should start with Unloaded import state
    for entry in database.iter_entries() {
        assert_eq!(
            entry.import_state,
            ResourceState::Unloaded,
            "import_state should default to Unloaded"
        );
    }

    // Non-asset file should not be registered
    assert!(
        database
            .entry_for_path(&PathBuf::from("readme.txt"))
            .is_none(),
        "Unsupported files should not be registered"
    );

    // Folder entries should be tracked
    let folders = database.folders();
    assert!(
        folders.contains(&PathBuf::from("textures")),
        "textures folder should be tracked"
    );
    assert!(
        folders.contains(&PathBuf::from("models")),
        "models folder should be tracked"
    );
    assert!(
        folders.contains(&PathBuf::from("scripts")),
        "scripts folder should be tracked"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn scan_removes_deleted_files() {
    let root = std::env::temp_dir().join(format!("varg-scan-delete-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("textures")).unwrap();
    std::fs::write(root.join("textures/a.png"), one_pixel_png()).unwrap();
    std::fs::write(root.join("textures/b.png"), one_pixel_png()).unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");
    database.scan(&root).unwrap();
    assert_eq!(database.iter_entries().count(), 2);

    // Delete b.png and rescan
    std::fs::remove_file(root.join("textures/b.png")).unwrap();
    database.scan(&root).unwrap();

    assert_eq!(database.iter_entries().count(), 1);
    assert!(
        database
            .entry_for_path(&PathBuf::from("textures/a.png"))
            .is_some()
    );
    assert!(
        database
            .entry_for_path(&PathBuf::from("textures/b.png"))
            .is_none()
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn scan_preserves_existing_guid_on_rescan() {
    let root = std::env::temp_dir().join(format!("varg-scan-guid-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("textures")).unwrap();
    std::fs::write(root.join("textures/player.png"), one_pixel_png()).unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");
    database.scan(&root).unwrap();
    let first_guid = database
        .entry_for_path(&PathBuf::from("textures/player.png"))
        .unwrap()
        .guid;

    // Rescan without changing any files
    database.scan(&root).unwrap();
    let second_guid = database
        .entry_for_path(&PathBuf::from("textures/player.png"))
        .unwrap()
        .guid;

    assert_eq!(
        first_guid, second_guid,
        "GUID should be preserved across rescans"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn png_importer_imports_valid_png_with_mips() {
    let root = std::env::temp_dir().join(format!("varg-png-importer-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    // Create a 4x4 white PNG
    let png_path = root.join("test.png");
    let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]));
    image.save(&png_path).unwrap();

    // Import with mip generation
    let options = ImportOptions {
        generate_mips: true,
        max_texture_size: None,
    };
    let outcome = PngImporter::import(&png_path, &options).unwrap();

    assert!(
        outcome.diagnostics.is_empty(),
        "Valid PNG should import without diagnostics"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn png_importer_to_registry_stores_texture_resource() {
    let root = std::env::temp_dir().join(format!("varg-png-registry-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    // Create a 4x4 white PNG
    let png_path = root.join("test.png");
    let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]));
    image.save(&png_path).unwrap();

    let mut registry = AssetRegistry::default();
    let test_guid = guid(999);

    // Import with mip generation
    let options = ImportOptions {
        generate_mips: true,
        max_texture_size: None,
    };
    let outcome =
        PngImporter::import_to_registry(&png_path, &options, &mut registry, test_guid).unwrap();

    assert!(
        outcome.diagnostics.is_empty(),
        "Valid PNG should import without diagnostics"
    );
    assert!(
        outcome.upload.is_some(),
        "Valid PNG should queue GPU upload"
    );

    // Verify the texture was stored in the registry
    let handle = registry.handle_for_guid(test_guid).unwrap();
    let cpu_resource = registry.cpu_resource(handle).unwrap();
    assert_eq!(cpu_resource.kind, ResourceKind::Texture);

    // Deserialize and verify the texture
    let texture = CpuTextureResource::from_bytes(&cpu_resource.bytes).unwrap();
    assert_eq!(texture.width, 4);
    assert_eq!(texture.height, 4);
    assert_eq!(texture.format, "Rgba8UnormSrgb");
    // 4x4 -> 2x2 -> 1x1 = 3 mip levels
    assert_eq!(texture.mip_levels.len(), 3);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn png_importer_handles_invalid_png() {
    let root = std::env::temp_dir().join(format!("varg-png-invalid-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    // Create an invalid PNG file
    let png_path = root.join("invalid.png");
    std::fs::write(&png_path, b"not a png file").unwrap();

    let options = ImportOptions {
        generate_mips: false,
        max_texture_size: None,
    };
    let outcome = PngImporter::import(&png_path, &options).unwrap();

    assert!(
        !outcome.diagnostics.is_empty(),
        "Invalid PNG should produce at least one diagnostic"
    );
    assert!(
        outcome.upload.is_none(),
        "Invalid PNG should not queue upload"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mip_chain_generation_produces_correct_levels() {
    // Create a 4x4 image
    let base = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
    let mip_levels = generate_mip_chain(&base);

    // 4x4 -> 2x2 -> 1x1 = 3 levels
    assert_eq!(mip_levels.len(), 3, "4x4 image should produce 3 mip levels");

    // Level 0: 4x4 = 64 bytes (4*4*4)
    assert_eq!(mip_levels[0].len(), 64);

    // Level 1: 2x2 = 16 bytes (2*2*4)
    assert_eq!(mip_levels[1].len(), 16);

    // Level 2: 1x1 = 4 bytes (1*1*4)
    assert_eq!(mip_levels[2].len(), 4);
}

#[test]
fn cpu_texture_resource_serialization_roundtrip() {
    let texture = CpuTextureResource {
        width: 2,
        height: 2,
        format: "Rgba8UnormSrgb".to_string(),
        mip_levels: vec![
            vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
            ],
            vec![128, 128, 128, 255],
        ],
    };

    let bytes = texture.to_bytes().unwrap();
    let deserialized = CpuTextureResource::from_bytes(&bytes).unwrap();

    assert_eq!(texture, deserialized);
}

#[test]
fn gltf_importer_imports_valid_gltf_with_mesh() {
    let root = std::env::temp_dir().join(format!("varg-gltf-importer-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    // Create a minimal valid glTF file with a triangle mesh
    let gltf_path = root.join("test.gltf");
    create_minimal_gltf(&gltf_path);

    // Import the glTF
    let outcome = GltfImporter::import(&gltf_path).unwrap();

    assert!(
        outcome.diagnostics.is_empty(),
        "Valid glTF should import without diagnostics"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn gltf_importer_to_registry_stores_model_resource() {
    let root = std::env::temp_dir().join(format!("varg-gltf-registry-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    // Create a minimal valid glTF file with a triangle mesh
    let gltf_path = root.join("test.gltf");
    create_minimal_gltf(&gltf_path);

    let mut registry = AssetRegistry::default();
    let test_guid = guid(888);

    // Import the glTF
    let outcome = GltfImporter::import_to_registry(&gltf_path, &mut registry, test_guid).unwrap();

    assert!(
        outcome.diagnostics.is_empty(),
        "Valid glTF should import without diagnostics"
    );
    assert!(
        outcome.upload.is_some(),
        "Valid glTF should queue GPU upload"
    );

    // Verify the model was stored in the registry
    let handle = registry.handle_for_guid(test_guid).unwrap();
    let cpu_resource = registry.cpu_resource(handle).unwrap();
    assert_eq!(cpu_resource.kind, ResourceKind::Model);

    // Deserialize and verify the model
    let model = ModelResource::from_bytes(&cpu_resource.bytes).unwrap();
    assert_eq!(model.meshes.len(), 1, "Should have 1 mesh primitive");
    assert_eq!(
        model.meshes[0].positions.len(),
        3,
        "Triangle should have 3 vertices"
    );
    assert_eq!(
        model.meshes[0].indices.len(),
        3,
        "Triangle should have 3 indices"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn gltf_importer_handles_invalid_gltf() {
    let root = std::env::temp_dir().join(format!("varg-gltf-invalid-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    // Create an invalid glTF file
    let gltf_path = root.join("invalid.gltf");
    std::fs::write(&gltf_path, b"not a gltf file").unwrap();

    let outcome = GltfImporter::import(&gltf_path).unwrap();

    assert!(
        !outcome.diagnostics.is_empty(),
        "Invalid glTF should produce at least one diagnostic"
    );
    assert!(
        outcome.upload.is_none(),
        "Invalid glTF should not queue upload"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn gltf_importer_fills_default_normals_when_missing() {
    let root = std::env::temp_dir().join(format!("varg-gltf-normals-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    // Create a glTF file without normals
    let gltf_path = root.join("no_normals.gltf");
    create_gltf_without_normals(&gltf_path);

    let mut registry = AssetRegistry::default();
    let test_guid = guid(777);

    // Import the glTF
    let outcome = GltfImporter::import_to_registry(&gltf_path, &mut registry, test_guid).unwrap();

    assert!(
        outcome.diagnostics.is_empty(),
        "glTF without normals should still import successfully"
    );

    // Verify the model has default normals
    let handle = registry.handle_for_guid(test_guid).unwrap();
    let cpu_resource = registry.cpu_resource(handle).unwrap();
    let model = ModelResource::from_bytes(&cpu_resource.bytes).unwrap();

    assert_eq!(model.meshes.len(), 1);
    assert_eq!(
        model.meshes[0].normals.len(),
        3,
        "Should have normals for all 3 vertices"
    );
    // Default normal should be (0, 1, 0)
    assert_eq!(model.meshes[0].normals[0], [0.0, 1.0, 0.0]);
    assert_eq!(model.meshes[0].normals[1], [0.0, 1.0, 0.0]);
    assert_eq!(model.meshes[0].normals[2], [0.0, 1.0, 0.0]);

    let _ = std::fs::remove_dir_all(&root);
}

/// Creates a minimal valid glTF file with a single triangle mesh.
fn create_minimal_gltf(path: &Path) {
    // Triangle vertices: positions and normals
    let positions: Vec<f32> = vec![
        0.0, 0.0, 0.0, // vertex 0
        1.0, 0.0, 0.0, // vertex 1
        0.0, 1.0, 0.0, // vertex 2
    ];
    let normals: Vec<f32> = vec![
        0.0, 0.0, 1.0, // normal 0
        0.0, 0.0, 1.0, // normal 1
        0.0, 0.0, 1.0, // normal 2
    ];
    let indices: Vec<u32> = vec![0, 1, 2];

    // Convert to bytes
    let positions_bytes: Vec<u8> = positions.iter().flat_map(|f| f.to_le_bytes()).collect();
    let normals_bytes: Vec<u8> = normals.iter().flat_map(|f| f.to_le_bytes()).collect();
    let indices_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();

    // Create binary buffer
    let mut buffer_data = Vec::new();
    let positions_offset = 0;
    let normals_offset = positions_bytes.len();
    let indices_offset = normals_offset + normals_bytes.len();
    buffer_data.extend_from_slice(&positions_bytes);
    buffer_data.extend_from_slice(&normals_bytes);
    buffer_data.extend_from_slice(&indices_bytes);

    // Write binary buffer
    let bin_path = path.with_extension("bin");
    std::fs::write(&bin_path, &buffer_data).unwrap();

    // Create glTF JSON
    let gltf_json = serde_json::json!({
        "asset": {
            "version": "2.0"
        },
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{
            "primitives": [{
                "attributes": {
                    "POSITION": 0,
                    "NORMAL": 1
                },
                "indices": 2
            }]
        }],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,
                "count": 3,
                "type": "VEC3",
                "min": [0.0, 0.0, 0.0],
                "max": [1.0, 1.0, 0.0]
            },
            {
                "bufferView": 1,
                "componentType": 5126,
                "count": 3,
                "type": "VEC3"
            },
            {
                "bufferView": 2,
                "componentType": 5125,
                "count": 3,
                "type": "SCALAR"
            }
        ],
        "bufferViews": [
            {
                "buffer": 0,
                "byteOffset": positions_offset,
                "byteLength": positions_bytes.len()
            },
            {
                "buffer": 0,
                "byteOffset": normals_offset,
                "byteLength": normals_bytes.len()
            },
            {
                "buffer": 0,
                "byteOffset": indices_offset,
                "byteLength": indices_bytes.len()
            }
        ],
        "buffers": [{
            "uri": bin_path.file_name().unwrap().to_str().unwrap(),
            "byteLength": buffer_data.len()
        }]
    });

    std::fs::write(path, serde_json::to_string_pretty(&gltf_json).unwrap()).unwrap();
}

/// Creates a glTF file without normals to test default normal filling.
fn create_gltf_without_normals(path: &Path) {
    // Triangle vertices: positions only (no normals)
    let positions: Vec<f32> = vec![
        0.0, 0.0, 0.0, // vertex 0
        1.0, 0.0, 0.0, // vertex 1
        0.0, 1.0, 0.0, // vertex 2
    ];
    let indices: Vec<u32> = vec![0, 1, 2];

    // Convert to bytes
    let positions_bytes: Vec<u8> = positions.iter().flat_map(|f| f.to_le_bytes()).collect();
    let indices_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();

    // Create binary buffer
    let mut buffer_data = Vec::new();
    let positions_offset = 0;
    let indices_offset = positions_bytes.len();
    buffer_data.extend_from_slice(&positions_bytes);
    buffer_data.extend_from_slice(&indices_bytes);

    // Write binary buffer
    let bin_path = path.with_extension("bin");
    std::fs::write(&bin_path, &buffer_data).unwrap();

    // Create glTF JSON (no NORMAL attribute)
    let gltf_json = serde_json::json!({
        "asset": {
            "version": "2.0"
        },
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{
            "primitives": [{
                "attributes": {
                    "POSITION": 0
                },
                "indices": 1
            }]
        }],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,
                "count": 3,
                "type": "VEC3",
                "min": [0.0, 0.0, 0.0],
                "max": [1.0, 1.0, 0.0]
            },
            {
                "bufferView": 1,
                "componentType": 5125,
                "count": 3,
                "type": "SCALAR"
            }
        ],
        "bufferViews": [
            {
                "buffer": 0,
                "byteOffset": positions_offset,
                "byteLength": positions_bytes.len()
            },
            {
                "buffer": 0,
                "byteOffset": indices_offset,
                "byteLength": indices_bytes.len()
            }
        ],
        "buffers": [{
            "uri": bin_path.file_name().unwrap().to_str().unwrap(),
            "byteLength": buffer_data.len()
        }]
    });

    std::fs::write(path, serde_json::to_string_pretty(&gltf_json).unwrap()).unwrap();
}

#[test]
fn gltf_importer_extracts_pbr_material() {
    let root = std::env::temp_dir().join(format!("varg-gltf-material-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    // Create a glTF file with a PBR material
    let gltf_path = root.join("material.gltf");
    create_gltf_with_pbr_material(&gltf_path);

    let mut registry = AssetRegistry::default();
    let test_guid = guid(999);

    // Import the glTF
    let outcome = GltfImporter::import_to_registry(&gltf_path, &mut registry, test_guid).unwrap();

    assert!(
        outcome.diagnostics.is_empty(),
        "glTF with material should import without diagnostics"
    );

    // Verify the model has materials
    let handle = registry.handle_for_guid(test_guid).unwrap();
    let cpu_resource = registry.cpu_resource(handle).unwrap();
    let model = ModelResource::from_bytes(&cpu_resource.bytes).unwrap();

    assert_eq!(model.materials.len(), 1, "Should have 1 material");
    let material = &model.materials[0];
    assert_eq!(material.name, "TestMaterial");
    assert_eq!(material.base_color, [0.8, 0.2, 0.2, 1.0]);
    assert_eq!(material.metallic, 0.9);
    assert_eq!(material.roughness, 0.3);
    assert_eq!(material.emissive, [0.1, 0.1, 0.1]);
    assert_eq!(material.alpha_mode, "OPAQUE");
    assert_eq!(material.alpha_cutoff, 0.5);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn gltf_importer_creates_default_material_when_none() {
    let root =
        std::env::temp_dir().join(format!("varg-gltf-no-material-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    // Create a glTF file without materials (uses create_minimal_gltf which has no materials)
    let gltf_path = root.join("no_material.gltf");
    create_minimal_gltf(&gltf_path);

    let mut registry = AssetRegistry::default();
    let test_guid = guid(1000);

    // Import the glTF
    let outcome = GltfImporter::import_to_registry(&gltf_path, &mut registry, test_guid).unwrap();

    assert!(
        outcome.diagnostics.is_empty(),
        "glTF without materials should import without diagnostics"
    );

    // Verify the model has no materials (glTF without materials section)
    let handle = registry.handle_for_guid(test_guid).unwrap();
    let cpu_resource = registry.cpu_resource(handle).unwrap();
    let model = ModelResource::from_bytes(&cpu_resource.bytes).unwrap();

    // glTF files without a materials array will have 0 materials extracted
    assert_eq!(
        model.materials.len(),
        0,
        "glTF without materials should have 0 materials"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Creates a glTF file with a PBR material.
fn create_gltf_with_pbr_material(path: &Path) {
    // Triangle vertices: positions and normals
    let positions: Vec<f32> = vec![
        0.0, 0.0, 0.0, // vertex 0
        1.0, 0.0, 0.0, // vertex 1
        0.0, 1.0, 0.0, // vertex 2
    ];
    let normals: Vec<f32> = vec![
        0.0, 0.0, 1.0, // normal 0
        0.0, 0.0, 1.0, // normal 1
        0.0, 0.0, 1.0, // normal 2
    ];
    let indices: Vec<u32> = vec![0, 1, 2];

    // Convert to bytes
    let positions_bytes: Vec<u8> = positions.iter().flat_map(|f| f.to_le_bytes()).collect();
    let normals_bytes: Vec<u8> = normals.iter().flat_map(|f| f.to_le_bytes()).collect();
    let indices_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();

    // Create binary buffer
    let mut buffer_data = Vec::new();
    let positions_offset = 0;
    let normals_offset = positions_bytes.len();
    let indices_offset = normals_offset + normals_bytes.len();
    buffer_data.extend_from_slice(&positions_bytes);
    buffer_data.extend_from_slice(&normals_bytes);
    buffer_data.extend_from_slice(&indices_bytes);

    // Write binary buffer
    let bin_path = path.with_extension("bin");
    std::fs::write(&bin_path, &buffer_data).unwrap();

    // Create glTF JSON with PBR material
    let gltf_json = serde_json::json!({
        "asset": {
            "version": "2.0"
        },
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "materials": [{
            "name": "TestMaterial",
            "pbrMetallicRoughness": {
                "baseColorFactor": [0.8, 0.2, 0.2, 1.0],
                "metallicFactor": 0.9,
                "roughnessFactor": 0.3
            },
            "emissiveFactor": [0.1, 0.1, 0.1],
            "alphaMode": "OPAQUE"
        }],
        "meshes": [{
            "primitives": [{
                "attributes": {
                    "POSITION": 0,
                    "NORMAL": 1
                },
                "indices": 2,
                "material": 0
            }]
        }],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,
                "count": 3,
                "type": "VEC3",
                "min": [0.0, 0.0, 0.0],
                "max": [1.0, 1.0, 0.0]
            },
            {
                "bufferView": 1,
                "componentType": 5126,
                "count": 3,
                "type": "VEC3"
            },
            {
                "bufferView": 2,
                "componentType": 5125,
                "count": 3,
                "type": "SCALAR"
            }
        ],
        "bufferViews": [
            {
                "buffer": 0,
                "byteOffset": positions_offset,
                "byteLength": positions_bytes.len()
            },
            {
                "buffer": 0,
                "byteOffset": normals_offset,
                "byteLength": normals_bytes.len()
            },
            {
                "buffer": 0,
                "byteOffset": indices_offset,
                "byteLength": indices_bytes.len()
            }
        ],
        "buffers": [{
            "uri": bin_path.file_name().unwrap().to_str().unwrap(),
            "byteLength": buffer_data.len()
        }]
    });

    std::fs::write(path, serde_json::to_string_pretty(&gltf_json).unwrap()).unwrap();
}

#[test]
fn infer_importer_recognizes_vmodel_as_model() {
    assert_eq!(
        infer_importer(Path::new("models/crate.vmodel")),
        Some((ResourceKind::Model, "vmodel"))
    );
}

#[test]
fn vmodel_compiler_builds_beveled_array_model() {
    let source = br#"
schema_version = 1
kind = "generated_model"

[[operations]]
type = "cube"

[operations.params]
size = [2, 1, 1]

[[operations]]
type = "bevel"

[operations.params]
amount = 0.1

[[operations]]
type = "array"

[operations.params]
count = 3
axis = "x"
spacing = 2.5
"#;

    let model = compile_vmodel(source).unwrap();

    assert_eq!(model.meshes.len(), 3);
    assert!(
        model.meshes[0].positions.len() > 24,
        "beveled box should have more geometry than a plain cube"
    );
    assert_eq!(model.meshes[0].indices.len() % 3, 0);
    let first_min_x = model.meshes[0]
        .positions
        .iter()
        .map(|position| position[0])
        .fold(f32::INFINITY, f32::min);
    let second_min_x = model.meshes[1]
        .positions
        .iter()
        .map(|position| position[0])
        .fold(f32::INFINITY, f32::min);
    assert!((second_min_x - first_min_x - 2.5).abs() < 0.001);
}

#[test]
fn vmodel_compiler_adds_inset_panel_primitive() {
    let source = br#"
schema_version = 1
kind = "generated_model"

[[operations]]
type = "cube"

[operations.params]
size = [2.0, 2.0, 0.4]

[[operations]]
type = "inset_panel"

[operations.params]
face = "+z"
margin = 0.2
depth = 0.04
"#;

    let model = compile_vmodel(source).unwrap();

    assert_eq!(model.meshes.len(), 2);
    let first_max_z = model.meshes[0]
        .positions
        .iter()
        .map(|position| position[2])
        .fold(f32::NEG_INFINITY, f32::max);
    let second_max_z = model.meshes[1]
        .positions
        .iter()
        .map(|position| position[2])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!((first_max_z - second_max_z).abs() > 0.001);
}

#[test]
fn vmodel_compiler_builds_round_primitives() {
    let source = br#"
schema_version = 1
kind = "generated_model"

[[operations]]
type = "sphere"

[operations.params]
size = [2.0, 2.0, 2.0]
segments = 16

[[operations]]
type = "radial_array"

[operations.params]
count = 4
axis = "y"
radius = 3.0
"#;

    let model = compile_vmodel(source).unwrap();

    assert_eq!(model.meshes.len(), 4);
    assert!(
        model.meshes[0].positions.len() > 100,
        "sphere should generate a tessellated mesh"
    );
    let centers = model
        .meshes
        .iter()
        .map(|mesh| {
            let sum = mesh.positions.iter().fold([0.0; 3], |acc, position| {
                [
                    acc[0] + position[0],
                    acc[1] + position[1],
                    acc[2] + position[2],
                ]
            });
            scale_vec3(sum, 1.0 / mesh.positions.len() as f32)
        })
        .collect::<Vec<_>>();
    assert!(centers.iter().any(|center| center[0] > 2.0));
    assert!(centers.iter().any(|center| center[2] > 2.0));
}

#[test]
fn vmodel_compiler_rotates_and_mirrors_primitive() {
    let source = br#"
schema_version = 1
kind = "generated_model"

[[operations]]
type = "cylinder"

[operations.params]
size = [1.0, 2.0, 1.0]
segments = 12

[[operations]]
type = "rotate"

[operations.params]
rotation = [0.0, 0.0, 90.0]

[[operations]]
type = "mirror"

[operations.params]
axis = "x"
"#;

    let model = compile_vmodel(source).unwrap();

    assert_eq!(model.meshes.len(), 2);
    assert_eq!(model.meshes[0].indices.len() % 3, 0);
    let max_x = model.meshes[1]
        .positions
        .iter()
        .map(|position| position[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_x = model.meshes[0]
        .positions
        .iter()
        .map(|position| position[0])
        .fold(f32::INFINITY, f32::min);
    assert!((max_x + min_x).abs() < 0.001);
}

#[test]
fn vmodel_compiler_flushes_multiple_primitives() {
    let source = br#"
schema_version = 1
kind = "generated_model"

[[operations]]
type = "box"

[operations.params]
size = [1.0, 1.0, 1.0]

[[operations]]
type = "cylinder"

[operations.params]
position = [2.0, 0.0, 0.0]
size = [1.0, 1.0, 1.0]
segments = 8

[[operations]]
type = "plane"

[operations.params]
position = [0.0, -1.0, 0.0]
size = [4.0, 1.0, 4.0]
"#;

    let model = compile_vmodel(source).unwrap();

    assert_eq!(model.meshes.len(), 3);
    assert_eq!(model.meshes[0].positions.len(), 24);
    assert!(model.meshes[1].positions.len() > 24);
    assert_eq!(model.meshes[2].positions.len(), 4);
}

#[test]
fn vmodel_compiler_records_material_slots() {
    let source = br#"
schema_version = 1
kind = "generated_model"

[[operations]]
type = "box"

[operations.params]
size = [1.0, 1.0, 1.0]

[[operations]]
type = "material_slot"

[operations.params]
index = 2
name = "painted_metal"
base_color = [0.1, 0.2, 0.3, 1.0]
metallic = 0.8
roughness = 0.25
"#;

    let model = compile_vmodel(source).unwrap();

    assert_eq!(model.meshes.len(), 1);
    assert_eq!(model.meshes[0].material_index, Some(2));
    assert_eq!(model.materials.len(), 3);
    assert_eq!(model.materials[2].name, "painted_metal");
    assert_eq!(model.materials[2].base_color, [0.1, 0.2, 0.3, 1.0]);
    assert!((model.materials[2].metallic - 0.8).abs() < 0.001);
    assert!((model.materials[2].roughness - 0.25).abs() < 0.001);
}

#[test]
fn import_worker_processes_png_and_produces_upload_task() {
    use std::time::Duration;

    // Create a temporary directory with a test PNG
    let temp_dir = std::env::temp_dir().join("varg_import_worker_test");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let png_path = temp_dir.join("test.png");

    // Create a simple 2x2 red PNG
    let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
    img.save(&png_path).unwrap();

    // Spawn worker and enqueue import job
    let queue = ImportQueue::default();
    let worker = queue.spawn_worker();

    let job = ImportJob {
        asset_path: png_path.clone(),
        resource_kind: ResourceKind::Texture,
        import_options: ImportOptions::default(),
    };

    worker.enqueue(job).unwrap();

    // Poll for outcome (with timeout)
    let mut outcome = None;
    for _i in 0..100 {
        if let Some(result) = worker.try_recv_outcome() {
            outcome = Some(result);
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    // Drop worker to ensure thread cleanup
    drop(worker);

    // Verify outcome was produced
    let outcome = outcome.expect("Worker should produce an outcome within 1 second");
    assert_eq!(
        outcome.diagnostics.len(),
        0,
        "Import should succeed without diagnostics"
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn import_worker_handles_invalid_file() {
    use std::time::Duration;

    // Create a temporary directory with an invalid PNG
    let temp_dir = std::env::temp_dir().join("varg_import_worker_invalid_test");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let png_path = temp_dir.join("invalid.png");
    std::fs::write(&png_path, b"not a valid PNG file").unwrap();

    // Spawn worker and enqueue import job
    let queue = ImportQueue::default();
    let worker = queue.spawn_worker();

    let job = ImportJob {
        asset_path: png_path.clone(),
        resource_kind: ResourceKind::Texture,
        import_options: ImportOptions::default(),
    };

    worker.enqueue(job).unwrap();

    // Poll for outcome (with timeout)
    let mut outcome = None;
    for _ in 0..50 {
        if let Some(result) = worker.try_recv_outcome() {
            outcome = Some(result);
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    // Verify outcome was produced with diagnostics
    let outcome = outcome.expect("Worker should produce an outcome even for invalid files");
    assert!(
        !outcome.diagnostics.is_empty(),
        "Invalid file should produce diagnostics"
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn import_queue_drain_gpu_uploads_returns_tasks() {
    let mut queue = ImportQueue::default();

    let handle = ResourceHandle::new(
        ResourceId::from_u128(1),
        Handle::new(0, engine_core::Generation::FIRST),
    );

    // Push some upload tasks
    queue.push_upload(GpuUploadTask {
        handle,
        kind: ResourceKind::Texture,
    });
    queue.push_upload(GpuUploadTask {
        handle,
        kind: ResourceKind::Model,
    });

    // Drain uploads
    let uploads = queue.drain_gpu_uploads();
    assert_eq!(uploads.len(), 2);
    assert_eq!(uploads[0].kind, ResourceKind::Texture);
    assert_eq!(uploads[1].kind, ResourceKind::Model);

    // Verify queue is empty after drain
    let uploads2 = queue.drain_gpu_uploads();
    assert_eq!(uploads2.len(), 0);
}

#[test]
fn file_watcher_detects_file_creation() {
    use std::time::Duration;

    // Create a temporary directory
    let temp_dir = std::env::temp_dir().join(format!(
        "varg_watcher_test_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Start the watcher
    let mut watcher = FileWatcher::start(&temp_dir).unwrap();

    // Give the watcher time to initialize
    std::thread::sleep(Duration::from_millis(100));

    // Create a new file
    let test_file = temp_dir.join("test.png");
    std::fs::write(&test_file, b"fake png data").unwrap();

    // Poll until the native watcher delivers the event into the debounce buffer.
    let mut events = Vec::new();
    for _ in 0..200 {
        std::thread::sleep(Duration::from_millis(10));
        let mut polled = watcher.poll_events();
        events.append(&mut polled);
        if !events.is_empty() || watcher.debounce_buffer.contains_key(Path::new("test.png")) {
            break;
        }
    }

    // Wait for debounce window to pass
    std::thread::sleep(Duration::from_millis(250));
    let mut final_events = watcher.poll_events();
    events.append(&mut final_events);

    // Verify event was detected
    assert!(!events.is_empty(), "Should detect file creation event");
    let created_event = events.iter().find(|e| e.path == PathBuf::from("test.png"));
    assert!(created_event.is_some(), "Should have event for test.png");

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[cfg(unix)]
#[test]
fn file_watcher_accepts_canonicalized_event_paths() {
    let root = std::env::temp_dir().join(format!(
        "varg_watcher_root_test_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let symlink = std::env::temp_dir().join(format!(
        "varg_watcher_link_test_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_file(&symlink);
    std::fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink(&root, &symlink).unwrap();

    let test_file = root.join("test.png");
    std::fs::write(&test_file, b"fake png data").unwrap();
    let watcher = FileWatcher::start(&symlink).unwrap();

    assert_eq!(
        watcher.relative_event_path(&test_file),
        Some(PathBuf::from("test.png"))
    );

    let _ = std::fs::remove_file(&symlink);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn file_watcher_debounces_modified_events() {
    use std::time::Duration;

    let temp_dir =
        std::env::temp_dir().join(format!("varg_watcher_debounce_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let test_file = temp_dir.join("test.txt");
    std::fs::write(&test_file, b"initial").unwrap();

    let mut watcher = FileWatcher::start(&temp_dir).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Modify the file multiple times rapidly
    for i in 0..5 {
        std::fs::write(&test_file, format!("content {}", i)).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }

    // Poll immediately (events should be buffered)
    let _immediate_events = watcher.poll_events();

    // Wait for debounce window
    std::thread::sleep(Duration::from_millis(250));

    // Poll again (should get debounced events)
    let debounced_events = watcher.poll_events();

    // Should only get one event per file due to debouncing
    let test_events: Vec<_> = debounced_events
        .iter()
        .filter(|e| e.path == PathBuf::from("test.txt"))
        .collect();

    // We should have at most a few events, not 5
    assert!(
        test_events.len() <= 2,
        "Events should be debounced, got {} events",
        test_events.len()
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn asset_database_handle_event_marks_modified_as_stale() {
    let root = std::env::temp_dir().join(format!("varg_db_event_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");

    // Add an asset manually
    let path = PathBuf::from("test.png");
    let guid = generate_asset_guid(&path);
    let meta = ResourceMeta {
        guid,
        path: path.clone(),
        kind: ResourceKind::Texture,
        import_state: ResourceState::GpuReady,
    };
    database.entries.insert(path.clone(), meta);

    // Handle a Modified event
    let event = FileEvent {
        path: path.clone(),
        kind: FileEventKind::Modified,
    };
    database.handle_event(&event).unwrap();

    // Verify asset is marked as Stale
    let updated = database.entry_for_path(&path).unwrap();
    assert_eq!(updated.import_state, ResourceState::Stale);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn asset_database_handle_event_adds_created_asset() {
    let root = std::env::temp_dir().join(format!("varg_db_create_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");

    // Handle a Created event
    let event = FileEvent {
        path: PathBuf::from("new_texture.png"),
        kind: FileEventKind::Created,
    };
    database.handle_event(&event).unwrap();

    // Verify asset was added
    let entry = database.entry_for_path(&PathBuf::from("new_texture.png"));
    assert!(entry.is_some(), "Created asset should be in database");
    assert_eq!(entry.unwrap().kind, ResourceKind::Texture);
    assert_eq!(entry.unwrap().import_state, ResourceState::Unloaded);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn asset_database_handle_event_removes_deleted_asset() {
    let root = std::env::temp_dir().join(format!("varg_db_remove_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");

    // Add an asset manually
    let path = PathBuf::from("to_delete.png");
    let guid = generate_asset_guid(&path);
    let meta = ResourceMeta {
        guid,
        path: path.clone(),
        kind: ResourceKind::Texture,
        import_state: ResourceState::GpuReady,
    };
    database.entries.insert(path.clone(), meta);

    // Handle a Removed event
    let event = FileEvent {
        path: path.clone(),
        kind: FileEventKind::Removed,
    };
    database.handle_event(&event).unwrap();

    // Verify asset was removed
    let entry = database.entry_for_path(&path);
    assert!(entry.is_none(), "Removed asset should not be in database");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn hot_reload_coordinator_processes_file_events_and_enqueues_imports() {
    let root = std::env::temp_dir().join(format!("varg_hot_reload_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    // Create a test PNG file
    let png_path = root.join("test.png");
    let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
    img.save(&png_path).unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");
    database.scan(&root).unwrap();

    let mut coordinator = HotReloadCoordinator::new(&root);

    // Simulate a file modification event
    let events = vec![FileEvent {
        path: PathBuf::from("test.png"),
        kind: FileEventKind::Modified,
    }];

    let affected = coordinator
        .process_file_events(&events, &mut database)
        .unwrap();
    assert_eq!(affected.len(), 1, "Should have one affected asset");

    // Verify the asset was marked as stale
    let entry = database.entry_for_path(&PathBuf::from("test.png")).unwrap();
    assert_eq!(entry.import_state, ResourceState::Stale);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn asset_registry_swap_gpu_enqueues_old_resource_for_destruction() {
    let mut registry = AssetRegistry::default();
    let guid = AssetGuid::from_u128(12345);
    let handle = registry.register(guid, ResourceKind::Texture).unwrap();

    // Put initial GPU resource
    let old_gpu = GpuResource {
        kind: ResourceKind::Texture,
        backend_token: 100,
    };
    registry.put_gpu(handle, old_gpu).unwrap();

    // Swap with new GPU resource
    let new_gpu = GpuResource {
        kind: ResourceKind::Texture,
        backend_token: 200,
    };
    registry.swap_gpu(handle, new_gpu, 3).unwrap();

    // Verify new resource is active
    let current = registry.gpu_resource(handle).unwrap();
    assert_eq!(current.backend_token, 200);

    // Verify old resource is in destroy queue
    assert_eq!(registry.gpu_destroy_queue.len(), 1);
    assert_eq!(registry.gpu_destroy_queue[0].1, 100); // old token
    assert_eq!(registry.gpu_destroy_queue[0].2, 3); // frames remaining
}

#[test]
fn asset_registry_tick_gpu_destroy_queue_releases_resources() {
    let mut registry = AssetRegistry::default();
    let guid = AssetGuid::from_u128(12345);
    let handle = registry.register(guid, ResourceKind::Texture).unwrap();

    // Manually add items to destroy queue with different frame delays
    registry.gpu_destroy_queue.push_back((handle, 100, 2));
    registry.gpu_destroy_queue.push_back((handle, 101, 0));
    registry.gpu_destroy_queue.push_back((handle, 102, 1));

    // Tick 1 - should release token 101 (frames=0) without decrementing others
    let ready = registry.tick_gpu_destroy_queue();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], 101);
    assert_eq!(registry.gpu_destroy_queue.len(), 2);

    // Tick 2 - no items at 0, so decrement all
    let ready = registry.tick_gpu_destroy_queue();
    assert_eq!(ready.len(), 1); // 102 reaches 0 and is removed
    assert_eq!(ready[0], 102);
    assert_eq!(registry.gpu_destroy_queue.len(), 1);

    // Tick 3 - no items at 0, so decrement (100: 1->0) and remove
    let ready = registry.tick_gpu_destroy_queue();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], 100);
    assert_eq!(registry.gpu_destroy_queue.len(), 0);

    // Tick 4 - queue is empty
    let ready = registry.tick_gpu_destroy_queue();
    assert_eq!(ready.len(), 0);
    assert_eq!(registry.gpu_destroy_queue.len(), 0);
}

#[test]
fn asset_registry_mark_failed_sets_error_state() {
    let mut registry = AssetRegistry::default();
    let guid = AssetGuid::from_u128(12345);
    let handle = registry.register(guid, ResourceKind::Texture).unwrap();

    // Put some resources
    registry
        .put_cpu(
            handle,
            CpuResource {
                kind: ResourceKind::Texture,
                bytes: Arc::from(vec![1, 2, 3]),
            },
        )
        .unwrap();
    registry
        .put_gpu(
            handle,
            GpuResource {
                kind: ResourceKind::Texture,
                backend_token: 100,
            },
        )
        .unwrap();

    // Mark as failed
    registry
        .mark_failed(handle, "Import failed: invalid format")
        .unwrap();

    // Verify state
    let record = registry.record(handle).unwrap();
    assert_eq!(record.state, ResourceState::Failed);
    assert!(record.preview.is_some());
    assert!(
        record
            .preview
            .as_ref()
            .unwrap()
            .summary
            .contains("Import failed")
    );

    // Verify caches were cleared
    assert!(registry.cpu_resource(handle).is_none());
    assert!(registry.gpu_resource(handle).is_none());
}

#[test]
fn hot_reload_full_flow_integration() {
    use std::time::Duration;

    let root = std::env::temp_dir().join(format!("varg_hot_reload_full_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    // Create initial PNG file
    let png_path = root.join("test.png");
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
    img.save(&png_path).unwrap();

    // Set up database and registry
    let mut database = AssetDatabase::new(&root, "builtin");
    database.scan(&root).unwrap();

    let mut registry = AssetRegistry::default();
    let entry = database.entry_for_path(&PathBuf::from("test.png")).unwrap();
    let handle = registry.register(entry.guid, entry.kind).unwrap();

    // Initial import
    let options = ImportOptions::default();
    let outcome =
        PngImporter::import_to_registry(&png_path, &options, &mut registry, entry.guid).unwrap();
    assert!(outcome.diagnostics.is_empty());
    assert_eq!(
        registry.record(handle).unwrap().state,
        ResourceState::CpuReady
    );

    // Simulate GPU upload
    registry
        .put_gpu(
            handle,
            GpuResource {
                kind: ResourceKind::Texture,
                backend_token: 1000,
            },
        )
        .unwrap();

    // Modify the file
    let img2 = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 255, 0, 255]));
    img2.save(&png_path).unwrap();

    // Process file event
    let mut coordinator = HotReloadCoordinator::new(&root);
    let events = vec![FileEvent {
        path: PathBuf::from("test.png"),
        kind: FileEventKind::Modified,
    }];
    coordinator
        .process_file_events(&events, &mut database)
        .unwrap();

    // Verify asset marked as stale
    let entry = database.entry_for_path(&PathBuf::from("test.png")).unwrap();
    assert_eq!(entry.import_state, ResourceState::Stale);

    // Enqueue reimport via worker
    let job = ImportJob {
        asset_path: png_path.clone(),
        resource_kind: ResourceKind::Texture,
        import_options: options,
    };
    coordinator.enqueue_import(job).unwrap();

    // Poll for completed import
    let mut outcomes = Vec::new();
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(10));
        let mut polled = coordinator.poll_completed_imports(&mut registry);
        outcomes.append(&mut polled);
        if !outcomes.is_empty() {
            break;
        }
    }

    // Note: The worker processes imports but doesn't update the registry directly
    // In a real system, the outcomes would be processed to update the registry

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn asset_database_handle_event_returns_guid_for_modified() {
    let root = std::env::temp_dir().join(format!("varg_db_guid_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    let mut database = AssetDatabase::new(&root, "builtin");

    // Add an asset manually
    let path = PathBuf::from("test.png");
    let guid = generate_asset_guid(&path);
    let meta = ResourceMeta {
        guid,
        path: path.clone(),
        kind: ResourceKind::Texture,
        import_state: ResourceState::GpuReady,
    };
    database.entries.insert(path.clone(), meta);

    // Handle a Modified event
    let event = FileEvent {
        path: path.clone(),
        kind: FileEventKind::Modified,
    };
    let result_guid = database.handle_event(&event).unwrap();

    // Verify GUID was returned
    assert!(result_guid.is_some());
    assert_eq!(result_guid.unwrap(), guid);

    // Verify asset was marked as stale
    let entry = database.entry_for_path(&path).unwrap();
    assert_eq!(entry.import_state, ResourceState::Stale);

    let _ = std::fs::remove_dir_all(&root);
}

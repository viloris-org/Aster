use super::*;

/// Extracts the active scene into a minimal render queue.
pub fn extract_render_world(scene: &Scene) -> RenderWorld {
    RenderWorld::extract(scene)
}

/// Parses an asset GUID from a mesh/material name string formatted as
/// `"asset:{:032x}"` or `"asset:{:032x}:N"` (multi-mesh model suffix).
pub(crate) fn parse_asset_guid(name: &str) -> Option<engine_core::AssetId> {
    let hex = name.strip_prefix("asset:")?.split(':').next()?;
    let value = u128::from_str_radix(hex, 16).ok()?;
    Some(engine_core::AssetId::from_u128(value))
}

fn parse_asset_mesh_index(name: &str) -> Option<usize> {
    name.strip_prefix("asset:")?.split(':').nth(1)?.parse().ok()
}

pub(crate) fn model_material_index_for_mesh(
    mesh_name: &str,
    model: &ModelResource,
) -> Option<usize> {
    let mesh_index = parse_asset_mesh_index(mesh_name).unwrap_or(0);
    model.meshes.get(mesh_index)?.material_index
}

#[cfg(feature = "asset-import")]
pub(crate) fn resolve_model_texture_ref(model_source_path: &Path, texture_ref: &str) -> PathBuf {
    let texture_path = Path::new(texture_ref);
    let joined = if texture_path.is_absolute() {
        texture_path.to_path_buf()
    } else {
        model_source_path.parent().map_or_else(
            || texture_path.to_path_buf(),
            |parent| parent.join(texture_path),
        )
    };
    normalize_relative_path(&joined)
}

#[cfg(feature = "asset-import")]
fn normalize_relative_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

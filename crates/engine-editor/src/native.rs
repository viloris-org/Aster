//! Native editor services: picking, gizmo operations, outline highlight, and previews.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ── Picking ───────────────────────────────────────────────────────────────────

/// A screen-space pick request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickRequest {
    /// Normalized device X coordinate in `[-1, 1]`.
    pub ndc_x: f32,
    /// Normalized device Y coordinate in `[-1, 1]`.
    pub ndc_y: f32,
}

/// Result of a scene pick operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PickResult {
    /// An entity was hit; carries its stable string id.
    Entity(String),
    /// Nothing was hit.
    Miss,
}

/// Service that maps screen-space clicks to scene entities.
#[derive(Clone, Debug, Default)]
pub struct PickingService {
    last_result: Option<PickResult>,
}

impl PickingService {
    /// Records a pick result (called by the renderer or scene view).
    pub fn record(&mut self, result: PickResult) {
        self.last_result = Some(result);
    }

    /// Returns the most recent pick result.
    pub fn last(&self) -> Option<&PickResult> {
        self.last_result.as_ref()
    }

    /// Clears the stored result.
    pub fn clear(&mut self) {
        self.last_result = None;
    }
}

// ── Gizmo ─────────────────────────────────────────────────────────────────────

/// Active gizmo operation mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GizmoOperation {
    /// Translate along an axis or plane.
    #[default]
    Translate,
    /// Rotate around an axis.
    Rotate,
    /// Scale along an axis or uniformly.
    Scale,
}

/// Gizmo coordinate space.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GizmoSpace {
    /// World-space axes.
    #[default]
    World,
    /// Object-local axes.
    Local,
}

/// Gizmo service that tracks the active operation and space.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GizmoService {
    /// Active operation.
    pub operation: GizmoOperation,
    /// Active coordinate space.
    pub space: GizmoSpace,
    /// Entity currently being manipulated (stable id).
    pub active_entity: Option<String>,
}

impl GizmoService {
    /// Begins a gizmo interaction on an entity.
    pub fn begin(&mut self, entity: impl Into<String>, operation: GizmoOperation) {
        self.active_entity = Some(entity.into());
        self.operation = operation;
    }

    /// Ends the current gizmo interaction.
    pub fn end(&mut self) {
        self.active_entity = None;
    }
}

// ── Outline highlight ─────────────────────────────────────────────────────────

/// Outline highlight entry for a single entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutlineEntry {
    /// Stable entity id.
    pub entity_id: String,
    /// RGBA hex color string (e.g. `"#4a9eff"`).
    pub color: String,
}

/// Service that tracks which entities should be outlined in the scene view.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OutlineService {
    entries: Vec<OutlineEntry>,
}

impl OutlineService {
    /// Adds or replaces an outline entry for an entity.
    pub fn set(&mut self, entity_id: impl Into<String>, color: impl Into<String>) {
        let id = entity_id.into();
        self.entries.retain(|e| e.entity_id != id);
        self.entries.push(OutlineEntry {
            entity_id: id,
            color: color.into(),
        });
    }

    /// Removes the outline for an entity.
    pub fn remove(&mut self, entity_id: &str) {
        self.entries.retain(|e| e.entity_id != entity_id);
    }

    /// Clears all outlines.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns all active outline entries.
    pub fn entries(&self) -> &[OutlineEntry] {
        &self.entries
    }
}

// ── Preview ───────────────────────────────────────────────────────────────────

/// Kind of asset being previewed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewKind {
    /// Texture or image asset.
    Resource,
    /// Material asset.
    Material,
    /// Mesh asset.
    Mesh,
}

/// A preview request submitted to the preview service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewRequest {
    /// Asset path.
    pub path: PathBuf,
    /// Kind of preview.
    pub kind: PreviewKind,
    /// Desired thumbnail size in pixels.
    pub size: u32,
}

/// State of a preview thumbnail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreviewState {
    /// Thumbnail is being generated.
    Pending,
    /// Thumbnail is ready; carries an opaque GPU texture id.
    Ready(u64),
    /// Thumbnail generation failed.
    Failed(String),
}

/// Service that manages asset preview thumbnail requests and results.
#[derive(Clone, Debug, Default)]
pub struct PreviewService {
    entries: Vec<(PreviewRequest, PreviewState)>,
}

impl PreviewService {
    /// Submits a preview request. Duplicate paths are ignored.
    pub fn request(&mut self, req: PreviewRequest) {
        if !self.entries.iter().any(|(r, _)| r.path == req.path) {
            self.entries.push((req, PreviewState::Pending));
        }
    }

    /// Records a completed thumbnail for a path.
    pub fn complete(&mut self, path: &std::path::Path, texture_id: u64) {
        for (req, state) in &mut self.entries {
            if req.path == path {
                *state = PreviewState::Ready(texture_id);
                return;
            }
        }
    }

    /// Records a failed thumbnail for a path.
    pub fn fail(&mut self, path: &std::path::Path, reason: impl Into<String>) {
        for (req, state) in &mut self.entries {
            if req.path == path {
                *state = PreviewState::Failed(reason.into());
                return;
            }
        }
    }

    /// Returns the state for a path, if any.
    pub fn state(&self, path: &std::path::Path) -> Option<&PreviewState> {
        self.entries
            .iter()
            .find(|(req, _)| req.path == path)
            .map(|(_, state)| state)
    }

    /// Returns all pending requests (for the renderer to process).
    pub fn pending(&self) -> impl Iterator<Item = &PreviewRequest> {
        self.entries
            .iter()
            .filter(|(_, state)| *state == PreviewState::Pending)
            .map(|(req, _)| req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn picking_service_records_and_clears() {
        let mut svc = PickingService::default();
        svc.record(PickResult::Entity("player".into()));
        assert_eq!(svc.last(), Some(&PickResult::Entity("player".into())));
        svc.clear();
        assert!(svc.last().is_none());
    }

    #[test]
    fn gizmo_service_begin_end() {
        let mut svc = GizmoService::default();
        svc.begin("cube", GizmoOperation::Rotate);
        assert_eq!(svc.active_entity.as_deref(), Some("cube"));
        svc.end();
        assert!(svc.active_entity.is_none());
    }

    #[test]
    fn outline_service_set_remove() {
        let mut svc = OutlineService::default();
        svc.set("a", "#ff0000");
        svc.set("b", "#00ff00");
        assert_eq!(svc.entries().len(), 2);
        svc.remove("a");
        assert_eq!(svc.entries().len(), 1);
        assert_eq!(svc.entries()[0].entity_id, "b");
    }

    #[test]
    fn outline_service_set_replaces_existing() {
        let mut svc = OutlineService::default();
        svc.set("a", "#ff0000");
        svc.set("a", "#0000ff");
        assert_eq!(svc.entries().len(), 1);
        assert_eq!(svc.entries()[0].color, "#0000ff");
    }

    #[test]
    fn preview_service_request_and_complete() {
        let mut svc = PreviewService::default();
        let path = PathBuf::from("assets/cube.mesh");
        svc.request(PreviewRequest {
            path: path.clone(),
            kind: PreviewKind::Mesh,
            size: 128,
        });
        assert_eq!(svc.state(&path), Some(&PreviewState::Pending));
        svc.complete(&path, 42);
        assert_eq!(svc.state(&path), Some(&PreviewState::Ready(42)));
    }

    #[test]
    fn preview_service_deduplicates_requests() {
        let mut svc = PreviewService::default();
        let path = PathBuf::from("assets/mat.mat");
        let req = PreviewRequest {
            path: path.clone(),
            kind: PreviewKind::Material,
            size: 64,
        };
        svc.request(req.clone());
        svc.request(req);
        assert_eq!(svc.pending().count(), 1);
    }
}

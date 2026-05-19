//! Editor render integration: Scene View and Game View offscreen targets,
//! GUI draw list submission, and preview render requests.

use engine_core::EngineResult;
use engine_render::{
    GuiDrawList, RenderDevice, RenderFrame, RenderGraph, RenderGraphBuilder, RenderTarget,
    RenderTargetDesc, ViewKind,
};

/// Manages the two editor offscreen render targets and the GUI draw list.
pub struct EditorRenderer<R: RenderDevice> {
    device: R,
    scene_view: Option<RenderTarget>,
    game_view: Option<RenderTarget>,
    render_graph: RenderGraph,
}

impl<R: RenderDevice> EditorRenderer<R> {
    /// Creates an editor renderer wrapping the given device.
    pub fn new(device: R) -> Self {
        let render_graph = build_editor_render_graph();
        Self {
            device,
            scene_view: None,
            game_view: None,
            render_graph,
        }
    }

    /// Allocates or resizes the Scene View render target.
    pub fn resize_scene_view(&mut self, width: u32, height: u32) -> EngineResult<()> {
        if let Some(old) = self.scene_view.take() {
            self.device.destroy_render_target(old);
        }
        let target = self.device.create_render_target(RenderTargetDesc::view(
            width,
            height,
            ViewKind::SceneView,
        ))?;
        self.scene_view = Some(target);
        Ok(())
    }

    /// Allocates or resizes the Game View render target.
    pub fn resize_game_view(&mut self, width: u32, height: u32) -> EngineResult<()> {
        if let Some(old) = self.game_view.take() {
            self.device.destroy_render_target(old);
        }
        let target = self.device.create_render_target(RenderTargetDesc::view(
            width,
            height,
            ViewKind::GameView,
        ))?;
        self.game_view = Some(target);
        Ok(())
    }

    /// Renders one editor frame: executes the graph then submits the GUI draw list.
    pub fn render_frame(&mut self, frame: RenderFrame, gui: &GuiDrawList) -> EngineResult<()> {
        self.device.execute_graph(&self.render_graph, frame)?;
        self.device.draw_gui(gui)?;
        self.device
            .flush_destroy_queue(frame.frame_index.saturating_sub(2));
        Ok(())
    }

    /// Returns the scene view target, if allocated.
    pub fn scene_view(&self) -> Option<&RenderTarget> {
        self.scene_view.as_ref()
    }

    /// Returns the game view target, if allocated.
    pub fn game_view(&self) -> Option<&RenderTarget> {
        self.game_view.as_ref()
    }

    /// Replaces the active render graph.
    pub fn set_render_graph(&mut self, graph: RenderGraph) {
        self.render_graph = graph;
    }

    /// Returns a reference to the underlying device.
    pub fn device(&self) -> &R {
        &self.device
    }

    /// Returns a mutable reference to the underlying device.
    pub fn device_mut(&mut self) -> &mut R {
        &mut self.device
    }
}

/// Builds the default editor render graph (shadow → forward → outline → post → gui).
pub fn build_editor_render_graph() -> RenderGraph {
    let mut builder = RenderGraphBuilder::new();
    let shadow = builder.add_pass("shadow");
    let forward = builder.add_pass("forward");
    let outline = builder.add_pass("outline");
    let post = builder.add_pass("post");
    let gui = builder.add_pass("gui");
    builder.order_before(shadow, forward);
    builder.order_before(forward, outline);
    builder.order_before(outline, post);
    builder.order_before(post, gui);
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_render::HeadlessRenderDevice;

    #[test]
    fn editor_render_graph_has_five_passes_in_order() {
        let graph = build_editor_render_graph();
        assert_eq!(graph.pass_count(), 5);
        let names: Vec<&str> = graph.passes.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["shadow", "forward", "outline", "post", "gui"]);
    }

    #[test]
    fn editor_renderer_allocates_scene_and_game_views() {
        let device = HeadlessRenderDevice::default();
        let mut renderer = EditorRenderer::new(device);
        renderer.resize_scene_view(1280, 720).unwrap();
        renderer.resize_game_view(1280, 720).unwrap();
        assert_eq!(renderer.scene_view().unwrap().kind(), ViewKind::SceneView);
        assert_eq!(renderer.game_view().unwrap().kind(), ViewKind::GameView);
    }

    #[test]
    fn editor_renderer_renders_frame_with_empty_gui() {
        let device = HeadlessRenderDevice::default();
        let mut renderer = EditorRenderer::new(device);
        renderer
            .render_frame(RenderFrame { frame_index: 0 }, &GuiDrawList::default())
            .unwrap();
    }
}

# VargCraft Prototype Notes

This is a throwaway MC-like capability probe for Varg.

Question: can the current engine build a playable voxel-style loop before adding voxel-specific engine systems?

Current slice:

- First-person camera and player movement copied from the existing runtime patterns.
- Runtime script spawns a larger island with terraces, cave markers, trees, ore nodes, and temporary placed blocks.
- Fire removes the nearest tagged block within edit radius using bounds distance queries.
- Interact places grass, stone, or tree blocks from a HUD palette slider.
- Space collects nearby ore nodes spawned with `scene.spawnSphere` and a small bobbing script.
- Runtime weather drives time of day, cloud cover, precipitation, wind, storm preset toggling, and GI intensity.
- Procedural audio now covers ambience loops, mining, placing, failed edits, and ore collection.
- HUD uses labels, rectangles, toggles, and sliders so the example exercises interactive UI, not only debug text.

Known findings from the first pass:

- The engine can parse and load the project as a normal runtime project.
- Runtime script spawning works for a moderate entity-per-block field plus scripted dynamic entities.
- `else if` is not currently accepted by the Varg runtime parser; use nested `if` blocks.
- Large dynamic `while`-driven generation is not suitable for a smoke test. Keep this example small, then move real voxel generation into a Rust-side chunk/mesh system.

Likely next engine work:

- Runtime-generated chunk mesh API instead of entity-per-block rendering.
- Chunk-level collider generation instead of one collider per block.
- Block registry and chunk storage, probably in a dedicated `engine-voxel` or `engine-terrain` crate.
- Renderer instancing or batching path for repeated block geometry and materials.

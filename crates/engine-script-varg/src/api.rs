/// Registry module containing script-facing engine APIs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct VargScriptApiModule {
    /// Module name used at the script boundary.
    pub name: &'static str,
    /// Human-readable module summary.
    pub description: &'static str,
    /// API entries in this module.
    pub items: &'static [VargScriptApiItem],
}

/// One script-facing engine API entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct VargScriptApiItem {
    /// API name as used in script source.
    pub name: &'static str,
    /// Compact Varg-facing signature.
    pub signature: &'static str,
    /// API entry kind.
    pub kind: VargScriptApiKind,
    /// Human-readable API summary.
    pub description: &'static str,
}

/// Script-facing engine API entry kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum VargScriptApiKind {
    /// Callable function or method.
    Function,
    /// Read-only value available to expressions.
    Property,
    /// Statement-like side effect accepted by the MVP interpreter.
    Statement,
    /// Mutable script binding that writes to engine state.
    AssignmentTarget,
}

/// Returns the script-facing engine API registry used by diagnostics and tools.
pub fn varg_script_api_registry() -> &'static [VargScriptApiModule] {
    VARG_SCRIPT_API_REGISTRY
}

const INPUT_API: &[VargScriptApiItem] = &[
    VargScriptApiItem {
        name: "Input.value",
        signature: "Input.value(action: String) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Reads the current analog value for a named input action.",
    },
    VargScriptApiItem {
        name: "Input.actionValue",
        signature: "Input.actionValue(action: String) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Alias for reading a named input action value.",
    },
    VargScriptApiItem {
        name: "Input.axis",
        signature: "Input.axis(axis: String) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Reads a built-in movement axis such as moveX or moveY.",
    },
    VargScriptApiItem {
        name: "Input.down",
        signature: "Input.down(action: String) -> Bool",
        kind: VargScriptApiKind::Function,
        description: "Returns whether a named action is currently held.",
    },
    VargScriptApiItem {
        name: "Input.pressed",
        signature: "Input.pressed(action: String) -> Bool",
        kind: VargScriptApiKind::Function,
        description: "Returns whether a named action was pressed this frame.",
    },
    VargScriptApiItem {
        name: "Input.justPressed",
        signature: "Input.justPressed(action: String) -> Bool",
        kind: VargScriptApiKind::Function,
        description: "Returns whether a named action was pressed this frame.",
    },
    VargScriptApiItem {
        name: "Input.released",
        signature: "Input.released(action: String) -> Bool",
        kind: VargScriptApiKind::Function,
        description: "Returns whether a named action was released this frame.",
    },
    VargScriptApiItem {
        name: "Input.justReleased",
        signature: "Input.justReleased(action: String) -> Bool",
        kind: VargScriptApiKind::Function,
        description: "Returns whether a named action was released this frame.",
    },
    VargScriptApiItem {
        name: "Input.captureMouse",
        signature: "Input.captureMouse(captured: Bool = true)",
        kind: VargScriptApiKind::Statement,
        description: "Requests mouse capture for the game window.",
    },
    VargScriptApiItem {
        name: "Input.releaseMouse",
        signature: "Input.releaseMouse()",
        kind: VargScriptApiKind::Statement,
        description: "Requests mouse release for the game window.",
    },
    VargScriptApiItem {
        name: "Input.mouseDeltaX",
        signature: "Input.mouseDeltaX() -> Float",
        kind: VargScriptApiKind::Function,
        description: "Reads horizontal mouse movement for the current frame.",
    },
    VargScriptApiItem {
        name: "Input.mouseDeltaY",
        signature: "Input.mouseDeltaY() -> Float",
        kind: VargScriptApiKind::Function,
        description: "Reads vertical mouse movement for the current frame.",
    },
    VargScriptApiItem {
        name: "Input.cursorX",
        signature: "Input.cursorX -> Float",
        kind: VargScriptApiKind::Property,
        description: "Current pointer x position in screen space.",
    },
    VargScriptApiItem {
        name: "Input.cursorY",
        signature: "Input.cursorY -> Float",
        kind: VargScriptApiKind::Property,
        description: "Current pointer y position in screen space.",
    },
];

const TIME_API: &[VargScriptApiItem] = &[
    VargScriptApiItem {
        name: "Time.time",
        signature: "Time.time -> Float",
        kind: VargScriptApiKind::Property,
        description: "Total elapsed runtime time in seconds.",
    },
    VargScriptApiItem {
        name: "Time.delta",
        signature: "Time.delta -> Float",
        kind: VargScriptApiKind::Property,
        description: "Current lifecycle delta time in seconds.",
    },
    VargScriptApiItem {
        name: "Time.frame",
        signature: "Time.frame -> Float",
        kind: VargScriptApiKind::Property,
        description: "Current runtime frame index.",
    },
];

const ENTITY_API: &[VargScriptApiItem] = &[
    VargScriptApiItem {
        name: "entity.translate",
        signature: "entity.translate(delta: Vec3)",
        kind: VargScriptApiKind::Statement,
        description: "Adds a local-space translation to the owning entity.",
    },
    VargScriptApiItem {
        name: "entity.destroy",
        signature: "entity.destroy()",
        kind: VargScriptApiKind::Statement,
        description: "Requests deferred destruction of the owning entity.",
    },
    VargScriptApiItem {
        name: "entity.hasTag",
        signature: "entity.hasTag(tag: String) -> Bool",
        kind: VargScriptApiKind::Function,
        description: "Returns whether the owning entity has the requested tag.",
    },
    VargScriptApiItem {
        name: "position",
        signature: "position: Vec3",
        kind: VargScriptApiKind::AssignmentTarget,
        description: "Owning entity local position.",
    },
    VargScriptApiItem {
        name: "rotation",
        signature: "rotation: Vec3",
        kind: VargScriptApiKind::AssignmentTarget,
        description: "Owning entity local Euler rotation in degrees.",
    },
];

const SCENE_API: &[VargScriptApiItem] = &[
    VargScriptApiItem {
        name: "scene.spawnBox",
        signature: "scene.spawnBox(name: String, tag: String, position: Vec3, size: Vec3, script: String)",
        kind: VargScriptApiKind::Statement,
        description: "Requests creation of a primitive box scene object.",
    },
    VargScriptApiItem {
        name: "scene.spawnSphere",
        signature: "scene.spawnSphere(name: String, tag: String, position: Vec3, radius: Float, script: String)",
        kind: VargScriptApiKind::Statement,
        description: "Requests creation of a primitive sphere scene object.",
    },
    VargScriptApiItem {
        name: "scene.destroyNearestWithTag",
        signature: "scene.destroyNearestWithTag(tag: String, radius: Float)",
        kind: VargScriptApiKind::Statement,
        description: "Requests destruction of the nearest object with a tag inside a radius.",
    },
    VargScriptApiItem {
        name: "scene.distanceToTag",
        signature: "scene.distanceToTag(tag: String) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Distance from the owning entity to the nearest object with a tag.",
    },
    VargScriptApiItem {
        name: "scene.distanceToTagBounds",
        signature: "scene.distanceToTagBounds(tag: String) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Distance from the owning entity to nearest tagged bounds.",
    },
    VargScriptApiItem {
        name: "scene.horizontalDistanceToTagBounds",
        signature: "scene.horizontalDistanceToTagBounds(tag: String) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Horizontal distance from the owning entity to nearest tagged bounds.",
    },
    VargScriptApiItem {
        name: "scene.xOf",
        signature: "scene.xOf(name: String) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Reads the x position of a named object.",
    },
    VargScriptApiItem {
        name: "scene.yOf",
        signature: "scene.yOf(name: String) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Reads the y position of a named object.",
    },
    VargScriptApiItem {
        name: "scene.zOf",
        signature: "scene.zOf(name: String) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Reads the z position of a named object.",
    },
];

const AUDIO_API: &[VargScriptApiItem] = &[
    VargScriptApiItem {
        name: "Audio.playTone",
        signature: "Audio.playTone(waveform: String, frequency: Float, duration: Float, volume: Float)",
        kind: VargScriptApiKind::Statement,
        description: "Plays a transient procedural tone.",
    },
    VargScriptApiItem {
        name: "Audio.playTone3D",
        signature: "Audio.playTone3D(waveform: String, frequency: Float, duration: Float, volume: Float)",
        kind: VargScriptApiKind::Statement,
        description: "Plays a transient procedural tone at the owning entity position.",
    },
    VargScriptApiItem {
        name: "Audio.startLoop",
        signature: "Audio.startLoop(id: String, waveform: String, pattern: String, bpm: Float, beatsPerNote: Float, volume: Float)",
        kind: VargScriptApiKind::Statement,
        description: "Starts or updates a procedural audio loop.",
    },
    VargScriptApiItem {
        name: "Audio.stopLoop",
        signature: "Audio.stopLoop(id: String)",
        kind: VargScriptApiKind::Statement,
        description: "Stops a procedural audio loop.",
    },
];

const RENDER_API: &[VargScriptApiItem] = &[
    VargScriptApiItem {
        name: "render.gi.useScreenSpace",
        signature: "render.gi.useScreenSpace()",
        kind: VargScriptApiKind::Statement,
        description: "Requests screen-space global illumination.",
    },
    VargScriptApiItem {
        name: "render.gi.useProbeVolume",
        signature: "render.gi.useProbeVolume(center: Vec3, extent: Vec3, counts: Vec3, intensity: Float)",
        kind: VargScriptApiKind::Statement,
        description: "Requests probe-volume global illumination.",
    },
    VargScriptApiItem {
        name: "render.gi.setIntensity",
        signature: "render.gi.setIntensity(intensity: Float)",
        kind: VargScriptApiKind::Statement,
        description: "Sets runtime global illumination intensity.",
    },
    VargScriptApiItem {
        name: "render.weather.set",
        signature: "render.weather.set(preset: String)",
        kind: VargScriptApiKind::Statement,
        description: "Sets the runtime weather preset.",
    },
    VargScriptApiItem {
        name: "render.weather.setTimeOfDay",
        signature: "render.weather.setTimeOfDay(hour: Float)",
        kind: VargScriptApiKind::Statement,
        description: "Sets runtime time of day in hours.",
    },
    VargScriptApiItem {
        name: "render.weather.setCloudCover",
        signature: "render.weather.setCloudCover(amount: Float)",
        kind: VargScriptApiKind::Statement,
        description: "Sets runtime cloud cover from 0 to 1.",
    },
    VargScriptApiItem {
        name: "render.weather.setPrecipitation",
        signature: "render.weather.setPrecipitation(amount: Float)",
        kind: VargScriptApiKind::Statement,
        description: "Sets runtime precipitation from 0 to 1.",
    },
    VargScriptApiItem {
        name: "render.weather.setWind",
        signature: "render.weather.setWind(wind: Vec3)",
        kind: VargScriptApiKind::Statement,
        description: "Sets runtime weather wind velocity.",
    },
];

const UI_API: &[VargScriptApiItem] = &[
    VargScriptApiItem {
        name: "ui.label",
        signature: "ui.label(id: String, text: String, x: Float, y: Float)",
        kind: VargScriptApiKind::Statement,
        description: "Draws retained text in screen space.",
    },
    VargScriptApiItem {
        name: "ui.rect",
        signature: "ui.rect(id: String, x: Float, y: Float, width: Float, height: Float, r: Float, g: Float, b: Float, a: Float)",
        kind: VargScriptApiKind::Statement,
        description: "Draws a retained rectangle in screen space.",
    },
    VargScriptApiItem {
        name: "ui.screenWidth",
        signature: "ui.screenWidth() -> Float",
        kind: VargScriptApiKind::Function,
        description: "Returns the current runtime output width in pixels.",
    },
    VargScriptApiItem {
        name: "ui.screenHeight",
        signature: "ui.screenHeight() -> Float",
        kind: VargScriptApiKind::Function,
        description: "Returns the current runtime output height in pixels.",
    },
    VargScriptApiItem {
        name: "ui.texture",
        signature: "ui.texture(id: String, texture: String, x: Float, y: Float, width: Float, height: Float)",
        kind: VargScriptApiKind::Statement,
        description: "Draws a retained textured rectangle in screen space.",
    },
    VargScriptApiItem {
        name: "ui.button",
        signature: "ui.button(id: String, text: String, x: Float, y: Float, width: Float, height: Float) -> Bool",
        kind: VargScriptApiKind::Function,
        description: "Draws a button and returns true when clicked.",
    },
    VargScriptApiItem {
        name: "ui.toggle",
        signature: "ui.toggle(id: String, current: Bool, x: Float, y: Float, width: Float, height: Float) -> Bool",
        kind: VargScriptApiKind::Function,
        description: "Draws a toggle and returns its next value.",
    },
    VargScriptApiItem {
        name: "ui.slider",
        signature: "ui.slider(id: String, current: Float, x: Float, y: Float, width: Float, height: Float, min: Float, max: Float) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Draws a slider and returns its next value.",
    },
];

const MATH_API: &[VargScriptApiItem] = &[
    VargScriptApiItem {
        name: "Vec3",
        signature: "Vec3(x: Float, y: Float, z: Float) -> Vec3",
        kind: VargScriptApiKind::Function,
        description: "Constructs a vector value.",
    },
    VargScriptApiItem {
        name: "clamp",
        signature: "clamp(value: Float, min: Float, max: Float) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Clamps a value to a numeric range.",
    },
    VargScriptApiItem {
        name: "lerp",
        signature: "lerp(from: Float, to: Float, t: Float) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Linearly interpolates between two numbers.",
    },
    VargScriptApiItem {
        name: "sin",
        signature: "sin(value: Float) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Sine function.",
    },
    VargScriptApiItem {
        name: "cos",
        signature: "cos(value: Float) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Cosine function.",
    },
    VargScriptApiItem {
        name: "floor",
        signature: "floor(value: Float) -> Float",
        kind: VargScriptApiKind::Function,
        description: "Rounds down to the nearest integer value.",
    },
];

const VARG_SCRIPT_API_REGISTRY: &[VargScriptApiModule] = &[
    VargScriptApiModule {
        name: "Input",
        description: "Frame input, pointer, and capture APIs.",
        items: INPUT_API,
    },
    VargScriptApiModule {
        name: "Time",
        description: "Runtime timing values.",
        items: TIME_API,
    },
    VargScriptApiModule {
        name: "entity",
        description: "Owning entity transform, tag, and lifecycle APIs.",
        items: ENTITY_API,
    },
    VargScriptApiModule {
        name: "scene",
        description: "Read-only scene queries and deferred scene mutation requests.",
        items: SCENE_API,
    },
    VargScriptApiModule {
        name: "Audio",
        description: "Procedural runtime audio commands.",
        items: AUDIO_API,
    },
    VargScriptApiModule {
        name: "render",
        description: "Runtime render environment commands.",
        items: RENDER_API,
    },
    VargScriptApiModule {
        name: "ui",
        description: "Retained immediate gameplay UI helpers.",
        items: UI_API,
    },
    VargScriptApiModule {
        name: "Math",
        description: "Numeric and vector helper functions.",
        items: MATH_API,
    },
];

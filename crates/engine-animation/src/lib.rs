#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Animation system: keyframe clips, animation player, blend tree, and tweens.

use std::collections::HashMap;

use engine_core::math::{Quat, Vec3};
use serde::{Deserialize, Serialize};

/// Interpolation mode between keyframes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum InterpolationMode {
    /// No interpolation; snap to next value.
    Step,
    /// Linear interpolation.
    Linear,
    /// Cubic spline interpolation.
    Cubic,
}

/// A keyframe value at a specific time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    /// Time in seconds.
    pub time: f32,
    /// Value at this keyframe.
    pub value: KeyframeValue,
    /// Interpolation mode from this keyframe to the next.
    pub interpolation: InterpolationMode,
}

/// Value stored at a keyframe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum KeyframeValue {
    /// Float value.
    Float(f32),
    /// 3D vector.
    Vec3(Vec3),
    /// Quaternion rotation.
    Quat(Quat),
    /// Boolean value.
    Bool(bool),
}

/// A track of keyframes targeting a specific property path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimationTrack {
    /// Property path (e.g., "transform.translation", "components.Light.intensity").
    pub path: String,
    /// Keyframes for this track.
    pub keyframes: Vec<Keyframe>,
}

/// Animation clip containing keyframe tracks and metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimationClip {
    /// Clip name.
    pub name: String,
    /// Duration in seconds.
    pub duration: f32,
    /// Tracks keyed by property path.
    pub tracks: Vec<AnimationTrack>,
    /// Loop behavior.
    #[serde(default)]
    pub loop_mode: LoopMode,
    /// Timed gameplay/editor events fired while the clip advances.
    #[serde(default)]
    pub notifies: Vec<AnimationNotify>,
}

/// Loop behavior for animation clips.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum LoopMode {
    /// Play once and stop.
    #[default]
    Once,
    /// Loop continuously.
    Loop,
    /// Play forward then reverse.
    PingPong,
}

/// A named event embedded in an animation clip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimationNotify {
    /// Notify name, such as `footstep.left` or `attack.hit`.
    pub name: String,
    /// Time in seconds within the source clip.
    pub time: f32,
    /// Optional string payload for gameplay systems.
    #[serde(default)]
    pub payload: Option<String>,
}

/// A notify fired during a concrete clip evaluation window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FiredAnimationNotify {
    /// Notify authored on the clip.
    pub notify: AnimationNotify,
    /// Number of completed loop cycles when this notify fired.
    pub loop_index: u32,
}

/// A sampled pose represented as property-path values.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AnimationPose {
    /// Values keyed by property path.
    pub values: HashMap<String, KeyframeValue>,
}

impl AnimationPose {
    /// Creates a pose from sampled values.
    pub fn new(values: HashMap<String, KeyframeValue>) -> Self {
        Self { values }
    }

    /// Returns an empty pose.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Blends this pose with another pose.
    pub fn blend(&self, other: &Self, alpha: f32) -> Self {
        blend_two_poses(self, other, alpha)
    }
}

/// Result of sampling a clip over a time window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimationSample {
    /// Sampled pose at `sample_time`.
    pub pose: AnimationPose,
    /// Wrapped or clamped clip-local time used for sampling.
    pub sample_time: f32,
    /// Normalized clip time in `[0, 1]` when duration is positive.
    pub normalized_time: f32,
    /// Whole loop cycles completed between the previous and current times.
    pub completed_loops: u32,
    /// Whether a non-looping clip reached its end this sample.
    pub reached_end: bool,
    /// Notifies crossed by the evaluation window.
    pub notifies: Vec<FiredAnimationNotify>,
}

/// Easing functions for tweens.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Easing {
    /// Linear interpolation.
    Linear,
    /// Quadratic ease in.
    EaseInQuad,
    /// Quadratic ease out.
    EaseOutQuad,
    /// Quadratic ease in-out.
    EaseInOutQuad,
    /// Cubic ease in.
    EaseInCubic,
    /// Cubic ease out.
    EaseOutCubic,
    /// Cubic ease in-out.
    EaseInOutCubic,
    /// Elastic ease out.
    EaseOutElastic,
    /// Bounce ease out.
    EaseOutBounce,
}

/// Evaluates an easing function at time t in [0, 1].
pub fn evaluate_easing(easing: Easing, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match easing {
        Easing::Linear => t,
        Easing::EaseInQuad => t * t,
        Easing::EaseOutQuad => t * (2.0 - t),
        Easing::EaseInOutQuad => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                -1.0 + (4.0 - 2.0 * t) * t
            }
        }
        Easing::EaseInCubic => t * t * t,
        Easing::EaseOutCubic => {
            let t1 = t - 1.0;
            t1 * t1 * t1 + 1.0
        }
        Easing::EaseInOutCubic => {
            if t < 0.5 {
                4.0 * t * t * t
            } else {
                let t1 = t - 1.0;
                4.0 * t1 * t1 * t1 + 1.0
            }
        }
        Easing::EaseOutElastic => {
            if t == 0.0 || t == 1.0 {
                return t;
            }
            let c4 = (2.0 * std::f32::consts::PI) / 3.0;
            -2.0_f32.powf(10.0 * t - 10.0) * ((t * 10.0 - 10.75) * c4).sin() + 1.0
        }
        Easing::EaseOutBounce => {
            let n1 = 7.5625;
            let d1 = 2.75;
            if t < 1.0 / d1 {
                n1 * t * t
            } else if t < 2.0 / d1 {
                let t1 = t - 1.5 / d1;
                n1 * t1 * t1 + 0.75
            } else if t < 2.5 / d1 {
                let t1 = t - 2.25 / d1;
                n1 * t1 * t1 + 0.9375
            } else {
                let t1 = t - 2.625 / d1;
                n1 * t1 * t1 + 0.984375
            }
        }
    }
}

/// Samples an animation clip at a given time.
pub fn sample_clip(clip: &AnimationClip, time: f32) -> HashMap<String, KeyframeValue> {
    let time = playback_sample_time(clip.loop_mode, clip.duration, time);

    let mut values = HashMap::new();
    for track in &clip.tracks {
        if let Some(value) = sample_track(track, time) {
            values.insert(track.path.clone(), value);
        }
    }
    values
}

/// Samples an animation clip and reports notifies crossed since `previous_time`.
pub fn sample_clip_window(
    clip: &AnimationClip,
    previous_time: f32,
    current_time: f32,
) -> AnimationSample {
    let sample_time = playback_sample_time(clip.loop_mode, clip.duration, current_time);
    let normalized_time = if clip.duration > f32::EPSILON {
        (sample_time / clip.duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let completed_loops =
        completed_loop_count(clip.loop_mode, clip.duration, previous_time, current_time);
    let reached_end = matches!(clip.loop_mode, LoopMode::Once)
        && previous_time < clip.duration
        && current_time >= clip.duration;
    let notifies = collect_notifies(clip, previous_time, current_time);

    AnimationSample {
        pose: AnimationPose::new(sample_clip(clip, current_time)),
        sample_time,
        normalized_time,
        completed_loops,
        reached_end,
        notifies,
    }
}

/// Input context used while evaluating animation graph nodes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AnimationGraphContext {
    /// Previous graph time in seconds.
    pub previous_time: f32,
    /// Current graph time in seconds.
    pub current_time: f32,
    /// Runtime parameters visible to graph nodes.
    #[serde(default)]
    pub parameters: AnimationParameters,
}

impl AnimationGraphContext {
    /// Returns a float parameter or zero when it is not set.
    pub fn parameter_float(&self, name: &str) -> f32 {
        self.parameters.floats.get(name).copied().unwrap_or(0.0)
    }

    /// Returns a copy with time values scaled from zero by `scale`.
    pub fn with_time_scale(self, scale: f32) -> Self {
        Self {
            previous_time: self.previous_time * scale,
            current_time: self.current_time * scale,
            parameters: self.parameters,
        }
    }
}

/// Result of evaluating an animation graph node.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AnimationGraphSample {
    /// Blended output pose.
    pub pose: AnimationPose,
    /// Notifies produced by contributing clip samples.
    pub notifies: Vec<FiredAnimationNotify>,
}

/// A compact runtime animation graph node.
///
/// This mirrors the core shape of Unreal's sequence, two-way blend, and
/// multi-way blend nodes while staying data-oriented and serializable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnimationGraphNode {
    /// Empty node that evaluates to no values.
    Empty,
    /// Evaluates a single keyframe clip.
    Clip(AnimationClip),
    /// Blends two child nodes by alpha.
    Blend2 {
        /// First child node.
        a: Box<AnimationGraphNode>,
        /// Second child node.
        b: Box<AnimationGraphNode>,
        /// Blend alpha in `[0, 1]`.
        alpha: f32,
    },
    /// Blends any number of child nodes by weight.
    BlendN {
        /// Weighted child nodes.
        children: Vec<WeightedAnimationNode>,
        /// Whether to normalize positive weights before blending.
        #[serde(default = "default_true")]
        normalize: bool,
    },
    /// Evaluates a 1D or 2D blend space from graph parameters.
    BlendSpace {
        /// Blend space asset.
        blend_space: BlendSpace,
        /// X axis parameter name.
        x_parameter: String,
        /// Optional Y axis parameter name. Missing values evaluate as zero.
        #[serde(default)]
        y_parameter: Option<String>,
    },
}

impl Default for AnimationGraphNode {
    fn default() -> Self {
        Self::Empty
    }
}

impl AnimationGraphNode {
    /// Evaluates this graph node at the supplied time window.
    pub fn evaluate(&self, context: AnimationGraphContext) -> AnimationGraphSample {
        match self {
            Self::Empty => AnimationGraphSample::default(),
            Self::Clip(clip) => {
                let sample = sample_clip_window(clip, context.previous_time, context.current_time);
                AnimationGraphSample {
                    pose: sample.pose,
                    notifies: sample.notifies,
                }
            }
            Self::Blend2 { a, b, alpha } => {
                let a = a.evaluate(context.clone());
                let b = b.evaluate(context);
                let mut notifies = a.notifies;
                notifies.extend(b.notifies);
                AnimationGraphSample {
                    pose: blend_two_poses(&a.pose, &b.pose, *alpha),
                    notifies,
                }
            }
            Self::BlendN {
                children,
                normalize,
            } => {
                let evaluated: Vec<_> = children
                    .iter()
                    .filter(|child| child.weight > f32::EPSILON)
                    .map(|child| (child.weight, child.node.evaluate(context.clone())))
                    .collect();
                let total_weight: f32 = evaluated.iter().map(|(weight, _)| *weight).sum();
                if evaluated.is_empty() || total_weight <= f32::EPSILON {
                    return AnimationGraphSample::default();
                }

                let mut notifies = Vec::new();
                let mut weighted_poses = Vec::with_capacity(evaluated.len());
                for (weight, sample) in evaluated {
                    let weight = if *normalize {
                        weight / total_weight
                    } else {
                        weight.clamp(0.0, 1.0)
                    };
                    notifies.extend(sample.notifies);
                    weighted_poses.push((sample.pose, weight));
                }

                AnimationGraphSample {
                    pose: blend_weighted_poses(&weighted_poses),
                    notifies,
                }
            }
            Self::BlendSpace {
                blend_space,
                x_parameter,
                y_parameter,
            } => {
                let x = context.parameter_float(x_parameter);
                let y = y_parameter
                    .as_deref()
                    .map(|name| context.parameter_float(name))
                    .unwrap_or(0.0);
                blend_space.evaluate(context, x, y)
            }
        }
    }
}

/// Weighted child for a multi-way blend node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeightedAnimationNode {
    /// Child graph node.
    pub node: AnimationGraphNode,
    /// Desired blend weight.
    pub weight: f32,
}

/// Axis settings for a 1D or 2D blend space.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendSpaceAxis {
    /// Parameter display name.
    pub name: String,
    /// Minimum input value.
    pub min: f32,
    /// Maximum input value.
    pub max: f32,
    /// Whether values wrap around instead of clamping.
    #[serde(default)]
    pub wrap: bool,
}

impl BlendSpaceAxis {
    /// Clamps or wraps an input value according to this axis.
    pub fn normalize_input(&self, value: f32) -> f32 {
        let range = self.max - self.min;
        if range <= f32::EPSILON {
            return self.min;
        }
        if self.wrap {
            self.min + (value - self.min).rem_euclid(range)
        } else {
            value.clamp(self.min, self.max)
        }
    }
}

/// A sample point inside a blend space.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendSpaceSample {
    /// Sample graph.
    pub node: AnimationGraphNode,
    /// X coordinate.
    pub x: f32,
    /// Y coordinate for 2D blend spaces.
    #[serde(default)]
    pub y: f32,
    /// Per-sample playback rate.
    #[serde(default = "default_one")]
    pub rate_scale: f32,
}

/// A lightweight 1D or 2D blend space.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendSpace {
    /// Blend space name.
    pub name: String,
    /// X axis settings.
    pub x_axis: BlendSpaceAxis,
    /// Optional Y axis settings. `None` makes this a 1D blend space.
    #[serde(default)]
    pub y_axis: Option<BlendSpaceAxis>,
    /// Sample points.
    pub samples: Vec<BlendSpaceSample>,
    /// Number of nearest samples to use for 2D inverse-distance blending.
    #[serde(default = "default_blend_space_neighbor_count")]
    pub neighbor_count: usize,
}

impl BlendSpace {
    /// Evaluates this blend space at the supplied input point.
    pub fn evaluate(&self, context: AnimationGraphContext, x: f32, y: f32) -> AnimationGraphSample {
        if self.samples.is_empty() {
            return AnimationGraphSample::default();
        }
        let x = self.x_axis.normalize_input(x);
        let y = self
            .y_axis
            .as_ref()
            .map(|axis| axis.normalize_input(y))
            .unwrap_or(0.0);
        let weights = if self.y_axis.is_some() {
            self.sample_weights_2d(x, y)
        } else {
            self.sample_weights_1d(x)
        };
        self.evaluate_weighted_samples(context, &weights)
    }

    fn sample_weights_1d(&self, x: f32) -> Vec<(usize, f32)> {
        if self.samples.len() == 1 {
            return vec![(0, 1.0)];
        }

        let mut sorted: Vec<_> = self
            .samples
            .iter()
            .enumerate()
            .map(|(index, sample)| (index, sample.x))
            .collect();
        sorted.sort_by(|a, b| a.1.total_cmp(&b.1));

        if x <= sorted[0].1 {
            return vec![(sorted[0].0, 1.0)];
        }
        if x >= sorted[sorted.len() - 1].1 {
            return vec![(sorted[sorted.len() - 1].0, 1.0)];
        }

        for pair in sorted.windows(2) {
            let (left_index, left_x) = pair[0];
            let (right_index, right_x) = pair[1];
            if left_x <= x && x <= right_x {
                let range = (right_x - left_x).abs();
                if range <= f32::EPSILON {
                    return vec![(left_index, 1.0)];
                }
                let alpha = ((x - left_x) / range).clamp(0.0, 1.0);
                return vec![(left_index, 1.0 - alpha), (right_index, alpha)];
            }
        }

        vec![(sorted[0].0, 1.0)]
    }

    fn sample_weights_2d(&self, x: f32, y: f32) -> Vec<(usize, f32)> {
        let count = self.neighbor_count.clamp(1, self.samples.len()).max(1);
        let mut distances: Vec<_> = self
            .samples
            .iter()
            .enumerate()
            .map(|(index, sample)| {
                let dx = sample.x - x;
                let dy = sample.y - y;
                (index, (dx * dx + dy * dy).sqrt())
            })
            .collect();
        distances.sort_by(|a, b| a.1.total_cmp(&b.1));

        if distances[0].1 <= f32::EPSILON {
            return vec![(distances[0].0, 1.0)];
        }

        let mut weights: Vec<_> = distances
            .into_iter()
            .take(count)
            .map(|(index, distance)| (index, 1.0 / distance.max(f32::EPSILON)))
            .collect();
        let total: f32 = weights.iter().map(|(_, weight)| *weight).sum();
        for (_, weight) in &mut weights {
            *weight /= total;
        }
        weights
    }

    fn evaluate_weighted_samples(
        &self,
        context: AnimationGraphContext,
        weights: &[(usize, f32)],
    ) -> AnimationGraphSample {
        let mut poses = Vec::with_capacity(weights.len());
        let mut notifies = Vec::new();
        for (index, weight) in weights {
            let Some(sample) = self.samples.get(*index) else {
                continue;
            };
            let scaled_context = context.clone().with_time_scale(sample.rate_scale.max(0.0));
            let graph_sample = sample.node.evaluate(scaled_context);
            notifies.extend(graph_sample.notifies);
            poses.push((graph_sample.pose, *weight));
        }
        AnimationGraphSample {
            pose: blend_weighted_poses(&poses),
            notifies,
        }
    }
}

/// A clip segment placed on a montage timeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MontageSegment {
    /// Segment name.
    pub name: String,
    /// Clip evaluated by this segment.
    pub clip: AnimationClip,
    /// Start time on the montage timeline.
    pub start_time: f32,
    /// Segment duration in montage seconds.
    pub duration: f32,
    /// Clip-local start offset.
    #[serde(default)]
    pub clip_start_time: f32,
    /// Playback rate for this segment.
    #[serde(default = "default_one")]
    pub rate_scale: f32,
}

/// A named section on a montage timeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MontageSection {
    /// Section name.
    pub name: String,
    /// Start time on the montage timeline.
    pub start_time: f32,
    /// Optional next section name. Point this back to self to loop.
    #[serde(default)]
    pub next_section: Option<String>,
}

/// A montage asset made from placed clip segments and named sections.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AnimationMontage {
    /// Montage name.
    pub name: String,
    /// Timeline length in seconds.
    pub duration: f32,
    /// Placed animation segments.
    pub segments: Vec<MontageSegment>,
    /// Named sections.
    #[serde(default)]
    pub sections: Vec<MontageSection>,
}

impl AnimationMontage {
    /// Creates a stopped runtime montage instance.
    pub fn instantiate(&self) -> MontageInstance {
        MontageInstance {
            time: self
                .sections
                .first()
                .map(|section| section.start_time)
                .unwrap_or(0.0),
            previous_time: 0.0,
            playing: false,
            play_rate: 1.0,
        }
    }

    /// Returns the section active at a timeline time.
    pub fn section_at(&self, time: f32) -> Option<&MontageSection> {
        self.sections
            .iter()
            .filter(|section| section.start_time <= time)
            .max_by(|a, b| a.start_time.total_cmp(&b.start_time))
    }

    /// Returns a section by name.
    pub fn section(&self, name: &str) -> Option<&MontageSection> {
        self.sections.iter().find(|section| section.name == name)
    }

    fn segment_at(&self, time: f32) -> Option<&MontageSegment> {
        self.segments.iter().find(|segment| {
            segment.start_time <= time && time <= segment.start_time + segment.duration
        })
    }

    fn next_section_time(&self, section: &MontageSection) -> Option<f32> {
        section
            .next_section
            .as_deref()
            .and_then(|name| self.section(name))
            .map(|section| section.start_time)
    }
}

/// Result of ticking a montage instance.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MontageSample {
    /// Evaluated graph sample.
    pub graph: AnimationGraphSample,
    /// Current montage timeline time.
    pub time: f32,
    /// Active section name.
    pub section: Option<String>,
    /// Whether the montage finished during this tick.
    pub finished: bool,
}

/// Runtime state for a montage playback instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MontageInstance {
    /// Current montage timeline time.
    pub time: f32,
    /// Previous montage timeline time.
    pub previous_time: f32,
    /// Whether playback is active.
    pub playing: bool,
    /// Playback rate multiplier.
    pub play_rate: f32,
}

impl MontageInstance {
    /// Starts playback from the current time.
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Stops playback.
    pub fn stop(&mut self) {
        self.playing = false;
    }

    /// Jumps to a named section.
    pub fn jump_to_section(&mut self, montage: &AnimationMontage, section: &str) -> bool {
        let Some(section) = montage.section(section) else {
            return false;
        };
        self.previous_time = self.time;
        self.time = section.start_time;
        true
    }

    /// Advances playback and evaluates the active segment.
    pub fn update(&mut self, montage: &AnimationMontage, delta_time: f32) -> MontageSample {
        self.previous_time = self.time;
        if self.playing {
            self.time += delta_time.max(0.0) * self.play_rate.max(0.0);
            self.resolve_section_boundary(montage);
            if self.time >= montage.duration {
                self.time = montage.duration.max(0.0);
                self.playing = false;
            }
        }

        let graph = self.evaluate_segment(montage).unwrap_or_default();
        MontageSample {
            graph,
            time: self.time,
            section: montage
                .section_at(self.time)
                .map(|section| section.name.clone()),
            finished: !self.playing && self.time >= montage.duration,
        }
    }

    fn resolve_section_boundary(&mut self, montage: &AnimationMontage) {
        let Some(section) = montage.section_at(self.previous_time) else {
            return;
        };
        let next_section_start = montage
            .sections
            .iter()
            .filter(|candidate| candidate.start_time > section.start_time)
            .map(|candidate| candidate.start_time)
            .min_by(f32::total_cmp)
            .unwrap_or(montage.duration);
        if self.previous_time < next_section_start && self.time >= next_section_start {
            if let Some(target_time) = montage.next_section_time(section) {
                self.time = target_time + (self.time - next_section_start);
            }
        }
    }

    fn evaluate_segment(&self, montage: &AnimationMontage) -> Option<AnimationGraphSample> {
        let segment = montage.segment_at(self.time)?;
        let previous_time = montage_time_to_clip_time(segment, self.previous_time);
        let current_time = montage_time_to_clip_time(segment, self.time);
        let sample = sample_clip_window(&segment.clip, previous_time, current_time);
        Some(AnimationGraphSample {
            pose: sample.pose,
            notifies: sample.notifies,
        })
    }
}

/// Blends two poses by alpha.
pub fn blend_two_poses(a: &AnimationPose, b: &AnimationPose, alpha: f32) -> AnimationPose {
    let alpha = alpha.clamp(0.0, 1.0);
    let mut output = a.values.clone();
    for (path, b_value) in &b.values {
        let value = match output.get(path) {
            Some(a_value) => blend_values(a_value, b_value, alpha),
            None => b_value.clone(),
        };
        output.insert(path.clone(), value);
    }
    AnimationPose::new(output)
}

/// Blends weighted poses into one output pose.
pub fn blend_weighted_poses(poses: &[(AnimationPose, f32)]) -> AnimationPose {
    let total_weight: f32 = poses.iter().map(|(_, weight)| weight.max(0.0)).sum();
    if poses.is_empty() || total_weight <= f32::EPSILON {
        return AnimationPose::empty();
    }

    let mut output = AnimationPose::empty();
    let mut accumulated_weight = 0.0;
    for (pose, weight) in poses {
        let weight = weight.max(0.0);
        if weight <= f32::EPSILON {
            continue;
        }
        output = if output.values.is_empty() {
            accumulated_weight = weight;
            pose.clone()
        } else {
            let alpha = (weight / (accumulated_weight + weight)).clamp(0.0, 1.0);
            accumulated_weight += weight;
            blend_two_poses(&output, pose, alpha)
        };
    }
    output
}

fn blend_values(a: &KeyframeValue, b: &KeyframeValue, alpha: f32) -> KeyframeValue {
    match (a, b) {
        (KeyframeValue::Float(a), KeyframeValue::Float(b)) => {
            KeyframeValue::Float(a + (b - a) * alpha)
        }
        (KeyframeValue::Vec3(a), KeyframeValue::Vec3(b)) => KeyframeValue::Vec3(a.lerp(*b, alpha)),
        (KeyframeValue::Quat(a), KeyframeValue::Quat(b)) => {
            KeyframeValue::Quat(slerp(*a, *b, alpha))
        }
        (KeyframeValue::Bool(a), KeyframeValue::Bool(b)) => {
            KeyframeValue::Bool(if alpha >= 0.5 { *b } else { *a })
        }
        _ => {
            if alpha >= 0.5 {
                b.clone()
            } else {
                a.clone()
            }
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_one() -> f32 {
    1.0
}

fn default_blend_space_neighbor_count() -> usize {
    3
}

/// A playable animation state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimationState {
    /// Stable state name.
    pub name: String,
    /// Graph evaluated while this state is active.
    pub graph: AnimationGraphNode,
}

/// Runtime condition supplied by gameplay code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TransitionCondition {
    /// Transition is always allowed.
    Always,
    /// Boolean parameter must match `expected`.
    Bool {
        /// Parameter name.
        parameter: String,
        /// Required value.
        expected: bool,
    },
    /// Float parameter must be greater than or equal to `threshold`.
    FloatGreaterEqual {
        /// Parameter name.
        parameter: String,
        /// Threshold.
        threshold: f32,
    },
    /// Float parameter must be less than or equal to `threshold`.
    FloatLessEqual {
        /// Parameter name.
        parameter: String,
        /// Threshold.
        threshold: f32,
    },
}

/// Transition between two animation states.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimationTransition {
    /// Source state name.
    pub from: String,
    /// Target state name.
    pub to: String,
    /// Condition that enables this transition.
    pub condition: TransitionCondition,
    /// Cross-fade duration in seconds.
    #[serde(default)]
    pub blend_duration: f32,
}

/// Parameter values visible to transition rules.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AnimationParameters {
    /// Boolean parameters.
    pub bools: HashMap<String, bool>,
    /// Float parameters.
    pub floats: HashMap<String, f32>,
}

impl AnimationParameters {
    /// Sets a boolean parameter.
    pub fn set_bool(&mut self, name: impl Into<String>, value: bool) {
        self.bools.insert(name.into(), value);
    }

    /// Sets a float parameter.
    pub fn set_float(&mut self, name: impl Into<String>, value: f32) {
        self.floats.insert(name.into(), value);
    }
}

/// A serializable animation state machine definition.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AnimationStateMachine {
    /// State list.
    pub states: Vec<AnimationState>,
    /// Initial state name.
    pub entry: String,
    /// Transition list evaluated in order.
    #[serde(default)]
    pub transitions: Vec<AnimationTransition>,
}

impl AnimationStateMachine {
    /// Creates a runtime instance initialized to the entry state.
    pub fn instantiate(&self) -> AnimationStateMachineInstance {
        let entry = self
            .state_index(&self.entry)
            .unwrap_or(0)
            .min(self.states.len().saturating_sub(1));
        AnimationStateMachineInstance {
            current_state: entry,
            previous_state: None,
            state_time: 0.0,
            previous_state_time: 0.0,
            transition_elapsed: 0.0,
            transition_duration: 0.0,
        }
    }

    fn state_index(&self, name: &str) -> Option<usize> {
        self.states.iter().position(|state| state.name == name)
    }
}

/// Runtime state for an animation state machine.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AnimationStateMachineInstance {
    /// Current state index.
    pub current_state: usize,
    /// Previous state index while a transition is active.
    pub previous_state: Option<usize>,
    /// Elapsed time in the current state.
    pub state_time: f32,
    /// Elapsed time retained for the previous state during a cross-fade.
    pub previous_state_time: f32,
    /// Elapsed time in the active transition.
    pub transition_elapsed: f32,
    /// Active transition duration.
    pub transition_duration: f32,
}

impl AnimationStateMachineInstance {
    /// Evaluates and advances the state machine by `delta_time`.
    pub fn update(
        &mut self,
        machine: &AnimationStateMachine,
        parameters: &AnimationParameters,
        delta_time: f32,
    ) -> AnimationGraphSample {
        if machine.states.is_empty() {
            return AnimationGraphSample::default();
        }

        let delta_time = delta_time.max(0.0);
        if let Some(transition) = self.next_transition(machine, parameters) {
            if let Some(target) = machine.state_index(&transition.to) {
                self.previous_state = Some(self.current_state);
                self.previous_state_time = self.state_time;
                self.current_state = target;
                self.state_time = 0.0;
                self.transition_elapsed = 0.0;
                self.transition_duration = transition.blend_duration.max(0.0);
            }
        }

        let previous_current_time = self.state_time;
        let previous_blend_time = self.previous_state_time;
        self.state_time += delta_time;
        if self.previous_state.is_some() {
            self.previous_state_time += delta_time;
            self.transition_elapsed += delta_time;
        }

        let current = machine.states[self.current_state]
            .graph
            .evaluate(AnimationGraphContext {
                previous_time: previous_current_time,
                current_time: self.state_time,
                parameters: parameters.clone(),
            });

        if let Some(previous_index) = self.previous_state {
            let previous = machine.states[previous_index]
                .graph
                .evaluate(AnimationGraphContext {
                    previous_time: previous_blend_time,
                    current_time: self.previous_state_time,
                    parameters: parameters.clone(),
                });
            let alpha = if self.transition_duration <= f32::EPSILON {
                1.0
            } else {
                (self.transition_elapsed / self.transition_duration).clamp(0.0, 1.0)
            };
            if alpha >= 1.0 {
                self.previous_state = None;
                self.transition_duration = 0.0;
                self.transition_elapsed = 0.0;
                return current;
            }
            let mut notifies = previous.notifies;
            notifies.extend(current.notifies);
            return AnimationGraphSample {
                pose: blend_two_poses(&previous.pose, &current.pose, alpha),
                notifies,
            };
        }

        current
    }

    fn next_transition<'a>(
        &self,
        machine: &'a AnimationStateMachine,
        parameters: &AnimationParameters,
    ) -> Option<&'a AnimationTransition> {
        let from = machine.states.get(self.current_state)?.name.as_str();
        machine
            .transitions
            .iter()
            .find(|transition| transition.from == from && transition.condition.matches(parameters))
    }
}

impl TransitionCondition {
    fn matches(&self, parameters: &AnimationParameters) -> bool {
        match self {
            Self::Always => true,
            Self::Bool {
                parameter,
                expected,
            } => parameters.bools.get(parameter).copied() == Some(*expected),
            Self::FloatGreaterEqual {
                parameter,
                threshold,
            } => parameters
                .floats
                .get(parameter)
                .copied()
                .is_some_and(|value| value >= *threshold),
            Self::FloatLessEqual {
                parameter,
                threshold,
            } => parameters
                .floats
                .get(parameter)
                .copied()
                .is_some_and(|value| value <= *threshold),
        }
    }
}

fn playback_sample_time(loop_mode: LoopMode, duration: f32, time: f32) -> f32 {
    match loop_mode {
        LoopMode::Once => time.clamp(0.0, duration.max(0.0)),
        LoopMode::Loop => {
            if duration > 0.0 {
                time.rem_euclid(duration)
            } else {
                time
            }
        }
        LoopMode::PingPong => {
            if duration > 0.0 {
                let cycle = time.div_euclid(duration);
                let phase = time.rem_euclid(duration);
                if cycle.rem_euclid(2.0) < 1.0 {
                    phase
                } else {
                    duration - phase
                }
            } else {
                time
            }
        }
    }
}

fn completed_loop_count(
    loop_mode: LoopMode,
    duration: f32,
    previous_time: f32,
    current_time: f32,
) -> u32 {
    if !matches!(loop_mode, LoopMode::Loop | LoopMode::PingPong) || duration <= f32::EPSILON {
        return 0;
    }
    let previous_cycle = (previous_time.max(0.0) / duration).floor() as i32;
    let current_cycle = (current_time.max(0.0) / duration).floor() as i32;
    current_cycle.saturating_sub(previous_cycle).max(0) as u32
}

fn collect_notifies(
    clip: &AnimationClip,
    previous_time: f32,
    current_time: f32,
) -> Vec<FiredAnimationNotify> {
    if clip.notifies.is_empty() || current_time <= previous_time {
        return Vec::new();
    }
    let duration = clip.duration;
    if duration <= f32::EPSILON {
        return clip
            .notifies
            .iter()
            .filter(|notify| previous_time < notify.time && notify.time <= current_time)
            .cloned()
            .map(|notify| FiredAnimationNotify {
                notify,
                loop_index: 0,
            })
            .collect();
    }

    match clip.loop_mode {
        LoopMode::Once => clip
            .notifies
            .iter()
            .filter(|notify| {
                let time = notify.time.clamp(0.0, duration);
                previous_time < time && time <= current_time.min(duration)
            })
            .cloned()
            .map(|notify| FiredAnimationNotify {
                notify,
                loop_index: 0,
            })
            .collect(),
        LoopMode::Loop | LoopMode::PingPong => {
            let first_cycle = (previous_time.max(0.0) / duration).floor() as u32;
            let last_cycle = (current_time.max(0.0) / duration).floor() as u32;
            let mut fired = Vec::new();
            for loop_index in first_cycle..=last_cycle {
                let loop_start = loop_index as f32 * duration;
                for notify in &clip.notifies {
                    let event_time = loop_start + notify.time.clamp(0.0, duration);
                    if previous_time < event_time && event_time <= current_time {
                        fired.push(FiredAnimationNotify {
                            notify: notify.clone(),
                            loop_index,
                        });
                    }
                }
            }
            fired
        }
    }
}

fn montage_time_to_clip_time(segment: &MontageSegment, montage_time: f32) -> f32 {
    let local = (montage_time - segment.start_time).clamp(0.0, segment.duration.max(0.0));
    segment.clip_start_time + local * segment.rate_scale.max(0.0)
}

fn sample_track(track: &AnimationTrack, time: f32) -> Option<KeyframeValue> {
    if track.keyframes.is_empty() {
        return None;
    }

    if time <= track.keyframes[0].time {
        return Some(track.keyframes[0].value.clone());
    }

    if time >= track.keyframes.last().unwrap().time {
        return Some(track.keyframes.last().unwrap().value.clone());
    }

    // Binary search: find first keyframe with time > target
    let idx = track.keyframes.partition_point(|kf| kf.time <= time);
    if idx > 0 && idx < track.keyframes.len() {
        let k0 = &track.keyframes[idx - 1];
        let k1 = &track.keyframes[idx];
        let t = if (k1.time - k0.time).abs() > f32::EPSILON {
            (time - k0.time) / (k1.time - k0.time)
        } else {
            0.0
        };
        return Some(interpolate_value(&k0.value, &k1.value, t, k0.interpolation));
    }

    Some(track.keyframes.last().unwrap().value.clone())
}

fn interpolate_value(
    a: &KeyframeValue,
    b: &KeyframeValue,
    t: f32,
    mode: InterpolationMode,
) -> KeyframeValue {
    match mode {
        InterpolationMode::Step => a.clone(),
        InterpolationMode::Linear | InterpolationMode::Cubic => match (a, b) {
            (KeyframeValue::Float(av), KeyframeValue::Float(bv)) => {
                KeyframeValue::Float(av + (bv - av) * t)
            }
            (KeyframeValue::Vec3(av), KeyframeValue::Vec3(bv)) => {
                KeyframeValue::Vec3(av.lerp(*bv, t))
            }
            (KeyframeValue::Quat(av), KeyframeValue::Quat(bv)) => {
                KeyframeValue::Quat(slerp(*av, *bv, t))
            }
            _ => a.clone(),
        },
    }
}

fn slerp(a: Quat, b: Quat, t: f32) -> Quat {
    let dot = (a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w).clamp(-1.0, 1.0);
    if dot > 0.9995 {
        let result = Quat {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
            z: a.z + (b.z - a.z) * t,
            w: a.w + (b.w - a.w) * t,
        };
        let len =
            (result.x * result.x + result.y * result.y + result.z * result.z + result.w * result.w)
                .sqrt();
        if len > f32::EPSILON {
            return Quat {
                x: result.x / len,
                y: result.y / len,
                z: result.z / len,
                w: result.w / len,
            };
        }
        return result;
    }

    let theta_0 = dot.acos();
    let theta = theta_0 * t;
    let sin_theta = theta.sin();
    let sin_theta_0 = theta_0.sin();

    let scale_a = (theta_0 - theta).cos() - dot * sin_theta / sin_theta_0;
    let scale_b = sin_theta / sin_theta_0;

    Quat {
        x: scale_a * a.x + scale_b * b.x,
        y: scale_a * a.y + scale_b * b.y,
        z: scale_a * a.z + scale_b * b.z,
        w: scale_a * a.w + scale_b * b.w,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float_track(path: &str, a: f32, b: f32) -> AnimationTrack {
        AnimationTrack {
            path: path.to_owned(),
            keyframes: vec![
                Keyframe {
                    time: 0.0,
                    value: KeyframeValue::Float(a),
                    interpolation: InterpolationMode::Linear,
                },
                Keyframe {
                    time: 1.0,
                    value: KeyframeValue::Float(b),
                    interpolation: InterpolationMode::Linear,
                },
            ],
        }
    }

    fn clip(name: &str, path: &str, a: f32, b: f32) -> AnimationClip {
        AnimationClip {
            name: name.to_owned(),
            duration: 1.0,
            tracks: vec![float_track(path, a, b)],
            loop_mode: LoopMode::Once,
            notifies: Vec::new(),
        }
    }

    fn pose_value(sample: &AnimationGraphSample, path: &str) -> f32 {
        match sample.pose.values.get(path) {
            Some(KeyframeValue::Float(value)) => *value,
            other => panic!("expected float at {path}, got {other:?}"),
        }
    }

    #[test]
    fn clip_window_fires_notifies_across_loop_boundary() {
        let mut clip = clip("walk", "root.x", 0.0, 1.0);
        clip.loop_mode = LoopMode::Loop;
        clip.notifies = vec![AnimationNotify {
            name: "footstep".to_owned(),
            time: 0.25,
            payload: None,
        }];

        let sample = sample_clip_window(&clip, 0.9, 1.3);

        assert_eq!(sample.completed_loops, 1);
        assert_eq!(sample.notifies.len(), 1);
        assert_eq!(sample.notifies[0].notify.name, "footstep");
        assert_eq!(sample.notifies[0].loop_index, 1);
    }

    #[test]
    fn weighted_blend_normalizes_multiple_poses() {
        let a = AnimationPose::new(HashMap::from([(
            "root.x".to_owned(),
            KeyframeValue::Float(0.0),
        )]));
        let b = AnimationPose::new(HashMap::from([(
            "root.x".to_owned(),
            KeyframeValue::Float(10.0),
        )]));
        let c = AnimationPose::new(HashMap::from([(
            "root.x".to_owned(),
            KeyframeValue::Float(20.0),
        )]));

        let blended = blend_weighted_poses(&[(a, 1.0), (b, 1.0), (c, 2.0)]);

        assert_eq!(blended.values["root.x"], KeyframeValue::Float(12.5));
    }

    #[test]
    fn graph_blend2_combines_clip_poses_and_notifies() {
        let mut b = clip("run", "root.x", 10.0, 20.0);
        b.notifies.push(AnimationNotify {
            name: "land".to_owned(),
            time: 0.5,
            payload: Some("heavy".to_owned()),
        });
        let graph = AnimationGraphNode::Blend2 {
            a: Box::new(AnimationGraphNode::Clip(clip("idle", "root.x", 0.0, 10.0))),
            b: Box::new(AnimationGraphNode::Clip(b)),
            alpha: 0.25,
        };

        let sample = graph.evaluate(AnimationGraphContext {
            previous_time: 0.0,
            current_time: 0.5,
            parameters: AnimationParameters::default(),
        });

        assert_eq!(pose_value(&sample, "root.x"), 7.5);
        assert_eq!(sample.notifies.len(), 1);
        assert_eq!(sample.notifies[0].notify.name, "land");
    }

    #[test]
    fn state_machine_cross_fades_into_matching_transition() {
        let machine = AnimationStateMachine {
            entry: "idle".to_owned(),
            states: vec![
                AnimationState {
                    name: "idle".to_owned(),
                    graph: AnimationGraphNode::Clip(clip("idle", "root.x", 0.0, 0.0)),
                },
                AnimationState {
                    name: "run".to_owned(),
                    graph: AnimationGraphNode::Clip(clip("run", "root.x", 10.0, 10.0)),
                },
            ],
            transitions: vec![AnimationTransition {
                from: "idle".to_owned(),
                to: "run".to_owned(),
                condition: TransitionCondition::Bool {
                    parameter: "moving".to_owned(),
                    expected: true,
                },
                blend_duration: 0.5,
            }],
        };
        let mut instance = machine.instantiate();
        let mut params = AnimationParameters::default();
        params.set_bool("moving", true);

        let sample = instance.update(&machine, &params, 0.25);

        assert_eq!(machine.states[instance.current_state].name, "run");
        assert_eq!(pose_value(&sample, "root.x"), 5.0);
        assert_eq!(instance.previous_state, Some(0));

        let sample = instance.update(&machine, &params, 0.25);

        assert_eq!(pose_value(&sample, "root.x"), 10.0);
        assert_eq!(instance.previous_state, None);
    }

    #[test]
    fn blend_space_1d_interpolates_between_neighbor_samples() {
        let blend_space = BlendSpace {
            name: "speed".to_owned(),
            x_axis: BlendSpaceAxis {
                name: "speed".to_owned(),
                min: 0.0,
                max: 100.0,
                wrap: false,
            },
            y_axis: None,
            samples: vec![
                BlendSpaceSample {
                    node: AnimationGraphNode::Clip(clip("idle", "root.x", 0.0, 0.0)),
                    x: 0.0,
                    y: 0.0,
                    rate_scale: 1.0,
                },
                BlendSpaceSample {
                    node: AnimationGraphNode::Clip(clip("run", "root.x", 100.0, 100.0)),
                    x: 100.0,
                    y: 0.0,
                    rate_scale: 1.0,
                },
            ],
            neighbor_count: 3,
        };

        let sample = blend_space.evaluate(
            AnimationGraphContext {
                previous_time: 0.0,
                current_time: 0.5,
                parameters: AnimationParameters::default(),
            },
            25.0,
            0.0,
        );

        assert_eq!(pose_value(&sample, "root.x"), 25.0);
    }

    #[test]
    fn graph_blend_space_reads_float_parameter() {
        let mut parameters = AnimationParameters::default();
        parameters.set_float("speed", 75.0);
        let graph = AnimationGraphNode::BlendSpace {
            blend_space: BlendSpace {
                name: "locomotion".to_owned(),
                x_axis: BlendSpaceAxis {
                    name: "speed".to_owned(),
                    min: 0.0,
                    max: 100.0,
                    wrap: false,
                },
                y_axis: None,
                samples: vec![
                    BlendSpaceSample {
                        node: AnimationGraphNode::Clip(clip("walk", "root.x", 0.0, 0.0)),
                        x: 0.0,
                        y: 0.0,
                        rate_scale: 1.0,
                    },
                    BlendSpaceSample {
                        node: AnimationGraphNode::Clip(clip("run", "root.x", 100.0, 100.0)),
                        x: 100.0,
                        y: 0.0,
                        rate_scale: 1.0,
                    },
                ],
                neighbor_count: 3,
            },
            x_parameter: "speed".to_owned(),
            y_parameter: None,
        };

        let sample = graph.evaluate(AnimationGraphContext {
            previous_time: 0.0,
            current_time: 0.5,
            parameters,
        });

        assert_eq!(pose_value(&sample, "root.x"), 75.0);
    }

    #[test]
    fn blend_space_2d_uses_nearest_inverse_distance_weights() {
        let blend_space = BlendSpace {
            name: "direction".to_owned(),
            x_axis: BlendSpaceAxis {
                name: "x".to_owned(),
                min: -1.0,
                max: 1.0,
                wrap: false,
            },
            y_axis: Some(BlendSpaceAxis {
                name: "y".to_owned(),
                min: -1.0,
                max: 1.0,
                wrap: false,
            }),
            samples: vec![
                BlendSpaceSample {
                    node: AnimationGraphNode::Clip(clip("left", "root.x", 0.0, 0.0)),
                    x: -1.0,
                    y: 0.0,
                    rate_scale: 1.0,
                },
                BlendSpaceSample {
                    node: AnimationGraphNode::Clip(clip("center", "root.x", 10.0, 10.0)),
                    x: 0.0,
                    y: 0.0,
                    rate_scale: 1.0,
                },
                BlendSpaceSample {
                    node: AnimationGraphNode::Clip(clip("right", "root.x", 20.0, 20.0)),
                    x: 1.0,
                    y: 0.0,
                    rate_scale: 1.0,
                },
            ],
            neighbor_count: 3,
        };

        let sample = blend_space.evaluate(
            AnimationGraphContext {
                previous_time: 0.0,
                current_time: 0.5,
                parameters: AnimationParameters::default(),
            },
            0.0,
            0.0,
        );

        assert_eq!(pose_value(&sample, "root.x"), 10.0);
    }

    #[test]
    fn montage_loops_to_next_section_and_fires_clip_notify() {
        let mut segment_clip = clip("attack", "root.x", 0.0, 10.0);
        segment_clip.notifies.push(AnimationNotify {
            name: "hit".to_owned(),
            time: 0.25,
            payload: None,
        });
        let montage = AnimationMontage {
            name: "combo".to_owned(),
            duration: 1.0,
            segments: vec![MontageSegment {
                name: "attack_a".to_owned(),
                clip: segment_clip,
                start_time: 0.0,
                duration: 0.5,
                clip_start_time: 0.0,
                rate_scale: 1.0,
            }],
            sections: vec![
                MontageSection {
                    name: "loop".to_owned(),
                    start_time: 0.0,
                    next_section: Some("loop".to_owned()),
                },
                MontageSection {
                    name: "end".to_owned(),
                    start_time: 0.5,
                    next_section: None,
                },
            ],
        };
        let mut instance = montage.instantiate();
        instance.play();

        let sample = instance.update(&montage, 0.3);

        assert_eq!(sample.section.as_deref(), Some("loop"));
        assert_eq!(sample.graph.notifies.len(), 1);
        assert_eq!(sample.graph.notifies[0].notify.name, "hit");

        let sample = instance.update(&montage, 0.3);

        assert_eq!(sample.section.as_deref(), Some("loop"));
        assert!(instance.playing);
        assert!(sample.time < 0.5);
    }
}

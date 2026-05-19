#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Audio abstraction and null backend for the Aster engine.
//!
//! The null backend compiles everywhere and satisfies the trait contract without
//! linking any audio library. A real backend (FMOD, kira, cpal, …) replaces it
//! by implementing [`AudioBackend`] and registering it at startup.

use engine_core::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};

pub use engine_core::math::Vec3;

// ── Handles ──────────────────────────────────────────────────────────────────

/// Opaque handle to a loaded audio clip.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct ClipHandle(pub u64);

/// Opaque handle to a playing audio source.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct SourceHandle(pub u64);

// ── AudioClip ────────────────────────────────────────────────────────────────

/// Metadata for a loaded audio clip.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct AudioClipInfo {
    /// Clip name or asset path.
    pub name: String,
    /// Duration in seconds.
    pub duration_secs: f32,
    /// Number of audio channels.
    pub channels: u16,
    /// Sample rate in Hz.
    pub sample_rate: u32,
}

// ── AudioSource ──────────────────────────────────────────────────────────────

/// Playback state of an audio source.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackState {
    /// Not playing.
    #[default]
    Stopped,
    /// Currently playing.
    Playing,
    /// Paused mid-playback.
    Paused,
}

/// Parameters for spawning an audio source.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct AudioSourceDesc {
    /// Clip to play.
    pub clip: ClipHandle,
    /// Playback volume in `[0.0, 1.0]`.
    pub volume: f32,
    /// Playback pitch multiplier.
    pub pitch: f32,
    /// Whether to loop.
    pub looping: bool,
    /// World-space position for 3-D spatialization; `None` for 2-D.
    pub position: Option<Vec3>,
    /// Start playing immediately on spawn.
    pub auto_play: bool,
}

impl AudioSourceDesc {
    /// Creates a simple 2-D source at full volume.
    pub fn simple(clip: ClipHandle) -> Self {
        Self {
            clip,
            volume: 1.0,
            pitch: 1.0,
            looping: false,
            position: None,
            auto_play: true,
        }
    }
}

// ── AudioListener ────────────────────────────────────────────────────────────

/// World-space listener transform used for 3-D spatialization.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct AudioListenerDesc {
    /// Listener position.
    pub position: Vec3,
    /// Forward direction (unit vector).
    pub forward: Vec3,
    /// Up direction (unit vector).
    pub up: Vec3,
}

impl Default for AudioListenerDesc {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            forward: Vec3::new(0.0, 0.0, -1.0),
            up: Vec3::new(0.0, 1.0, 0.0),
        }
    }
}

// ── Backend trait ─────────────────────────────────────────────────────────────

/// Pluggable audio backend contract.
pub trait AudioBackend: Send + Sync {
    /// Loads a clip from raw PCM data (interleaved f32 samples).
    fn load_clip(
        &mut self,
        name: &str,
        samples: &[f32],
        channels: u16,
        sample_rate: u32,
    ) -> EngineResult<ClipHandle>;

    /// Unloads a clip.
    fn unload_clip(&mut self, clip: ClipHandle) -> EngineResult<()>;

    /// Returns clip metadata.
    fn clip_info(&self, clip: ClipHandle) -> EngineResult<AudioClipInfo>;

    /// Spawns an audio source and returns its handle.
    fn spawn_source(&mut self, desc: &AudioSourceDesc) -> EngineResult<SourceHandle>;

    /// Destroys a source.
    fn destroy_source(&mut self, source: SourceHandle) -> EngineResult<()>;

    /// Starts or resumes playback.
    fn play(&mut self, source: SourceHandle) -> EngineResult<()>;

    /// Pauses playback.
    fn pause(&mut self, source: SourceHandle) -> EngineResult<()>;

    /// Stops playback and rewinds.
    fn stop(&mut self, source: SourceHandle) -> EngineResult<()>;

    /// Sets the volume of a source.
    fn set_volume(&mut self, source: SourceHandle, volume: f32) -> EngineResult<()>;

    /// Sets the loop flag of a source.
    fn set_looping(&mut self, source: SourceHandle, looping: bool) -> EngineResult<()>;

    /// Returns the current playback state.
    fn playback_state(&self, source: SourceHandle) -> EngineResult<PlaybackState>;

    /// Updates the listener transform for 3-D spatialization.
    fn set_listener(&mut self, desc: &AudioListenerDesc);

    /// Advances the audio engine by `dt` seconds (called each frame).
    fn update(&mut self, dt: f32);
}

// ── Null backend ──────────────────────────────────────────────────────────────

/// No-op audio backend. Compiles everywhere; produces no sound.
#[derive(Default)]
pub struct NullAudioBackend;

impl AudioBackend for NullAudioBackend {
    fn load_clip(
        &mut self,
        _name: &str,
        _samples: &[f32],
        _channels: u16,
        _sample_rate: u32,
    ) -> EngineResult<ClipHandle> {
        Err(EngineError::other("null audio backend"))
    }

    fn unload_clip(&mut self, _clip: ClipHandle) -> EngineResult<()> {
        Ok(())
    }

    fn clip_info(&self, _clip: ClipHandle) -> EngineResult<AudioClipInfo> {
        Err(EngineError::other("null audio backend"))
    }

    fn spawn_source(&mut self, _desc: &AudioSourceDesc) -> EngineResult<SourceHandle> {
        Err(EngineError::other("null audio backend"))
    }

    fn destroy_source(&mut self, _source: SourceHandle) -> EngineResult<()> {
        Ok(())
    }

    fn play(&mut self, _source: SourceHandle) -> EngineResult<()> {
        Ok(())
    }

    fn pause(&mut self, _source: SourceHandle) -> EngineResult<()> {
        Ok(())
    }

    fn stop(&mut self, _source: SourceHandle) -> EngineResult<()> {
        Ok(())
    }

    fn set_volume(&mut self, _source: SourceHandle, _volume: f32) -> EngineResult<()> {
        Ok(())
    }

    fn set_looping(&mut self, _source: SourceHandle, _looping: bool) -> EngineResult<()> {
        Ok(())
    }

    fn playback_state(&self, _source: SourceHandle) -> EngineResult<PlaybackState> {
        Err(EngineError::other("null audio backend"))
    }

    fn set_listener(&mut self, _desc: &AudioListenerDesc) {}

    fn update(&mut self, _dt: f32) {}
}

// ── AudioContext ──────────────────────────────────────────────────────────────

/// Top-level audio context that owns a backend.
pub struct AudioContext {
    backend: Box<dyn AudioBackend>,
}

impl AudioContext {
    /// Creates an audio context with the given backend.
    pub fn new(backend: impl AudioBackend + 'static) -> Self {
        Self {
            backend: Box::new(backend),
        }
    }

    /// Creates an audio context backed by the null backend.
    pub fn null() -> Self {
        Self::new(NullAudioBackend)
    }

    /// Returns a mutable reference to the backend.
    pub fn backend_mut(&mut self) -> &mut dyn AudioBackend {
        self.backend.as_mut()
    }

    /// Returns a shared reference to the backend.
    pub fn backend(&self) -> &dyn AudioBackend {
        self.backend.as_ref()
    }

    /// Advances the audio engine.
    pub fn update(&mut self, dt: f32) {
        self.backend.update(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_backend_load_clip_returns_error() {
        let mut ctx = AudioContext::null();
        let result = ctx.backend_mut().load_clip("test", &[], 1, 44100);
        assert!(result.is_err());
    }

    #[test]
    fn null_backend_play_pause_stop_are_noops() {
        let mut ctx = AudioContext::null();
        let handle = SourceHandle(0);
        assert!(ctx.backend_mut().play(handle).is_ok());
        assert!(ctx.backend_mut().pause(handle).is_ok());
        assert!(ctx.backend_mut().stop(handle).is_ok());
    }

    #[test]
    fn null_backend_update_does_not_panic() {
        let mut ctx = AudioContext::null();
        ctx.update(1.0 / 60.0);
    }

    #[test]
    fn audio_source_desc_simple_defaults() {
        let desc = AudioSourceDesc::simple(ClipHandle(1));
        assert_eq!(desc.volume, 1.0);
        assert!(!desc.looping);
        assert!(desc.auto_play);
    }

    #[test]
    fn audio_listener_default_faces_negative_z() {
        let listener = AudioListenerDesc::default();
        assert_eq!(listener.forward, Vec3::new(0.0, 0.0, -1.0));
    }
}

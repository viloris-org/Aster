//! Audio scene components: AudioSource and AudioListener lifecycle integration.

use std::any::Any;

use engine_audio::{AudioListenerDesc, AudioSourceDesc, ClipHandle, SourceHandle};

use crate::world::Component;

/// Scene component that owns an audio source handle.
///
/// Stores the creation parameters and the live handle assigned by the backend.
/// Systems are responsible for calling the backend; this component only carries
/// the data.
#[derive(Debug)]
pub struct AudioSourceComponent {
    /// Parameters used to spawn the source.
    pub desc: AudioSourceDesc,
    /// Live handle assigned by the audio backend, if spawned.
    pub handle: Option<SourceHandle>,
}

impl AudioSourceComponent {
    /// Creates a simple 2-D audio source component.
    pub fn simple(clip: ClipHandle) -> Self {
        Self {
            desc: AudioSourceDesc::simple(clip),
            handle: None,
        }
    }
}

impl Component for AudioSourceComponent {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Scene component that marks an entity as the audio listener.
///
/// Only one listener is active at a time; the system picks the first live one.
#[derive(Debug)]
pub struct AudioListenerComponent {
    /// Listener parameters (position/orientation are typically synced from the transform).
    pub desc: AudioListenerDesc,
}

impl Default for AudioListenerComponent {
    fn default() -> Self {
        Self {
            desc: AudioListenerDesc::default(),
        }
    }
}

impl Component for AudioListenerComponent {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    #[test]
    fn audio_source_component_attaches_to_entity() {
        let mut world = World::default();
        let entity = world.spawn().unwrap();
        world
            .insert_component(entity, AudioSourceComponent::simple(ClipHandle(1)))
            .unwrap();
        let src = world.component_mut::<AudioSourceComponent>(entity).unwrap();
        assert!(src.handle.is_none());
        assert_eq!(src.desc.clip, ClipHandle(1));
        assert!(src.desc.auto_play);
    }

    #[test]
    fn audio_listener_component_attaches_to_entity() {
        let mut world = World::default();
        let entity = world.spawn().unwrap();
        world
            .insert_component(entity, AudioListenerComponent::default())
            .unwrap();
        let listener = world
            .component_mut::<AudioListenerComponent>(entity)
            .unwrap();
        use engine_audio::Vec3;
        assert_eq!(listener.desc.forward, Vec3::new(0.0, 0.0, -1.0));
    }
}

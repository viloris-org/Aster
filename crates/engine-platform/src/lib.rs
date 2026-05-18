#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Platform capability abstraction.

pub mod callbacks;
pub mod filesystem;
pub mod input;
pub mod library;
pub mod window;

pub use callbacks::{CallbackThread, ThreadBoundCallback};
pub use filesystem::{FileSystem, HostFileSystem};
pub use input::{InputEvent, KeyCode};
pub use library::DynamicLibraryProvider;
pub use window::{WindowDescriptor, WindowProvider};

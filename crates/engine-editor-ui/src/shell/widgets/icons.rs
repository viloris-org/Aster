//! Icon system for the editor shell UI.

use egui::{Color32, RichText};
use egui_phosphor::regular as icons;

/// Icon button configuration.
pub struct IconButton {
    /// The icon character or string to display.
    pub icon: &'static str,
    /// Font size for the icon.
    pub size: f32,
    /// Color of the icon.
    pub color: Color32,
}

impl IconButton {
    /// Creates a new icon button with default settings.
    pub fn new(icon: &'static str) -> Self {
        Self {
            icon,
            size: 16.0,
            color: Color32::WHITE,
        }
    }

    /// Sets the icon size.
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Sets the icon color.
    pub fn with_color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    /// Converts the icon button to a RichText for rendering.
    pub fn to_rich_text(&self) -> RichText {
        RichText::new(self.icon).size(self.size).color(self.color)
    }
}

/// Common editor tool icons.
pub mod tools {
    use super::icons;

    /// View/hand tool icon.
    pub const VIEW: &str = icons::HAND;
    /// Move/translate tool icon.
    pub const MOVE: &str = icons::ARROWS_OUT_CARDINAL;
    /// Rotate tool icon.
    pub const ROTATE: &str = icons::ARROW_CLOCKWISE;
    /// Scale tool icon.
    pub const SCALE: &str = icons::ARROWS_OUT;
}

/// Transport control icons.
pub mod transport {
    use super::icons;

    /// Play button icon.
    pub const PLAY: &str = icons::PLAY;
    /// Pause button icon.
    pub const PAUSE: &str = icons::PAUSE;
    /// Stop button icon.
    pub const STOP: &str = icons::STOP;
}

/// UI element icons.
pub mod ui {
    use super::icons;

    /// Dropdown caret icon.
    pub const DROPDOWN: &str = icons::CARET_DOWN;
    /// Right chevron icon.
    pub const CHEVRON_RIGHT: &str = icons::CARET_RIGHT;
    /// Down chevron icon.
    pub const CHEVRON_DOWN: &str = icons::CARET_DOWN;
    /// Folder icon.
    pub const FOLDER: &str = icons::FOLDER;
    /// Open folder icon.
    pub const FOLDER_OPEN: &str = icons::FOLDER_OPEN;
    /// File icon.
    pub const FILE: &str = icons::FILE;
    /// Search/magnifying glass icon.
    pub const SEARCH: &str = icons::MAGNIFYING_GLASS;
    /// Settings/gear icon.
    pub const SETTINGS: &str = icons::GEAR;
    /// Close/X icon.
    pub const CLOSE: &str = icons::X;
}

/// Action icons.
pub mod actions {
    use super::icons;

    /// Save/floppy disk icon.
    pub const SAVE: &str = icons::FLOPPY_DISK;
    /// Undo icon.
    pub const UNDO: &str = icons::ARROW_U_UP_LEFT;
    /// Redo icon.
    pub const REDO: &str = icons::ARROW_U_UP_RIGHT;
    /// Add/plus icon.
    pub const ADD: &str = icons::PLUS;
    /// Delete/trash icon.
    pub const DELETE: &str = icons::TRASH;
    /// Copy icon.
    pub const COPY: &str = icons::COPY;
    /// Paste/clipboard icon.
    pub const PASTE: &str = icons::CLIPBOARD;
}

/// Component type icons for hierarchy and inspector.
pub mod components {
    use super::icons;

    /// Camera component icon.
    pub const CAMERA: &str = icons::CAMERA;
    /// Light component icon.
    pub const LIGHTBULB: &str = icons::LIGHTBULB;
    /// Mesh renderer component icon.
    pub const CUBE: &str = icons::CUBE;
    /// Default GameObject icon.
    pub const GAME_OBJECT: &str = icons::CUBE_TRANSPARENT;
}

//! Experimental host-managed WebView panel layout.
//!
//! This is the first slice of the split-panel native editor shell. It creates
//! multiple child WebViews under the existing native host window so layout can
//! move from one full-window React/WebView shell toward host-owned panels.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::utils::config::{Color, WebviewUrl};
use tauri::{LogicalPosition, LogicalSize, Manager, Webview, webview::WebviewBuilder};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum NativePanelKind {
    Toolbar,
    Hierarchy,
    Inspector,
    Statusbar,
}

impl NativePanelKind {
    fn id(self) -> &'static str {
        match self {
            Self::Toolbar => "toolbar",
            Self::Hierarchy => "hierarchy",
            Self::Inspector => "inspector",
            Self::Statusbar => "statusbar",
        }
    }

    fn label(self) -> String {
        format!("main-native-panel-{}", self.id())
    }

    fn all() -> [Self; 4] {
        [
            Self::Toolbar,
            Self::Hierarchy,
            Self::Inspector,
            Self::Statusbar,
        ]
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
pub struct NativePanelRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl NativePanelRect {
    fn visible(self) -> bool {
        self.width > 0 && self.height > 0
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
pub struct NativePanelLayout {
    pub toolbar: NativePanelRect,
    pub hierarchy: NativePanelRect,
    pub inspector: NativePanelRect,
    pub statusbar: NativePanelRect,
}

impl NativePanelLayout {
    fn entries(&self) -> [(NativePanelKind, NativePanelRect); 4] {
        [
            (NativePanelKind::Toolbar, self.toolbar),
            (NativePanelKind::Hierarchy, self.hierarchy),
            (NativePanelKind::Inspector, self.inspector),
            (NativePanelKind::Statusbar, self.statusbar),
        ]
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct NativePanelHostStatus {
    pub enabled: bool,
    pub installed: bool,
    pub panels: Vec<NativePanelKind>,
    pub visible_panels: Vec<NativePanelKind>,
    pub layout: NativePanelLayout,
}

#[derive(Default)]
pub struct NativePanelHost {
    webviews: HashMap<NativePanelKind, Webview>,
    layout: NativePanelLayout,
    visible_panels: Vec<NativePanelKind>,
}

impl NativePanelHost {
    pub fn enabled() -> bool {
        native_panel_host_enabled_from_env(std::env::var("ASTER_NATIVE_PANEL_WEBVIEWS").ok())
    }

    pub fn status(&self) -> NativePanelHostStatus {
        let mut panels = self.webviews.keys().copied().collect::<Vec<_>>();
        panels.sort_by_key(|kind| kind.id());
        let mut visible_panels = self.visible_panels.clone();
        visible_panels.sort_by_key(|kind| kind.id());
        NativePanelHostStatus {
            enabled: Self::enabled(),
            installed: !self.webviews.is_empty(),
            panels,
            visible_panels,
            layout: self.layout.clone(),
        }
    }

    pub fn ensure_installed(
        &mut self,
        app: &tauri::AppHandle,
        window_config: &tauri::utils::config::WindowConfig,
        transparent: bool,
        background: Color,
    ) -> tauri::Result<()> {
        if !Self::enabled() {
            return Ok(());
        }
        let window = app
            .get_window("main")
            .ok_or_else(|| tauri::Error::WindowNotFound)?;

        for kind in NativePanelKind::all() {
            if self.webviews.contains_key(&kind) {
                continue;
            }

            let mut config = window_config.clone();
            config.label = kind.label();
            config.url = WebviewUrl::App(PathBuf::from(format!(
                "index.html?native-panel={}",
                kind.id()
            )));
            let webview = window.add_child(
                WebviewBuilder::from_config(&config)
                    .transparent(transparent)
                    .background_color(background),
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(1.0, 1.0),
            )?;
            webview.set_auto_resize(false)?;
            webview.hide()?;
            self.webviews.insert(kind, webview);
        }

        self.apply_layout(self.layout.clone())?;

        Ok(())
    }

    pub fn apply_layout(&mut self, layout: NativePanelLayout) -> tauri::Result<()> {
        self.layout = layout;
        self.visible_panels.clear();
        for (kind, rect) in self.layout.entries() {
            if let Some(webview) = self.webviews.get(&kind) {
                if rect.visible() {
                    webview.set_bounds(tauri::Rect {
                        position: LogicalPosition::new(rect.x as f64, rect.y as f64).into(),
                        size: LogicalSize::new(rect.width as f64, rect.height as f64).into(),
                    })?;
                    webview.show()?;
                    self.visible_panels.push(kind);
                } else {
                    webview.set_bounds(tauri::Rect {
                        position: LogicalPosition::new(0.0, 0.0).into(),
                        size: LogicalSize::new(1.0, 1.0).into(),
                    })?;
                    webview.hide()?;
                }
            }
        }
        Ok(())
    }
}

fn native_panel_host_enabled_from_env(value: Option<String>) -> bool {
    value
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_kind_labels_are_stable() {
        assert_eq!(
            NativePanelKind::Toolbar.label(),
            "main-native-panel-toolbar"
        );
        assert_eq!(
            NativePanelKind::Hierarchy.label(),
            "main-native-panel-hierarchy"
        );
        assert_eq!(
            NativePanelKind::Inspector.label(),
            "main-native-panel-inspector"
        );
        assert_eq!(
            NativePanelKind::Statusbar.label(),
            "main-native-panel-statusbar"
        );
    }

    #[test]
    fn panel_layout_entries_keep_host_order() {
        let layout = NativePanelLayout {
            toolbar: NativePanelRect {
                x: 0,
                y: 0,
                width: 1200,
                height: 44,
            },
            hierarchy: NativePanelRect {
                x: 0,
                y: 82,
                width: 240,
                height: 600,
            },
            inspector: NativePanelRect {
                x: 920,
                y: 82,
                width: 280,
                height: 600,
            },
            statusbar: NativePanelRect {
                x: 0,
                y: 697,
                width: 1200,
                height: 23,
            },
        };

        assert_eq!(
            layout.entries().map(|(kind, _)| kind),
            [
                NativePanelKind::Toolbar,
                NativePanelKind::Hierarchy,
                NativePanelKind::Inspector,
                NativePanelKind::Statusbar,
            ]
        );
    }

    #[test]
    fn status_reports_visible_panels_separately() {
        let host = NativePanelHost {
            layout: NativePanelLayout::default(),
            visible_panels: vec![NativePanelKind::Inspector, NativePanelKind::Toolbar],
            ..NativePanelHost::default()
        };
        let status = host.status();
        assert_eq!(
            status.visible_panels,
            vec![NativePanelKind::Inspector, NativePanelKind::Toolbar]
        );
        assert!(!status.installed);
    }

    #[test]
    fn split_panel_host_defaults_to_enabled() {
        assert!(native_panel_host_enabled_from_env(None));
    }

    #[test]
    fn split_panel_host_can_be_disabled_for_diagnostics() {
        assert!(!native_panel_host_enabled_from_env(Some("0".to_owned())));
        assert!(!native_panel_host_enabled_from_env(Some(
            "false".to_owned()
        )));
        assert!(native_panel_host_enabled_from_env(Some("1".to_owned())));
        assert!(native_panel_host_enabled_from_env(Some("yes".to_owned())));
    }
}

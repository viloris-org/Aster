use std::path::Path;

/// Varg source role inferred from extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VargFileRole {
    /// Logic files: scripts, modules, and declarative behaviors.
    Logic,
    /// World files: scenes, prefabs, and network declarations.
    World,
    /// Asset files: models, materials, and audio declarations.
    Asset,
}

impl VargFileRole {
    /// Infers a Varg file role from a path extension.
    pub fn from_path(path: impl AsRef<Path>) -> Option<Self> {
        match path
            .as_ref()
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("varg") => Some(Self::Logic),
            Some("vscene") => Some(Self::World),
            Some("vasset") => Some(Self::Asset),
            _ => None,
        }
    }
}

/// Parsed Varg file summary.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct VargFileAst {
    /// Top-level imports.
    pub imports: Vec<VargImport>,
    /// Top-level declarations.
    pub declarations: Vec<VargDeclaration>,
}

/// Parsed import declaration.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct VargImport {
    /// Imported module path.
    pub path: String,
    /// One-based line.
    pub line: usize,
}

/// Parsed top-level declaration.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct VargDeclaration {
    /// Declaration kind.
    pub kind: String,
    /// Declaration name, when present.
    pub name: Option<String>,
    /// One-based line.
    pub line: usize,
    /// Exported properties declared inside this declaration.
    pub exports: Vec<VargExport>,
}

/// Exported script property.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct VargExport {
    /// Property name.
    pub name: String,
    /// Varg type annotation.
    pub type_name: String,
    /// Optional default literal.
    pub default_value: Option<String>,
    /// One-based line.
    pub line: usize,
}

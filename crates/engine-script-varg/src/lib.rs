#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Varg language parser and diagnostics.
//!
//! This crate owns the public Varg authoring surface. Execution backends such as
//! Rhai remain implementation details behind this language service.

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

/// Diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum VargDiagnosticSeverity {
    /// The source cannot compile.
    Error,
    /// The source is accepted but likely unintended.
    Warning,
}

/// Structured Varg diagnostic suitable for editor and AI tools.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct VargDiagnostic {
    /// Stable diagnostic code.
    pub code: String,
    /// Diagnostic severity.
    pub severity: VargDiagnosticSeverity,
    /// One-based line, when available.
    pub line: Option<usize>,
    /// One-based column, when available.
    pub column: Option<usize>,
    /// Human-readable message.
    pub message: String,
    /// Expected syntax or semantic shape.
    pub expected: String,
    /// Concrete suggested fix.
    pub suggestion: String,
    /// Whether the diagnostic blocks compilation.
    pub blocking: bool,
    /// Source line containing the issue, when available.
    pub source_line: Option<String>,
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

/// Parses and validates Varg source for the path role.
pub fn diagnose_source(path: impl AsRef<Path>, source: &str) -> Vec<VargDiagnostic> {
    let role = VargFileRole::from_path(path);
    let mut parser = Parser::new(source, role);
    parser.parse();
    parser.diagnostics
}

/// Parses Varg source and returns an AST summary plus diagnostics.
pub fn parse_source(
    path: impl AsRef<Path>,
    source: &str,
) -> (Option<VargFileAst>, Vec<VargDiagnostic>) {
    let (ast, diagnostics) = parse_source_lossy(path, source);
    if diagnostics.iter().any(|diagnostic| diagnostic.blocking) {
        (None, diagnostics)
    } else {
        (Some(ast), diagnostics)
    }
}

/// Parses Varg source and always returns the best-effort AST summary plus diagnostics.
pub fn parse_source_lossy(
    path: impl AsRef<Path>,
    source: &str,
) -> (VargFileAst, Vec<VargDiagnostic>) {
    let role = VargFileRole::from_path(path);
    let mut parser = Parser::new(source, role);
    let ast = parser.parse();
    let diagnostics = parser.diagnostics;
    (ast, diagnostics)
}

struct Parser<'a> {
    source: &'a str,
    role: Option<VargFileRole>,
    diagnostics: Vec<VargDiagnostic>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, role: Option<VargFileRole>) -> Self {
        Self {
            source,
            role,
            diagnostics: Vec::new(),
        }
    }

    fn parse(&mut self) -> VargFileAst {
        let mut ast = VargFileAst::default();
        let mut stack: Vec<Block> = Vec::new();

        if self.role.is_none() {
            self.push(
                "VARG1000",
                1,
                1,
                "unsupported Varg file extension",
                "Use .varg, .vscene, or .vasset.",
                "Rename the file to the Varg role extension that matches its contents.",
            );
        }

        for (line_index, raw_line) in self.source.lines().enumerate() {
            let line_no = line_index + 1;
            let without_comment = strip_line_comment(raw_line);
            let trimmed = without_comment.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some(path) = parse_import(trimmed) {
                self.validate_import(&path, line_no, raw_line);
                ast.imports.push(VargImport {
                    path,
                    line: line_no,
                });
            }

            if stack.last().is_some_and(|block| block.declarative) && starts_imperative(trimmed) {
                self.push_line(
                    "VARG4001",
                    line_no,
                    raw_line,
                    "imperative control flow is not allowed in declarative Varg files or behavior blocks",
                    "Scene, asset, and behavior declarations must stay deterministic and declarative.",
                    "Move runtime logic into a `script` declaration in a .varg file, or use a declarative construct such as `scatter`.",
                );
            }

            if let Some(header) = parse_header(trimmed) {
                self.validate_top_level_header(&header, stack.len(), line_no, raw_line);
                self.validate_declaration_name(&header, stack.len(), line_no, raw_line);
                let declarative = header.kind == "behavior"
                    || matches!(self.role, Some(VargFileRole::World | VargFileRole::Asset));
                if stack.is_empty() && is_known_top_level_kind(&header.kind) {
                    ast.declarations.push(VargDeclaration {
                        kind: header.kind.clone(),
                        name: header.name.clone(),
                        line: line_no,
                        exports: Vec::new(),
                    });
                }
                stack.push(Block {
                    line: line_no,
                    declarative,
                });
            } else if stack.is_empty() && looks_like_top_level_declaration(trimmed) {
                self.push_line(
                    "VARG1003",
                    line_no,
                    raw_line,
                    "unknown top-level Varg declaration",
                    "Use a declaration allowed by the file role, followed by a block.",
                    "Use `script`, `module`, or `behavior` in .varg; `scene`, `prefab`, or `network` in .vscene; `model`, `material`, or `audio` in .vasset.",
                );
            }

            if let Some(export) = parse_export(trimmed, line_no) {
                if let Some(declaration) = ast.declarations.last_mut() {
                    declaration.exports.push(export);
                }
            } else if trimmed.starts_with("@export var ") {
                self.push_line(
                    "VARG3002",
                    line_no,
                    raw_line,
                    "exported property is missing a name or explicit type annotation",
                    "`@export var name: Type = value`",
                    "Add a camelCase property name and explicit Varg type annotation.",
                );
            }

            if let Some(signature) = parse_function_signature(trimmed) {
                self.validate_lifecycle_signature(&signature, line_no, raw_line);
            }

            let opens = trimmed.matches('{').count();
            let closes = trimmed.matches('}').count();
            for _ in 0..closes.saturating_sub(opens) {
                stack.pop();
            }
        }

        if let Some(block) = stack.last() {
            self.push(
                "VARG1004",
                block.line,
                1,
                "unclosed Varg block",
                "Every `{` must be paired with a closing `}`.",
                "Add a closing brace for this declaration or nested block.",
            );
        }

        ast
    }

    fn validate_import(&mut self, path: &str, line: usize, raw_line: &str) {
        if self.role != Some(VargFileRole::Logic) {
            self.push_line(
                "VARG1005",
                line,
                raw_line,
                "imports are only allowed in .varg logic files",
                "`import \"path/to/module.varg\"` may only import Varg code modules.",
                "Use typed resource constructors such as `Asset(...)`, `Scene(...)`, or `Prefab(...)` for non-code references.",
            );
        }
        if !path.ends_with(".varg") {
            self.push_line(
                "VARG1006",
                line,
                raw_line,
                "imports may only reference .varg code modules",
                "`import \"scripts/combat.varg\"`",
                "Replace this import with a .varg module import, or use a typed resource constructor for scenes and assets.",
            );
        }
    }

    fn validate_top_level_header(
        &mut self,
        header: &Header,
        depth: usize,
        line: usize,
        raw_line: &str,
    ) {
        if depth != 0 {
            return;
        }

        let Some(role) = self.role else {
            return;
        };
        let allowed = match role {
            VargFileRole::Logic => matches!(header.kind.as_str(), "script" | "module" | "behavior"),
            VargFileRole::World => matches!(header.kind.as_str(), "scene" | "prefab" | "network"),
            VargFileRole::Asset => matches!(header.kind.as_str(), "model" | "material" | "audio"),
        };

        if !allowed {
            let expected = match role {
                VargFileRole::Logic => "`script`, `module`, or `behavior`",
                VargFileRole::World => "`scene`, `prefab`, or `network`",
                VargFileRole::Asset => "`model`, `material`, or `audio`",
            };
            self.push_line(
                "VARG1002",
                line,
                raw_line,
                &format!("`{}` is not a valid top-level declaration for this file role", header.kind),
                expected,
                "Move the declaration to the matching Varg file type or change the declaration kind.",
            );
        }
    }

    fn validate_declaration_name(
        &mut self,
        header: &Header,
        depth: usize,
        line: usize,
        raw_line: &str,
    ) {
        if depth != 0 || !is_known_top_level_kind(&header.kind) || header.name.is_some() {
            return;
        }

        self.push_line(
            "VARG1007",
            line,
            raw_line,
            "top-level Varg declarations must have a PascalCase name",
            "`script PlayerController { ... }`, `scene MainScene { ... }`, or `material WoodCrate { ... }`",
            "Add a declaration name after the declaration kind.",
        );
    }

    fn validate_lifecycle_signature(
        &mut self,
        signature: &FunctionSignature,
        line: usize,
        raw_line: &str,
    ) {
        let expected = match signature.name.as_str() {
            "start" => Some(""),
            "update" | "fixedUpdate" => Some("_ dt: Float"),
            "collisionEnter" | "collisionExit" => Some("_ other: Entity"),
            "event" => Some("_ name: String, _ data: EventData"),
            _ => None,
        };

        if let Some(expected_params) = expected {
            let actual = normalize_params(&signature.params);
            if actual != normalize_params(expected_params) {
                self.push_line(
                    "VARG3001",
                    line,
                    raw_line,
                    &format!("{} hook has invalid parameters", signature.name),
                    &format!("`func {}({expected_params})`", signature.name),
                    "Update the hook signature to the reserved Varg lifecycle shape.",
                );
            }
        }
    }

    fn push(
        &mut self,
        code: &str,
        line: usize,
        column: usize,
        message: &str,
        expected: &str,
        suggestion: &str,
    ) {
        self.diagnostics.push(VargDiagnostic {
            code: code.to_string(),
            severity: VargDiagnosticSeverity::Error,
            line: Some(line),
            column: Some(column),
            message: message.to_string(),
            expected: expected.to_string(),
            suggestion: suggestion.to_string(),
            blocking: true,
            source_line: self
                .source
                .lines()
                .nth(line.saturating_sub(1))
                .map(str::to_string),
        });
    }

    fn push_line(
        &mut self,
        code: &str,
        line: usize,
        raw_line: &str,
        message: &str,
        expected: &str,
        suggestion: &str,
    ) {
        let column = raw_line
            .chars()
            .position(|ch| !ch.is_whitespace())
            .map(|index| index + 1)
            .unwrap_or(1);
        self.push(code, line, column, message, expected, suggestion);
    }
}

struct Block {
    line: usize,
    declarative: bool,
}

struct Header {
    kind: String,
    name: Option<String>,
}

struct FunctionSignature {
    name: String,
    params: String,
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//").map_or(line, |(before, _)| before)
}

fn parse_import(line: &str) -> Option<String> {
    let rest = line.strip_prefix("import ")?.trim();
    parse_quoted(rest)
}

fn parse_header(line: &str) -> Option<Header> {
    if !line.ends_with('{') {
        return None;
    }
    let before_brace = line.trim_end_matches('{').trim();
    let mut parts = before_brace.split_whitespace();
    let kind = parts.next()?.to_string();
    let name = parts.next().map(|part| part.trim_matches('"').to_string());
    Some(Header { kind, name })
}

fn parse_export(line: &str, line_no: usize) -> Option<VargExport> {
    let rest = line.strip_prefix("@export var ")?.trim();
    let (name, after_name) = rest.split_once(':')?;
    let (type_name, default_value) = match after_name.split_once('=') {
        Some((type_name, value)) => (type_name.trim(), Some(value.trim().to_string())),
        None => (after_name.trim(), None),
    };
    Some(VargExport {
        name: name.trim().to_string(),
        type_name: type_name.to_string(),
        default_value,
        line: line_no,
    })
}

fn parse_function_signature(line: &str) -> Option<FunctionSignature> {
    let rest = line.strip_prefix("func ")?.trim();
    let open = rest.find('(')?;
    let close = rest[open + 1..].find(')')? + open + 1;
    Some(FunctionSignature {
        name: rest[..open].trim().to_string(),
        params: rest[open + 1..close].trim().to_string(),
    })
}

fn parse_quoted(value: &str) -> Option<String> {
    let value = value.trim();
    let value = value.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].to_string())
}

fn starts_imperative(line: &str) -> bool {
    matches!(
        line.split_whitespace().next(),
        Some("if" | "for" | "while" | "func" | "return" | "break" | "continue" | "var" | "let")
    )
}

fn is_known_top_level_kind(kind: &str) -> bool {
    matches!(
        kind,
        "script"
            | "module"
            | "behavior"
            | "scene"
            | "prefab"
            | "network"
            | "model"
            | "material"
            | "audio"
    )
}

fn looks_like_top_level_declaration(line: &str) -> bool {
    let Some(first) = line.split_whitespace().next() else {
        return false;
    };
    first.chars().next().is_some_and(char::is_alphabetic)
        && !matches!(
            first,
            "import" | "let" | "var" | "func" | "if" | "else" | "for" | "while" | "return"
        )
}

fn normalize_params(params: &str) -> String {
    params.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_script_lifecycle() {
        let diagnostics = diagnose_source(
            "scripts/player.varg",
            r#"script PlayerController {
    @export var speed: Float = 6.0

    func update(_ dt: Float) {
        log("tick")
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn rejects_invalid_update_signature() {
        let diagnostics = diagnose_source(
            "scripts/player.varg",
            r#"script PlayerController {
    func update() {
    }
}
"#,
        );

        assert_eq!(diagnostics[0].code, "VARG3001");
    }

    #[test]
    fn rejects_scene_loops() {
        let diagnostics = diagnose_source(
            "scenes/main.vscene",
            r#"scene MainScene {
    for i in 0..<100 {
        spawnTree(i)
    }
}
"#,
        );

        assert_eq!(diagnostics[0].code, "VARG4001");
    }

    #[test]
    fn extracts_exported_properties() {
        let (ast, diagnostics) = parse_source(
            "scripts/player.varg",
            r#"script PlayerController {
    @export var jumpForce: Float = 8.0
}
"#,
        );

        assert!(diagnostics.is_empty());
        let ast = ast.unwrap();
        assert_eq!(ast.declarations[0].exports[0].name, "jumpForce");
    }

    #[test]
    fn rejects_scene_imports() {
        let diagnostics = diagnose_source("scenes/main.vscene", "import \"scripts/combat.varg\"\n");

        assert_eq!(diagnostics[0].code, "VARG1005");
    }

    #[test]
    fn rejects_non_varg_import_targets() {
        let diagnostics = diagnose_source("scripts/player.varg", "import \"scenes/main.vscene\"\n");

        assert_eq!(diagnostics[0].code, "VARG1006");
    }

    #[test]
    fn rejects_unclosed_blocks() {
        let diagnostics = diagnose_source("scripts/player.varg", "script Player {\n");

        assert_eq!(diagnostics[0].code, "VARG1004");
    }

    #[test]
    fn rejects_missing_declaration_name() {
        let diagnostics = diagnose_source("scripts/player.varg", "script {\n}\n");

        assert_eq!(diagnostics[0].code, "VARG1007");
    }

    #[test]
    fn rejects_malformed_export() {
        let diagnostics = diagnose_source(
            "scripts/player.varg",
            r#"script Player {
    @export var speed = 6.0
}
"#,
        );

        assert_eq!(diagnostics[0].code, "VARG3002");
    }
}

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

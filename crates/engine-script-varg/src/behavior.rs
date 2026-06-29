use std::path::Path;

use crate::diagnostics::{VargDiagnostic, VargDiagnosticSeverity};
use crate::parser::parse_source;
use crate::syntax::{parse_quoted, strip_line_comment};

/// Compiled declarative behavior tree summary from a `.varg` behavior declaration.
pub struct VargBehavior {
    /// Behavior declaration name.
    pub name: String,
    /// Root behavior node.
    pub root: VargBehaviorNode,
}

/// Declarative behavior tree node.
#[derive(Clone, Debug, PartialEq)]
pub enum VargBehaviorNode {
    /// Execute children in order until one fails.
    Sequence {
        /// Optional author-facing node name.
        name: Option<String>,
        /// Child nodes.
        children: Vec<VargBehaviorNode>,
    },
    /// Execute children in order until one succeeds.
    Selector {
        /// Optional author-facing node name.
        name: Option<String>,
        /// Child nodes.
        children: Vec<VargBehaviorNode>,
    },
    /// Execute child nodes in parallel.
    Parallel {
        /// Optional author-facing node name.
        name: Option<String>,
        /// Child nodes.
        children: Vec<VargBehaviorNode>,
    },
    /// Pure condition expression.
    Condition {
        /// Varg condition expression source after `when`.
        expression: String,
    },
    /// Declarative action call.
    Action {
        /// Varg action expression source after `action`.
        expression: String,
    },
    /// Invert child result.
    Invert {
        /// Child node.
        child: Box<VargBehaviorNode>,
    },
    /// Force child result to success.
    Succeed {
        /// Child node.
        child: Box<VargBehaviorNode>,
    },
    /// Repeat a child node.
    Repeat {
        /// Optional repeat count. `None` means unbounded.
        count: Option<u32>,
        /// Child node.
        child: Box<VargBehaviorNode>,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct BehaviorBlock {
    kind: String,
    name: Option<String>,
    repeat_count: Option<u32>,
    line: usize,
    children: Vec<VargBehaviorNode>,
}

/// Per-invocation context passed to the Varg script runtime.
pub fn compile_behavior_source(
    path: impl AsRef<Path>,
    source: &str,
) -> (Option<VargBehavior>, Vec<VargDiagnostic>) {
    let (ast, mut diagnostics) = parse_source(path, source);
    let Some(ast) = ast else {
        return (None, diagnostics);
    };
    let Some(declaration) = ast
        .declarations
        .iter()
        .find(|declaration| declaration.kind == "behavior")
    else {
        diagnostics.push(VargDiagnostic {
            code: "VARG5000".to_string(),
            severity: VargDiagnosticSeverity::Error,
            line: Some(1),
            column: Some(1),
            message: "logic file does not contain a behavior declaration".to_string(),
            expected: "`behavior Name { ... }`".to_string(),
            suggestion: "Add a behavior declaration or compile a file that contains one."
                .to_string(),
            blocking: true,
            source_line: source.lines().next().map(str::to_string),
        });
        return (None, diagnostics);
    };

    match parse_behavior_declaration(
        source,
        declaration
            .name
            .clone()
            .unwrap_or_else(|| "UnnamedBehavior".to_string()),
        declaration.line,
    ) {
        Ok(behavior) => (Some(behavior), diagnostics),
        Err(error) => {
            diagnostics.push(error);
            (None, diagnostics)
        }
    }
}

/// Parses a `.vscene` world file into the native scene file structure.
///
fn parse_behavior_declaration(
    source: &str,
    name: String,
    declaration_line: usize,
) -> Result<VargBehavior, VargDiagnostic> {
    let mut stack: Vec<BehaviorBlock> = Vec::new();
    let mut root_children = Vec::new();
    let mut inside_behavior = false;
    let mut behavior_depth = 0isize;

    for (line_index, raw_line) in source.lines().enumerate() {
        let line = line_index + 1;
        let without_comment = strip_line_comment(raw_line);
        let trimmed = without_comment.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !inside_behavior {
            if line == declaration_line {
                inside_behavior = true;
                behavior_depth =
                    trimmed.matches('{').count() as isize - trimmed.matches('}').count() as isize;
            }
            continue;
        }

        if line == declaration_line {
            continue;
        }

        if trimmed == "}" {
            if let Some(block) = stack.pop() {
                let node = behavior_block_to_node(block)?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root_children.push(node);
                }
            } else {
                behavior_depth -= 1;
                if behavior_depth <= 0 {
                    break;
                }
            }
            continue;
        }

        if let Some(block) = parse_behavior_block_header(trimmed, line, source)? {
            stack.push(block);
            behavior_depth += 1;
            continue;
        }

        if let Some(expression) = trimmed.strip_prefix("when ") {
            let node = VargBehaviorNode::Condition {
                expression: expression.trim().to_string(),
            };
            if let Some(parent) = stack.last_mut() {
                parent.children.push(node);
            } else {
                root_children.push(node);
            }
            continue;
        }

        if let Some(expression) = trimmed.strip_prefix("action ") {
            let node = VargBehaviorNode::Action {
                expression: expression.trim().to_string(),
            };
            if let Some(parent) = stack.last_mut() {
                parent.children.push(node);
            } else {
                root_children.push(node);
            }
            continue;
        }

        return Err(behavior_error(
            source,
            line,
            1,
            "VARG5001",
            "unsupported behavior statement",
            "`selector`, `sequence`, `parallel`, `repeat`, `invert`, `succeed`, `when`, or `action`",
            "Rewrite this line using declarative behavior tree syntax.",
        ));
    }

    if let Some(block) = stack.last() {
        return Err(behavior_error(
            source,
            block.line,
            1,
            "VARG5002",
            "unclosed behavior block",
            "Every behavior node block must be closed with `}`.",
            "Add a closing brace for this behavior node.",
        ));
    }

    let root = match root_children.len() {
        0 => {
            return Err(behavior_error(
                source,
                declaration_line,
                1,
                "VARG5003",
                "behavior declaration has no nodes",
                "At least one `when`, `action`, `selector`, `sequence`, or `parallel` node.",
                "Add a root behavior node.",
            ));
        }
        1 => root_children.remove(0),
        _ => VargBehaviorNode::Parallel {
            name: Some("root".to_string()),
            children: root_children,
        },
    };

    Ok(VargBehavior { name, root })
}

fn parse_behavior_block_header(
    trimmed: &str,
    line: usize,
    source: &str,
) -> Result<Option<BehaviorBlock>, VargDiagnostic> {
    if !trimmed.ends_with('{') {
        return Ok(None);
    }
    let before_brace = trimmed.trim_end_matches('{').trim();
    let mut parts = before_brace.split_whitespace();
    let Some(kind) = parts.next() else {
        return Ok(None);
    };
    if !matches!(
        kind,
        "selector" | "sequence" | "parallel" | "repeat" | "invert" | "succeed"
    ) {
        return Ok(None);
    }

    let mut repeat_count = None;
    let name = match kind {
        "repeat" => match parts.next() {
            Some("forever") | None => None,
            Some(value) => {
                repeat_count = Some(value.parse::<u32>().map_err(|_| {
                    behavior_error(
                        source,
                        line,
                        1,
                        "VARG5004",
                        "repeat count must be a positive integer or `forever`",
                        "`repeat 3 { ... }` or `repeat forever { ... }`",
                        "Use an integer repeat count, or omit it for an unbounded repeat.",
                    )
                })?);
                None
            }
        },
        _ => {
            let rest = before_brace[kind.len()..].trim();
            if rest.is_empty() {
                None
            } else {
                parse_quoted(rest).or_else(|| Some(rest.to_string()))
            }
        }
    };

    Ok(Some(BehaviorBlock {
        kind: kind.to_string(),
        name,
        repeat_count,
        line,
        children: Vec::new(),
    }))
}

fn behavior_block_to_node(block: BehaviorBlock) -> Result<VargBehaviorNode, VargDiagnostic> {
    match block.kind.as_str() {
        "sequence" => Ok(VargBehaviorNode::Sequence {
            name: block.name,
            children: block.children,
        }),
        "selector" => Ok(VargBehaviorNode::Selector {
            name: block.name,
            children: block.children,
        }),
        "parallel" => Ok(VargBehaviorNode::Parallel {
            name: block.name,
            children: block.children,
        }),
        "invert" => {
            let child = single_behavior_child(&block)?;
            Ok(VargBehaviorNode::Invert {
                child: Box::new(child),
            })
        }
        "succeed" => {
            let child = single_behavior_child(&block)?;
            Ok(VargBehaviorNode::Succeed {
                child: Box::new(child),
            })
        }
        "repeat" => {
            let child = single_behavior_child(&block)?;
            Ok(VargBehaviorNode::Repeat {
                count: block.repeat_count,
                child: Box::new(child),
            })
        }
        _ => unreachable!("behavior block kind validated before push"),
    }
}

fn single_behavior_child(block: &BehaviorBlock) -> Result<VargBehaviorNode, VargDiagnostic> {
    if block.children.len() == 1 {
        return Ok(block.children[0].clone());
    }
    Err(VargDiagnostic {
        code: "VARG5005".to_string(),
        severity: VargDiagnosticSeverity::Error,
        line: Some(block.line),
        column: Some(1),
        message: format!("`{}` behavior node requires exactly one child", block.kind),
        expected: format!("`{} {{ <one child node> }}`", block.kind),
        suggestion: "Wrap multiple children in a `sequence`, `selector`, or `parallel` node."
            .to_string(),
        blocking: true,
        source_line: None,
    })
}

fn behavior_error(
    source: &str,
    line: usize,
    column: usize,
    code: &str,
    message: &str,
    expected: &str,
    suggestion: &str,
) -> VargDiagnostic {
    VargDiagnostic {
        code: code.to_string(),
        severity: VargDiagnosticSeverity::Error,
        line: Some(line),
        column: Some(column),
        message: message.to_string(),
        expected: expected.to_string(),
        suggestion: suggestion.to_string(),
        blocking: true,
        source_line: source
            .lines()
            .nth(line.saturating_sub(1))
            .map(str::to_string),
    }
}

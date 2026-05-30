//! Parses AI model responses into [`AgentOperation`] lists.
//!
//! Expects JSON array output. Handles common formatting issues:
//! - Leading/trailing text outside the JSON block
//! - Code-fenced JSON blocks

use engine_core::{EngineError, EngineResult};

use crate::AgentOperation;

/// Attempts to parse AI model output text into a list of agent operations.
///
/// First tries to parse the entire text as a JSON array of operations.
/// If that fails, extracts the first JSON array found in the text and tries again.
pub fn parse_operations(raw: &str) -> EngineResult<Vec<AgentOperation>> {
    let trimmed = raw.trim();

    // Try direct parse first
    if let Ok(ops) = serde_json::from_str::<Vec<AgentOperation>>(trimmed) {
        return Ok(ops);
    }

    // Try to extract JSON array from code fences or surrounding text
    let extracted = extract_json_array(trimmed).ok_or_else(|| {
        EngineError::other(format!("failed to parse AI response as operations: {raw}"))
    })?;

    serde_json::from_str::<Vec<AgentOperation>>(&extracted)
        .map_err(|e| EngineError::other(format!("failed to parse operations: {e}")))
}

/// Extracts the first JSON array block from text that may include markdown fences
/// or explanatory text.
fn extract_json_array(text: &str) -> Option<String> {
    // Try to find ```json ... ``` code fence
    if let Some(start) = text.find("```json") {
        let after_fence = &text[start + 7..];
        if let Some(end) = after_fence.find("```") {
            let inner = after_fence[..end].trim();
            if inner.starts_with('[') {
                return Some(inner.to_string());
            }
        }
    }

    // Try to find ``` ... ``` code fence (no language tag)
    if let Some(start) = text.find("```") {
        let after_fence = &text[start + 3..];
        if let Some(end) = after_fence.find("```") {
            let inner = after_fence[..end].trim();
            if inner.starts_with('[') {
                return Some(inner.to_string());
            }
        }
    }

    // Find first '[' and last ']' pair
    let first_bracket = text.find('[')?;
    let last_bracket = text.rfind(']')?;
    if last_bracket > first_bracket {
        return Some(text[first_bracket..=last_bracket].to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_json_array() {
        let input = r#"[
            {"action": "create_object", "name": "Player"},
            {"action": "complete", "summary": "done"}
        ]"#;
        let ops = parse_operations(input).unwrap();
        assert_eq!(ops.len(), 2);
        assert!(matches!(ops[0], AgentOperation::CreateObject { .. }));
        assert!(matches!(ops[1], AgentOperation::Complete { .. }));
    }

    #[test]
    fn parses_code_fenced_json() {
        let input = r#"Here are the operations:
```json
[
    {"action": "write_script", "path": "test.rhai", "source": "fn on_start() {}"}
]
```
Done."#;
        let ops = parse_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], AgentOperation::WriteScript { .. }));
    }

    #[test]
    fn parses_json_array_in_text() {
        let input = r#"I'll create a light.
[{"action": "create_object", "name": "Light", "components": [{"type": "Light"}]}]
That should do it."#;
        let ops = parse_operations(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], AgentOperation::CreateObject { .. }));
    }

    #[test]
    fn returns_error_for_invalid_input() {
        let input = "This is not JSON at all.";
        let result = parse_operations(input);
        assert!(result.is_err());
    }

    #[test]
    fn parses_empty_array() {
        let ops = parse_operations("[]").unwrap();
        assert!(ops.is_empty());
    }
}

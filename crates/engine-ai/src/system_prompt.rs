//! Builds the system prompt for the AI model.
//!
//! The prompt describes the Aster engine's capabilities, the available
//! operations, and the constraints the AI must follow.

use engine_editor::EditorCommand;

/// Builds the system prompt from available commands.
///
/// Includes:
/// - Engine context description
/// - Available operations with their schemas
/// - Output format requirements
/// - Safety and best-practice constraints
pub fn build(available_commands: &[&EditorCommand]) -> String {
    let mut prompt = String::new();

    prompt.push_str(include_str!("system_prompt_base.txt"));

    prompt.push_str("\n\n## Available Commands\n\n");
    prompt.push_str("You can execute these editor commands via `execute_command`:\n\n");
    for cmd in available_commands {
        prompt.push_str(&format!(
            "- **{}** (`{}`) — category: {}\n",
            cmd.label, cmd.id, cmd.category
        ));
    }

    prompt.push_str(
        r#"

## Output Format

Respond with a JSON array of operations. Each operation has an `action` field and
relevant parameters. Available actions:

### create_object
```json
{ "action": "create_object", "name": "Player", "position": [0.0, 0.0, 0.0], "components": [
    { "type": "Rigidbody" },
    { "type": "Collider", "properties": { "shape": "capsule" } }
] }
```

### set_property
```json
{ "action": "set_property", "entity": "1:1", "component": "Rigidbody", "field": "mass", "value": 2.0 }
```

### write_script
```json
{ "action": "write_script", "path": "scripts/player.rhai", "source": "fn on_update(dt) {\n    if is_pressed(\"Space\") {\n        translate(0.0, 10.0 * dt, 0.0);\n    }\n}" }
```

### execute_command
```json
{ "action": "execute_command", "command": "gameobject.create_empty" }
```

### remove_component
```json
{ "action": "remove_component", "entity": "1:1", "component": "Rigidbody" }
```

### destroy_object
```json
{ "action": "destroy_object", "entity": "1:2" }
```

### read_file
```json
{ "action": "read_file", "path": "scripts/player.rhai" }
```

### complete
```json
{ "action": "complete", "summary": "Created player controller with jump mechanic" }
```

## Constraints

1. Use entity IDs exactly as shown in the context.
2. Component types: Camera, MeshRenderer, Light, Rigidbody, Collider, AudioSource, Script, Sprite2D.
3. Script paths use `project:/` prefix when referencing existing scripts.
4. Create or modify scripts to implement game logic; use set_property for parameter tweaks.
5. End every response with a `complete` operation.
"#
    );

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_editor::{CommandAvailability, CommandRegistry, EditorCommand};

    #[test]
    fn system_prompt_includes_commands() {
        let mut registry = CommandRegistry::default();
        registry.register(EditorCommand {
            id: "test.command".into(),
            label: "Test Command".into(),
            category: "Test".into(),
            shortcut: None,
            availability: CommandAvailability::Always,
        });
        let commands: Vec<&EditorCommand> = registry.commands().collect();
        let prompt = build(&commands);
        assert!(prompt.contains("Test Command"));
        assert!(prompt.contains("test.command"));
    }

    #[test]
    fn system_prompt_includes_action_descriptions() {
        let registry = CommandRegistry::default();
        let commands: Vec<&EditorCommand> = registry.commands().collect();
        let prompt = build(&commands);
        assert!(prompt.contains("create_object"));
        assert!(prompt.contains("write_script"));
        assert!(prompt.contains("complete"));
    }
}

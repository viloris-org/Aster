#![forbid(unsafe_code)]

use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader, Read, Write},
};

use engine_script_varg::{
    VargDeclaration, VargDiagnostic, VargDiagnosticSeverity, VargFileAst, parse_source,
    parse_source_lossy,
};
use serde_json::{Value, json};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut server = LspServer::new(stdout.lock());
    server.run(stdin.lock())
}

struct LspServer<W> {
    writer: W,
    documents: HashMap<String, String>,
}

impl<W: Write> LspServer<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            documents: HashMap::new(),
        }
    }

    fn run<R: Read>(&mut self, reader: R) -> io::Result<()> {
        let mut reader = BufReader::new(reader);
        while let Some(message) = read_lsp_message(&mut reader)? {
            self.handle_message(message)?;
        }
        Ok(())
    }

    fn handle_message(&mut self, message: Value) -> io::Result<()> {
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id").cloned();

        match method {
            Some("initialize") => {
                if let Some(id) = id {
                    self.write_response(id, initialize_result())?;
                }
            }
            Some("shutdown") => {
                if let Some(id) = id {
                    self.write_response(id, Value::Null)?;
                }
            }
            Some("exit") => {}
            Some("textDocument/didOpen") => {
                if let Some((uri, text)) = text_document_text(&message, "textDocument") {
                    self.documents.insert(uri.clone(), text);
                    self.publish_diagnostics(&uri)?;
                }
            }
            Some("textDocument/didChange") => {
                if let Some(uri) = text_document_uri(&message, "textDocument") {
                    if let Some(text) = latest_change_text(&message) {
                        self.documents.insert(uri.clone(), text);
                        self.publish_diagnostics(&uri)?;
                    }
                }
            }
            Some("textDocument/didClose") => {
                if let Some(uri) = text_document_uri(&message, "textDocument") {
                    self.documents.remove(&uri);
                    self.write_notification(
                        "textDocument/publishDiagnostics",
                        json!({"uri": uri, "diagnostics": []}),
                    )?;
                }
            }
            Some("textDocument/documentSymbol") => {
                if let Some(id) = id {
                    let symbols = text_document_uri(&message, "textDocument")
                        .and_then(|uri| self.documents.get(&uri).map(|text| (uri, text)))
                        .map(|(uri, text)| document_symbols(&uri, text))
                        .unwrap_or_default();
                    self.write_response(id, Value::Array(symbols))?;
                }
            }
            Some(method) => {
                if let Some(id) = id {
                    self.write_error(id, -32601, format!("method not found: {method}"))?;
                }
            }
            None => {}
        }

        Ok(())
    }

    fn publish_diagnostics(&mut self, uri: &str) -> io::Result<()> {
        let Some(text) = self.documents.get(uri) else {
            return Ok(());
        };
        let diagnostics = varg_diagnostics(uri, text);
        self.write_notification(
            "textDocument/publishDiagnostics",
            json!({
                "uri": uri,
                "diagnostics": diagnostics,
            }),
        )
    }

    fn write_response(&mut self, id: Value, result: Value) -> io::Result<()> {
        self.write_message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        }))
    }

    fn write_error(&mut self, id: Value, code: i64, message: String) -> io::Result<()> {
        self.write_message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": code,
                "message": message,
            },
        }))
    }

    fn write_notification(&mut self, method: &str, params: Value) -> io::Result<()> {
        self.write_message(json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
    }

    fn write_message(&mut self, message: Value) -> io::Result<()> {
        let body = serde_json::to_vec(&message)?;
        write!(self.writer, "Content-Length: {}\r\n\r\n", body.len())?;
        self.writer.write_all(&body)?;
        self.writer.flush()
    }
}

fn read_lsp_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
        }
    }

    let Some(length) = content_length else {
        return Ok(None);
    };
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn initialize_result() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": 1,
            "documentSymbolProvider": true,
            "diagnosticProvider": {
                "interFileDependencies": false,
                "workspaceDiagnostics": false
            }
        },
        "serverInfo": {
            "name": "varg-lsp",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn text_document_uri(message: &Value, key: &str) -> Option<String> {
    message
        .get("params")?
        .get(key)?
        .get("uri")?
        .as_str()
        .map(str::to_string)
}

fn text_document_text(message: &Value, key: &str) -> Option<(String, String)> {
    let document = message.get("params")?.get(key)?;
    let uri = document.get("uri")?.as_str()?.to_string();
    let text = document.get("text")?.as_str()?.to_string();
    Some((uri, text))
}

fn latest_change_text(message: &Value) -> Option<String> {
    message
        .get("params")?
        .get("contentChanges")?
        .as_array()?
        .last()?
        .get("text")?
        .as_str()
        .map(str::to_string)
}

fn varg_diagnostics(uri: &str, text: &str) -> Vec<Value> {
    let (_, diagnostics) = parse_source(uri, text);
    diagnostics
        .iter()
        .map(varg_diagnostic_to_lsp)
        .collect::<Vec<_>>()
}

fn document_symbols(uri: &str, text: &str) -> Vec<Value> {
    let (ast, _) = parse_source_lossy(uri, text);
    ast_to_symbols(&ast)
}

fn ast_to_symbols(ast: &VargFileAst) -> Vec<Value> {
    ast.declarations.iter().map(declaration_to_symbol).collect()
}

fn declaration_to_symbol(declaration: &VargDeclaration) -> Value {
    let line = declaration.line.saturating_sub(1) as u32;
    json!({
        "name": declaration.name.clone().unwrap_or_else(|| "Unnamed".to_string()),
        "detail": declaration.kind,
        "kind": symbol_kind(&declaration.kind),
        "range": lsp_range(line, 0, line, 0),
        "selectionRange": lsp_range(line, 0, line, 0),
        "children": declaration.exports.iter().map(|export| {
            let export_line = export.line.saturating_sub(1) as u32;
            json!({
                "name": export.name,
                "detail": format!("{}{}", export.type_name, export.default_value.as_ref().map(|value| format!(" = {value}")).unwrap_or_default()),
                "kind": 7,
                "range": lsp_range(export_line, 0, export_line, 0),
                "selectionRange": lsp_range(export_line, 0, export_line, 0)
            })
        }).collect::<Vec<_>>()
    })
}

fn symbol_kind(kind: &str) -> u32 {
    match kind {
        "module" => 2,
        "script" | "behavior" => 5,
        "scene" | "prefab" | "network" => 3,
        "model" | "material" | "audio" => 13,
        _ => 13,
    }
}

fn varg_diagnostic_to_lsp(diagnostic: &VargDiagnostic) -> Value {
    let line = diagnostic.line.unwrap_or(1).saturating_sub(1) as u32;
    let column = diagnostic.column.unwrap_or(1).saturating_sub(1) as u32;
    json!({
        "range": lsp_range(line, column, line, column.saturating_add(1)),
        "severity": match diagnostic.severity {
            VargDiagnosticSeverity::Error => 1,
            VargDiagnosticSeverity::Warning => 2,
        },
        "code": diagnostic.code,
        "source": "varg",
        "message": format!("{}\nExpected: {}\nFix: {}", diagnostic.message, diagnostic.expected, diagnostic.suggestion),
    })
}

fn lsp_range(start_line: u32, start_character: u32, end_line: u32, end_character: u32) -> Value {
    json!({
        "start": {"line": start_line, "character": start_character},
        "end": {"line": end_line, "character": end_character},
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_diagnostics_use_zero_based_positions() {
        let diagnostics = varg_diagnostics(
            "file:///game/scripts/player.varg",
            "script Player {\n    func update() {}\n}\n",
        );

        assert_eq!(diagnostics[0]["range"]["start"]["line"], 1);
        assert_eq!(diagnostics[0]["code"], "VARG3001");
    }

    #[test]
    fn document_symbols_include_exported_properties() {
        let symbols = document_symbols(
            "file:///game/scripts/player.varg",
            "script Player {\n    @export var speed: Float = 6.0\n}\n",
        );

        assert_eq!(symbols[0]["name"], "Player");
        assert_eq!(symbols[0]["children"][0]["name"], "speed");
    }

    #[test]
    fn document_symbols_survive_blocking_diagnostics() {
        let symbols = document_symbols(
            "file:///game/scripts/player.varg",
            "script Player {\n    func update() {}\n}\n",
        );

        assert_eq!(symbols[0]["name"], "Player");
    }

    #[test]
    fn initialize_advertises_document_symbols() {
        let result = initialize_result();

        assert_eq!(result["capabilities"]["documentSymbolProvider"], true);
    }
}

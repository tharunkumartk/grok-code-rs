use crate::events::{AppEvent, EventSender};
use crate::tools::types::*;
use serde_json::Value;
use std::path::Path;

/// Code analysis executor
pub struct CodeExecutor {
    event_sender: EventSender,
    max_output_size: usize,
}

impl CodeExecutor {
    pub fn new(event_sender: EventSender, max_output_size: usize) -> Self {
        Self {
            event_sender,
            max_output_size,
        }
    }

    /// Truncate a JSON value if it exceeds the maximum output size
    fn truncate_result(&self, result: Value) -> Value {
        let json_str = serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string());

        if json_str.len() <= self.max_output_size {
            result
        } else {
            // Create a truncated result with a clear message
            serde_json::json!({
                "truncated": true,
                "original_size_bytes": json_str.len(),
                "max_allowed_bytes": self.max_output_size,
                "message": "The tool output was too large and has been truncated. The rest of the output was too long.",
                "note": "Output exceeded the maximum size limit to prevent excessive token usage in the conversation."
            })
        }
    }

    pub async fn execute_symbols(&self, id: String, args: Value) -> Result<(), String> {
        let _result = self.execute_symbols_with_result(id, args).await?;
        Ok(())
    }

    pub async fn execute_symbols_with_result(&self, id: String, args: Value) -> Result<Value, String> {
        let args: CodeSymbolsArgs = serde_json::from_value(args)
            .map_err(|e| format!("Invalid CodeSymbols arguments: {}", e))?;

        // Send progress event
        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: format!("Analyzing symbols in: {}", args.path),
        }).ok();

        let path = Path::new(&args.path);
        
        // Check if file exists
        if !path.exists() {
            return Err(format!("File not found: {}", args.path));
        }

        if !path.is_file() {
            return Err(format!("Path is not a file: {}", args.path));
        }

        // Read file contents
        let content = tokio::fs::read_to_string(&args.path).await
            .map_err(|e| format!("Failed to read file {}: {}", args.path, e))?;

        // Detect language
        let language = args.language.unwrap_or_else(|| {
            detect_language_from_path(path).unwrap_or_else(|| "unknown".to_string())
        });

        // Extract symbols based on language
        let symbols = extract_symbols(&content, &language, args.symbol_types.as_deref());

        let result = CodeSymbolsResult {
            symbols,
            language,
        };

        let result_value = serde_json::to_value(result).unwrap();
        let truncated_result = self.truncate_result(result_value.clone());

        // Send result event for UI
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: result_value,
        }).ok();

        Ok(truncated_result)
    }
}

// Helper functions for code analysis
fn detect_language_from_path(path: &Path) -> Option<String> {
    path.extension()?.to_str().map(|ext| {
        match ext {
            "rs" => "rust",
            "js" | "jsx" => "javascript", 
            "ts" | "tsx" => "typescript",
            "py" => "python",
            "java" => "java",
            "cpp" | "cc" | "cxx" => "cpp",
            "c" => "c",
            "go" => "go",
            "rb" => "ruby",
            "php" => "php",
            "cs" => "csharp",
            "swift" => "swift",
            "kt" => "kotlin",
            "scala" => "scala",
            "clj" | "cljs" => "clojure",
            _ => "unknown",
        }.to_string()
    })
}

fn extract_symbols(content: &str, language: &str, symbol_types: Option<&[String]>) -> Vec<CodeSymbol> {
    let mut symbols = Vec::new();
    
    // Simple regex-based symbol extraction for common languages
    match language {
        "rust" => extract_rust_symbols(content, &mut symbols, symbol_types),
        "javascript" | "typescript" => extract_js_symbols(content, &mut symbols, symbol_types),
        "python" => extract_python_symbols(content, &mut symbols, symbol_types),
        "java" => extract_java_symbols(content, &mut symbols, symbol_types),
        _ => {
            // Generic extraction for unknown languages
            extract_generic_symbols(content, &mut symbols);
        }
    }
    
    symbols
}

fn extract_rust_symbols(content: &str, symbols: &mut Vec<CodeSymbol>, _symbol_types: Option<&[String]>) {
    let lines: Vec<&str> = content.lines().collect();
    
    for (line_num, line) in lines.iter().enumerate() {
        let line_number = (line_num + 1) as u32;
        let trimmed = line.trim();
        
        // Functions
        if let Some(fn_match) = extract_rust_function(trimmed) {
            symbols.push(CodeSymbol {
                name: fn_match,
                symbol_type: "function".to_string(),
                line_start: line_number,
                line_end: line_number,
                scope: None,
                visibility: get_rust_visibility(trimmed),
            });
        }
        
        // Structs
        if let Some(struct_match) = extract_rust_struct(trimmed) {
            symbols.push(CodeSymbol {
                name: struct_match,
                symbol_type: "struct".to_string(),
                line_start: line_number,
                line_end: line_number,
                scope: None,
                visibility: get_rust_visibility(trimmed),
            });
        }
        
        // Enums
        if let Some(enum_match) = extract_rust_enum(trimmed) {
            symbols.push(CodeSymbol {
                name: enum_match,
                symbol_type: "enum".to_string(),
                line_start: line_number,
                line_end: line_number,
                scope: None,
                visibility: get_rust_visibility(trimmed),
            });
        }
        
        // Traits
        if let Some(trait_match) = extract_rust_trait(trimmed) {
            symbols.push(CodeSymbol {
                name: trait_match,
                symbol_type: "trait".to_string(),
                line_start: line_number,
                line_end: line_number,
                scope: None,
                visibility: get_rust_visibility(trimmed),
            });
        }
        
        // Modules
        if let Some(mod_match) = extract_rust_module(trimmed) {
            symbols.push(CodeSymbol {
                name: mod_match,
                symbol_type: "module".to_string(),
                line_start: line_number,
                line_end: line_number,
                scope: None,
                visibility: get_rust_visibility(trimmed),
            });
        }
    }
}

fn extract_rust_function(line: &str) -> Option<String> {
    if line.contains("fn ") {
        let parts: Vec<&str> = line.split("fn ").collect();
        if parts.len() > 1 {
            let after_fn = parts[1];
            let name_end = after_fn.find('(').unwrap_or(after_fn.len());
            let name = after_fn[..name_end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn extract_rust_struct(line: &str) -> Option<String> {
    if line.contains("struct ") {
        let parts: Vec<&str> = line.split("struct ").collect();
        if parts.len() > 1 {
            let after_struct = parts[1];
            let name_end = after_struct.find(|c: char| c.is_whitespace() || c == '{' || c == '<')
                .unwrap_or(after_struct.len());
            let name = after_struct[..name_end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn extract_rust_enum(line: &str) -> Option<String> {
    if line.contains("enum ") {
        let parts: Vec<&str> = line.split("enum ").collect();
        if parts.len() > 1 {
            let after_enum = parts[1];
            let name_end = after_enum.find(|c: char| c.is_whitespace() || c == '{' || c == '<')
                .unwrap_or(after_enum.len());
            let name = after_enum[..name_end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn extract_rust_trait(line: &str) -> Option<String> {
    if line.contains("trait ") {
        let parts: Vec<&str> = line.split("trait ").collect();
        if parts.len() > 1 {
            let after_trait = parts[1];
            let name_end = after_trait.find(|c: char| c.is_whitespace() || c == '{' || c == '<')
                .unwrap_or(after_trait.len());
            let name = after_trait[..name_end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn extract_rust_module(line: &str) -> Option<String> {
    if line.contains("mod ") && !line.contains("//") {
        let parts: Vec<&str> = line.split("mod ").collect();
        if parts.len() > 1 {
            let after_mod = parts[1];
            let name_end = after_mod.find(|c: char| c.is_whitespace() || c == ';' || c == '{')
                .unwrap_or(after_mod.len());
            let name = after_mod[..name_end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn get_rust_visibility(line: &str) -> Option<String> {
    if line.starts_with("pub ") {
        Some("public".to_string())
    } else if line.starts_with("pub(") {
        Some("restricted".to_string())
    } else {
        Some("private".to_string())
    }
}

fn extract_js_symbols(content: &str, symbols: &mut Vec<CodeSymbol>, _symbol_types: Option<&[String]>) {
    let lines: Vec<&str> = content.lines().collect();
    
    for (line_num, line) in lines.iter().enumerate() {
        let line_number = (line_num + 1) as u32;
        let trimmed = line.trim();
        
        // Functions
        if let Some(fn_match) = extract_js_function(trimmed) {
            symbols.push(CodeSymbol {
                name: fn_match,
                symbol_type: "function".to_string(),
                line_start: line_number,
                line_end: line_number,
                scope: None,
                visibility: None,
            });
        }
        
        // Classes
        if let Some(class_match) = extract_js_class(trimmed) {
            symbols.push(CodeSymbol {
                name: class_match,
                symbol_type: "class".to_string(),
                line_start: line_number,
                line_end: line_number,
                scope: None,
                visibility: None,
            });
        }
    }
}

fn extract_js_function(line: &str) -> Option<String> {
    // Function declarations
    if line.contains("function ") {
        let parts: Vec<&str> = line.split("function ").collect();
        if parts.len() > 1 {
            let after_fn = parts[1];
            let name_end = after_fn.find('(').unwrap_or(after_fn.len());
            let name = after_fn[..name_end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    
    // Arrow functions assigned to variables
    if line.contains(" => ") && line.contains("const ") {
        let parts: Vec<&str> = line.split("const ").collect();
        if parts.len() > 1 {
            let after_const = parts[1];
            let name_end = after_const.find(" =").unwrap_or(after_const.len());
            let name = after_const[..name_end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    
    None
}

fn extract_js_class(line: &str) -> Option<String> {
    if line.contains("class ") {
        let parts: Vec<&str> = line.split("class ").collect();
        if parts.len() > 1 {
            let after_class = parts[1];
            let name_end = after_class.find(|c: char| c.is_whitespace() || c == '{' || c == 'e')
                .unwrap_or(after_class.len());
            let name = after_class[..name_end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn extract_python_symbols(content: &str, symbols: &mut Vec<CodeSymbol>, _symbol_types: Option<&[String]>) {
    let lines: Vec<&str> = content.lines().collect();
    
    for (line_num, line) in lines.iter().enumerate() {
        let line_number = (line_num + 1) as u32;
        let trimmed = line.trim();
        
        // Functions
        if let Some(fn_match) = extract_python_function(trimmed) {
            symbols.push(CodeSymbol {
                name: fn_match,
                symbol_type: "function".to_string(),
                line_start: line_number,
                line_end: line_number,
                scope: None,
                visibility: None,
            });
        }
        
        // Classes
        if let Some(class_match) = extract_python_class(trimmed) {
            symbols.push(CodeSymbol {
                name: class_match,
                symbol_type: "class".to_string(),
                line_start: line_number,
                line_end: line_number,
                scope: None,
                visibility: None,
            });
        }
    }
}

fn extract_python_function(line: &str) -> Option<String> {
    if line.starts_with("def ") {
        let parts: Vec<&str> = line.split("def ").collect();
        if parts.len() > 1 {
            let after_def = parts[1];
            let name_end = after_def.find('(').unwrap_or(after_def.len());
            let name = after_def[..name_end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn extract_python_class(line: &str) -> Option<String> {
    if line.starts_with("class ") {
        let parts: Vec<&str> = line.split("class ").collect();
        if parts.len() > 1 {
            let after_class = parts[1];
            let name_end = after_class.find(|c: char| c.is_whitespace() || c == '(' || c == ':')
                .unwrap_or(after_class.len());
            let name = after_class[..name_end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn extract_java_symbols(content: &str, symbols: &mut Vec<CodeSymbol>, _symbol_types: Option<&[String]>) {
    let lines: Vec<&str> = content.lines().collect();
    
    for (line_num, line) in lines.iter().enumerate() {
        let line_number = (line_num + 1) as u32;
        let trimmed = line.trim();
        
        // Classes
        if let Some(class_match) = extract_java_class(trimmed) {
            symbols.push(CodeSymbol {
                name: class_match,
                symbol_type: "class".to_string(),
                line_start: line_number,
                line_end: line_number,
                scope: None,
                visibility: get_java_visibility(trimmed),
            });
        }
        
        // Methods (simplified)
        if let Some(method_match) = extract_java_method(trimmed) {
            symbols.push(CodeSymbol {
                name: method_match,
                symbol_type: "function".to_string(),
                line_start: line_number,
                line_end: line_number,
                scope: None,
                visibility: get_java_visibility(trimmed),
            });
        }
    }
}

fn extract_java_class(line: &str) -> Option<String> {
    if line.contains("class ") && !line.trim_start().starts_with("//") {
        let parts: Vec<&str> = line.split("class ").collect();
        if parts.len() > 1 {
            let after_class = parts[1];
            let name_end = after_class.find(|c: char| c.is_whitespace() || c == '{' || c == 'e')
                .unwrap_or(after_class.len());
            let name = after_class[..name_end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn extract_java_method(line: &str) -> Option<String> {
    // Simple method detection - looks for patterns with parentheses and common modifiers
    if line.contains("(") && line.contains(")") && 
       (line.contains("public") || line.contains("private") || line.contains("protected")) &&
       !line.trim_start().starts_with("//") {
        
        // Extract method name (this is a simplified approach)
        if let Some(paren_pos) = line.find('(') {
            let before_paren = &line[..paren_pos];
            if let Some(last_space) = before_paren.rfind(' ') {
                let method_name = before_paren[last_space + 1..].trim();
                if !method_name.is_empty() && method_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return Some(method_name.to_string());
                }
            }
        }
    }
    None
}

fn get_java_visibility(line: &str) -> Option<String> {
    if line.contains("public ") {
        Some("public".to_string())
    } else if line.contains("private ") {
        Some("private".to_string())
    } else if line.contains("protected ") {
        Some("protected".to_string())
    } else {
        Some("package".to_string())
    }
}

fn extract_generic_symbols(_content: &str, _symbols: &mut Vec<CodeSymbol>) {
    // Generic fallback - could implement basic pattern matching for common constructs
    // For now, just return empty to avoid false positives
}

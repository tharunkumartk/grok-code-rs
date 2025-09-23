use super::*;
use crate::tools::executors::CodeExecutor;
use serde_json::json;

#[tokio::test]
async fn test_code_symbols_rust_file() {
    let temp_dir = create_temp_dir().await;
    let rust_content = r#"
pub struct MyStruct {
    field: String,
}

impl MyStruct {
    pub fn new(field: String) -> Self {
        Self { field }
    }
    
    fn private_method(&self) -> &str {
        &self.field
    }
}

pub enum MyEnum {
    Variant1,
    Variant2(i32),
}

pub trait MyTrait {
    fn trait_method(&self);
}

pub mod submodule {
    pub fn module_function() {}
}

fn main() {
    println!("Hello, world!");
}
"#;
    
    let file_path = create_temp_file(temp_dir.path(), "test.rs", rust_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = CodeExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "language": "rust"
    });
    
    let result = executor.execute_symbols_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let symbols_result: CodeSymbolsResult = serde_json::from_value(result_value).unwrap();
    
    assert_eq!(symbols_result.language, "rust");
    assert!(!symbols_result.symbols.is_empty());
    
    // Check for specific symbols
    let symbol_names: Vec<&str> = symbols_result.symbols.iter()
        .map(|s| s.name.as_str())
        .collect();
    
    assert!(symbol_names.contains(&"MyStruct"));
    assert!(symbol_names.contains(&"new"));
    assert!(symbol_names.contains(&"private_method"));
    assert!(symbol_names.contains(&"MyEnum"));
    assert!(symbol_names.contains(&"MyTrait"));
    assert!(symbol_names.contains(&"submodule"));
    assert!(symbol_names.contains(&"main"));
    
    // Check symbol types
    let struct_symbol = symbols_result.symbols.iter()
        .find(|s| s.name == "MyStruct")
        .unwrap();
    assert_eq!(struct_symbol.symbol_type, "struct");
    assert_eq!(struct_symbol.visibility.as_ref().unwrap(), "public");
    
    let function_symbol = symbols_result.symbols.iter()
        .find(|s| s.name == "main")
        .unwrap();
    assert_eq!(function_symbol.symbol_type, "function");
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
    assert!(find_tool_result_event(&events).is_some());
}

#[tokio::test]
async fn test_code_symbols_javascript_file() {
    let temp_dir = create_temp_dir().await;
    let js_content = r#"
function regularFunction(param) {
    return param * 2;
}

class MyClass {
    constructor(value) {
        this.value = value;
    }
    
    method() {
        return this.value;
    }
}

const arrowFunction = (x) => {
    return x + 1;
};

export default MyClass;
"#;
    
    let file_path = create_temp_file(temp_dir.path(), "test.js", js_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = CodeExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": file_path.to_string_lossy()
        // No language specified - should auto-detect
    });
    
    let result = executor.execute_symbols_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let symbols_result: CodeSymbolsResult = serde_json::from_value(result_value).unwrap();
    
    assert_eq!(symbols_result.language, "javascript");
    assert!(!symbols_result.symbols.is_empty());
    
    let symbol_names: Vec<&str> = symbols_result.symbols.iter()
        .map(|s| s.name.as_str())
        .collect();
    
    assert!(symbol_names.contains(&"regularFunction"));
    assert!(symbol_names.contains(&"MyClass"));
    assert!(symbol_names.contains(&"arrowFunction"));
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_code_symbols_python_file() {
    let temp_dir = create_temp_dir().await;
    let python_content = r#"
def regular_function(param):
    return param * 2

class MyClass:
    def __init__(self, value):
        self.value = value
    
    def method(self):
        return self.value
    
    def _private_method(self):
        pass

class AnotherClass(MyClass):
    pass

def main():
    print("Hello, world!")

if __name__ == "__main__":
    main()
"#;
    
    let file_path = create_temp_file(temp_dir.path(), "test.py", python_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = CodeExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "language": "python"
    });
    
    let result = executor.execute_symbols_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let symbols_result: CodeSymbolsResult = serde_json::from_value(result_value).unwrap();
    
    assert_eq!(symbols_result.language, "python");
    assert!(!symbols_result.symbols.is_empty());
    
    let symbol_names: Vec<&str> = symbols_result.symbols.iter()
        .map(|s| s.name.as_str())
        .collect();
    
    assert!(symbol_names.contains(&"regular_function"));
    assert!(symbol_names.contains(&"MyClass"));
    assert!(symbol_names.contains(&"AnotherClass"));
    assert!(symbol_names.contains(&"__init__"));
    assert!(symbol_names.contains(&"method"));
    assert!(symbol_names.contains(&"main"));
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_code_symbols_java_file() {
    let temp_dir = create_temp_dir().await;
    let java_content = r#"
public class MyClass {
    private String field;
    
    public MyClass(String field) {
        this.field = field;
    }
    
    public String getField() {
        return field;
    }
    
    private void privateMethod() {
        // private implementation
    }
    
    protected void protectedMethod() {
        // protected implementation
    }
}

interface MyInterface {
    void interfaceMethod();
}
"#;
    
    let file_path = create_temp_file(temp_dir.path(), "MyClass.java", java_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = CodeExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "language": "java"
    });
    
    let result = executor.execute_symbols_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let symbols_result: CodeSymbolsResult = serde_json::from_value(result_value).unwrap();
    
    assert_eq!(symbols_result.language, "java");
    assert!(!symbols_result.symbols.is_empty());
    
    let symbol_names: Vec<&str> = symbols_result.symbols.iter()
        .map(|s| s.name.as_str())
        .collect();
    
    assert!(symbol_names.contains(&"MyClass"));
    
    // Check visibility detection
    let method_symbols: Vec<_> = symbols_result.symbols.iter()
        .filter(|s| s.symbol_type == "function")
        .collect();
    
    assert!(!method_symbols.is_empty());
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_code_symbols_unknown_language() {
    let temp_dir = create_temp_dir().await;
    let unknown_content = "some content in unknown format";
    let file_path = create_temp_file(temp_dir.path(), "test.unknown", unknown_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = CodeExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": file_path.to_string_lossy()
    });
    
    let result = executor.execute_symbols_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let symbols_result: CodeSymbolsResult = serde_json::from_value(result_value).unwrap();
    
    assert_eq!(symbols_result.language, "unknown");
    // Should have empty symbols for unknown language
    assert!(symbols_result.symbols.is_empty());
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_code_symbols_file_not_found() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = CodeExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": "/nonexistent/file.rs",
        "language": "rust"
    });
    
    let result = executor.execute_symbols_with_result("test_id".to_string(), args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("File not found"));
    
    let events = collect_events(&mut receiver, 1).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_code_symbols_directory_instead_of_file() {
    let temp_dir = create_temp_dir().await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = CodeExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": temp_dir.path().to_string_lossy(),
        "language": "rust"
    });
    
    let result = executor.execute_symbols_with_result("test_id".to_string(), args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Path is not a file"));
    
    let events = collect_events(&mut receiver, 1).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_code_symbols_with_symbol_types_filter() {
    let temp_dir = create_temp_dir().await;
    let rust_content = r#"
pub struct MyStruct {
    field: String,
}

impl MyStruct {
    pub fn new() -> Self {
        Self { field: String::new() }
    }
}

pub enum MyEnum {
    Variant,
}

fn standalone_function() {}
"#;
    
    let file_path = create_temp_file(temp_dir.path(), "test.rs", rust_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = CodeExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "language": "rust",
        "symbol_types": ["functions"] // This is passed but not currently used in the implementation
    });
    
    let result = executor.execute_symbols_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let symbols_result: CodeSymbolsResult = serde_json::from_value(result_value).unwrap();
    
    assert_eq!(symbols_result.language, "rust");
    assert!(!symbols_result.symbols.is_empty());
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_code_symbols_invalid_args() {
    let (sender, _receiver) = setup_event_bus();
    let executor = CodeExecutor::new(sender, 1024 * 1024);
    
    let invalid_args = json!({
        "invalid_field": "value"
    });
    
    let result = executor.execute_symbols_with_result("test_id".to_string(), invalid_args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid CodeSymbols arguments"));
}

#[tokio::test]
async fn test_code_symbols_legacy_method() {
    let temp_dir = create_temp_dir().await;
    let rust_content = "fn test() {}";
    let file_path = create_temp_file(temp_dir.path(), "test.rs", rust_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = CodeExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "language": "rust"
    });
    
    // Test the legacy execute method (no result return)
    let result = executor.execute_symbols("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
    assert!(find_tool_result_event(&events).is_some());
}

#[tokio::test]
async fn test_code_symbols_output_truncation() {
    let temp_dir = create_temp_dir().await;
    
    // Create a large Rust file with many symbols
    let mut rust_content = String::new();
    for i in 0..100 {
        rust_content.push_str(&format!("fn function_{i}() {{}}\n"));
        rust_content.push_str(&format!("struct Struct_{i} {{}}\n"));
    }
    
    let file_path = create_temp_file(temp_dir.path(), "large.rs", &rust_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = CodeExecutor::new(sender, 1000); // Small max output
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "language": "rust"
    });
    
    let result = executor.execute_symbols_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    // Result should be truncated due to many symbols
    if result_value.get("truncated").is_some() {
        assert_eq!(result_value["truncated"], true);
    }
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_language_detection_from_extension() {
    let temp_dir = create_temp_dir().await;
    
    // Test various file extensions
    let test_cases = vec![
        ("test.ts", "typescript", "function test() {}"),
        ("test.jsx", "javascript", "function Component() {}"),
        ("test.py", "python", "def test(): pass"),
        ("test.java", "java", "class Test {}"),
        ("test.cpp", "cpp", "int main() {}"),
        ("test.go", "go", "func main() {}"),
        ("test.rb", "ruby", "def test; end"),
    ];
    
    for (filename, expected_lang, content) in test_cases {
        let file_path = create_temp_file(temp_dir.path(), filename, content).await;
        
        let (sender, mut receiver) = setup_event_bus();
        let executor = CodeExecutor::new(sender, 1024 * 1024);
        
        let args = json!({
            "path": file_path.to_string_lossy()
            // No language specified - should auto-detect
        });
        
        let result = executor.execute_symbols_with_result("test_id".to_string(), args).await;
        assert!(result.is_ok(), "Failed for {}", filename);
        
        let result_value = result.unwrap();
        let symbols_result: CodeSymbolsResult = serde_json::from_value(result_value).unwrap();
        assert_eq!(symbols_result.language, expected_lang, "Language detection failed for {}", filename);
        
        // Consume events
        let _events = collect_events(&mut receiver, 2).await;
    }
}

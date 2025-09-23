// Simple test script to verify the new fs.read_all_code tool works
use std::fs;

fn main() {
    // Create a test directory with some code files
    let test_dir = "test_code_dir";
    fs::create_dir_all(test_dir).unwrap();
    
    // Create some test files
    fs::write(format!("{}/test.rs", test_dir), r#"
fn main() {
    println!("Hello, Rust!");
}
"#).unwrap();
    
    fs::write(format!("{}/test.py", test_dir), r#"
def main():
    print("Hello, Python!")

if __name__ == "__main__":
    main()
"#).unwrap();
    
    fs::write(format!("{}/test.js", test_dir), r#"
function main() {
    console.log("Hello, JavaScript!");
}

main();
"#).unwrap();
    
    // Create a non-code file that should be ignored
    fs::write(format!("{}/README.txt", test_dir), "This is a text file").unwrap();
    
    println!("Test files created in {}/", test_dir);
    println!("You can now test the fs.read_all_code tool with:");
    println!("  base_path: {}", test_dir);
    println!("Expected: Should read test.rs, test.py, test.js but ignore README.txt");
}

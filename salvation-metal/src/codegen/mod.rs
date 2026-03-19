use std::fs;
use std::path::Path;

fn write_generated_file(output_dir: &str, filename: &str, content: &str) {
    let path = Path::new(output_dir).join(filename);
    fs::create_dir_all(output_dir).unwrap();
    fs::write(&path, content).unwrap();
    println!("Generated: {}", path.display());
}

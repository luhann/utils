// utils/build.rs
use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=templates");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let template_dir = Path::new(&manifest_dir).join("templates");
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("template_dirs.json");

    let mut dirs = Vec::new();
    if template_dir.exists() {
        collect_dirs(&template_dir, &template_dir, &mut dirs);
    }

    // Sort paths so the output is consistent and predictable
    dirs.sort();

    // Serialize to JSON format: ["data/processed", "data/raw", ...]
    let json_content = serde_json::to_string_pretty(&dirs).unwrap();
    fs::write(dest_path, json_content).unwrap();
}

fn collect_dirs(base_dir: &Path, current_dir: &Path, dirs: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let path = entry.path();
                if let Ok(rel_path) = path.strip_prefix(base_dir)
                    && let Some(s) = rel_path.to_str()
                {
                    dirs.push(s.to_string());
                }

                collect_dirs(base_dir, &path, dirs);
            }
        }
    }
}

use std::env;
use std::path::Path;

fn find_protoc() -> Option<String> {
    if let Ok(path) = env::var("PROTOC") {
        return Some(path);
    }
    let local = Path::new("bin/protoc");
    if local.exists() {
        return local.to_str().map(|s| s.to_string());
    }
    None
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = find_protoc() {
        env::set_var("PROTOC", &path);
    }
    tonic_build::configure().compile_protos(&["../../proto/lm_service.proto"], &["../../proto"])?;
    Ok(())
}

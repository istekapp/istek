fn main() {
    // Build Tauri
    tauri_build::build();
    
    // Compile gRPC proto files for playground
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    
    tonic_build::configure()
        .file_descriptor_set_path(out_dir.join("playground_descriptor.bin"))
        .compile_protos(&["proto/playground.proto"], &["proto"])
        .expect("Failed to compile playground proto");
}

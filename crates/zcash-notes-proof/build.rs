use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_dir = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| {
            format!(
                "Failed to find workspace directory from manifest dir: {}",
                manifest_dir.display()
            )
        })?;

    let proto_dir = workspace_dir.join("proto/lightwallet-protocol/walletrpc");
    let compact_formats = proto_dir.join("compact_formats.proto");
    let service = proto_dir.join("service.proto");

    // Validate that proto files exist
    if !compact_formats.exists() {
        return Err(format!(
            "Proto file not found: {}. Did you run 'git submodule update --init'?",
            compact_formats.display()
        )
        .into());
    }
    if !service.exists() {
        return Err(format!(
            "Proto file not found: {}. Did you run 'git submodule update --init'?",
            service.display()
        )
        .into());
    }

    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(
            &[
                compact_formats.to_str().ok_or_else(|| {
                    format!("Invalid UTF-8 in path: {}", compact_formats.display())
                })?,
                service
                    .to_str()
                    .ok_or_else(|| format!("Invalid UTF-8 in path: {}", service.display()))?,
            ],
            &[proto_dir
                .to_str()
                .ok_or_else(|| format!("Invalid UTF-8 in path: {}", proto_dir.display()))?],
        )
        .map_err(|e| format!("Failed to compile protobuf definitions: {}", e))?;

    Ok(())
}

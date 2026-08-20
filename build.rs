fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure bundled protoc from protobuf-src crate
    std::env::set_var("PROTOC", protobuf_src::protoc());

    tonic_build::configure()
        .compile_protos(&["proto/telemetry.proto"], &["proto"])?;
    Ok(())
}

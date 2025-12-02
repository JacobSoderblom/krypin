fn main() {
    tonic_build::configure()
        .build_server(true)
        .compile_protos(&["proto/hub.proto"], &["proto"])
        .expect("failed to compile gRPC protos");
}

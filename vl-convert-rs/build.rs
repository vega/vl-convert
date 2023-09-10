fn main() {
    // Setup protoc environment variable
    #[cfg(feature = "protobuf-src")]
    std::env::set_var("PROTOC", protobuf_src::protoc());
}

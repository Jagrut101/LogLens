fn main() {
    println!("cargo:rerun-if-changed=native/include/loglens_engine.h");
    println!("cargo:rerun-if-changed=native/src/loglens_engine.cpp");
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .include("native/include")
        .file("native/src/loglens_engine.cpp")
        .flag_if_supported("-O3")
        .compile("loglens_native");
}

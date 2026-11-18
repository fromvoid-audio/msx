fn main() {
    println!("cargo:rerun-if-env-changed=MXH_DEV_BUILD_VERSION");
}

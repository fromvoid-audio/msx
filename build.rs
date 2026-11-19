fn main() {
    println!("cargo:rerun-if-env-changed=MSX_DEV_BUILD_VERSION");
}

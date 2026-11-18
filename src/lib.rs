#[cfg(not(target_os = "macos"))]
compile_error!("MXH supports macOS only.");

#[cfg(target_os = "macos")]
mod analyzer;
#[cfg(target_os = "macos")]
mod session;
#[cfg(target_os = "macos")]
mod timeline;
#[cfg(target_os = "macos")]
mod vst3;

#[cfg(not(target_os = "macos"))]
compile_error!("MSX supports macOS only.");

#[cfg(target_os = "macos")]
mod analyzer;
#[cfg(target_os = "macos")]
mod session;
#[cfg(target_os = "macos")]
mod vst3;

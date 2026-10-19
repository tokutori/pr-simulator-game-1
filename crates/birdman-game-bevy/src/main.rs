#![doc = "Desktop native Screen entry point; the authoritative game session remains platform independent."]

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
mod app;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn main() {
    eprintln!("birdman-game-bevy requires Windows, Linux or macOS.");
    std::process::exit(1);
}

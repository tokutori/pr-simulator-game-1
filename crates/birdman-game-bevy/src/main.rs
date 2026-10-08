#![doc = "Windows native Screen entry point; the authoritative game session remains platform independent."]

#[cfg(target_os = "windows")]
mod app;

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("birdman-game-bevy is currently a Windows native Screen prototype.");
    std::process::exit(1);
}

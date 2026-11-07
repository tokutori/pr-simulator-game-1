//! Native stdin/stdout adapter for the cooperative controller guest.

use birdman_game_format::external_controller::{HEADER_BYTES, MAX_FRAME_BYTES};
use std::io::{self, Read, Write};

fn run() -> io::Result<()> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut request = [0u8; MAX_FRAME_BYTES];
    let mut response = [0u8; MAX_FRAME_BYTES];
    loop {
        // EOF at a frame boundary is normal retirement; an incomplete frame is an error.
        match input.read(&mut request[..1]) {
            Ok(0) => return Ok(()),
            Ok(_) => {}
            Err(cause) if cause.kind() == io::ErrorKind::Interrupted => continue,
            Err(cause) => return Err(cause),
        }
        input.read_exact(&mut request[1..HEADER_BYTES])?;
        let length = u32::from_le_bytes(request[8..12].try_into().unwrap()) as usize;
        if !(HEADER_BYTES..=MAX_FRAME_BYTES).contains(&length) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Controller frame length is outside 64..=2048",
            ));
        }
        input.read_exact(&mut request[HEADER_BYTES..length])?;
        let length = controller_sample_guest::exchange_frame(&request[..length], &mut response)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Controller binary request was rejected",
                )
            })?;
        output.write_all(&response[..length])?;
        output.flush()?;
    }
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(cause) => {
            eprintln!("Controller stdio: {cause}");
            std::process::ExitCode::FAILURE
        }
    }
}

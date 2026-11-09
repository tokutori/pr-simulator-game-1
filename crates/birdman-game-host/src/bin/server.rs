//! Native management entry point; early CLI executes before assets or graphics.

fn main() {
    use birdman_game_host::{
        DECLARED_SOURCE_SHA,
        server_cli::{HELP, ServerInvocation, parse},
    };
    let result = match parse(std::env::args().skip(1)) {
        Ok(ServerInvocation::Help) => {
            print!("{HELP}");
            Ok(())
        }
        Ok(ServerInvocation::Version) => {
            println!(
                "birdman-game-server {} source {} {} {}",
                env!("CARGO_PKG_VERSION"),
                DECLARED_SOURCE_SHA.unwrap_or("unrecorded"),
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            Ok(())
        }
        Ok(ServerInvocation::Run(options)) => match DECLARED_SOURCE_SHA {
            Some(source) => birdman_game_host::admin::run(options, source),
            None => Err("配布ソース版を指定してビルドする: BPG_NATIVE_SOURCE_SHA".into()),
        },
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

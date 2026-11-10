//! CPU-only command-line parsing before native administration or bundle initialization.

use std::{net::SocketAddr, path::PathBuf};

/// Startup paths and listener selected by the native operator.
pub struct ServerOptions {
    /// Maximum simultaneous connections, including a host display peer.
    pub peer_limit: usize,
    /// Native HTTP/HTTPS socket address.
    pub bind: SocketAddr,
    /// Directory containing the fixed-source manifest and Web assets.
    pub web_root: PathBuf,
    /// Native flight-record storage directory.
    pub records: PathBuf,
    /// Optional PEM certificate chain for HTTPS.
    pub certificate: Option<PathBuf>,
    /// Optional PEM private key for HTTPS.
    pub private_key: Option<PathBuf>,
}

/// CPU-only early output or a complete native administration invocation.
pub enum ServerInvocation {
    /// Print usage and exit before accessing assets or graphics.
    Help,
    /// Print source provenance and exit before initializing the application.
    Version,
    /// Open native administration with the parsed settings.
    Run(ServerOptions),
}

/// Parse all options without accessing files, graphics or the application runtime.
pub fn parse(arguments: impl IntoIterator<Item = String>) -> Result<ServerInvocation, String> {
    let arguments: Vec<_> = arguments.into_iter().collect();
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        return Ok(ServerInvocation::Help);
    }
    if arguments
        .iter()
        .any(|argument| argument == "--version" || argument == "-V")
    {
        return Ok(ServerInvocation::Version);
    }
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let root = executable
        .parent()
        .ok_or("実行ファイルの配置場所を取得できない。")?;
    let mut options = ServerOptions {
        peer_limit: birdman_game_runtime::APPLICATION_DEFAULT_PEER_LIMIT,
        bind: "127.0.0.1:8080".parse().expect("fixed socket address"),
        web_root: root.join("web"),
        records: root.join("records"),
        certificate: None,
        private_key: None,
    };
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("{option} の値を指定する。"))?;
        match option.as_str() {
            "--bind" => {
                options.bind = value
                    .parse()
                    .map_err(|error| format!("待受アドレスを確認する: {error}"))?
            }
            "--max-peers" => {
                options.peer_limit = value
                    .parse()
                    .map_err(|error| format!("接続上限を確認する: {error}"))?;
                birdman_game_runtime::ApplicationServer::validate_peer_limit(options.peer_limit)
                    .map_err(|error| error.to_string())?;
            }
            "--web-root" => options.web_root = value.into(),
            "--records" => options.records = value.into(),
            "--tls-cert" => options.certificate = Some(value.into()),
            "--tls-key" => options.private_key = Some(value.into()),
            _ => return Err(format!("不明なオプション: {option}")),
        }
    }
    if options.certificate.is_some() != options.private_key.is_some() {
        return Err("TLS証明書と秘密鍵を両方指定する。".into());
    }
    Ok(ServerInvocation::Run(options))
}

/// Native administration usage printed by the early command-line path.
pub const HELP: &str = "鳥人間滑空ゲーム 配信サーバー\n\
使用方法: birdman-game-server [options]\n\
  --bind IP:PORT     待受アドレス（既定: 127.0.0.1:8080）\n\
  --max-peers COUNT  接続上限（既定8、1〜16、ホスト自身を含む）\n\
  --web-root PATH    同版Web bundleの配置先\n\
  --records PATH     飛行記録の保存先\n\
  --tls-cert PATH    HTTPS証明書チェーン（PEM）\n\
  --tls-key PATH     HTTPS秘密鍵（PEM）\n\
  --help            操作方法を表示\n\
  --version         バージョン・ソース版・対象OS/CPUを表示\n";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn early_cli_and_explicit_listener_options() {
        assert!(matches!(
            parse(["--help".into()]).unwrap(),
            ServerInvocation::Help
        ));
        assert!(matches!(
            parse(["--version".into()]).unwrap(),
            ServerInvocation::Version
        ));
        let ServerInvocation::Run(options) = parse([
            "--bind".into(),
            "[::1]:0".into(),
            "--tls-cert".into(),
            "chain.pem".into(),
            "--tls-key".into(),
            "key.pem".into(),
        ])
        .unwrap() else {
            panic!()
        };
        assert_eq!(options.bind.to_string(), "[::1]:0");
        assert_eq!(options.peer_limit, 8);
        for limit in [1, 16] {
            let ServerInvocation::Run(options) =
                parse(["--max-peers".into(), limit.to_string()]).unwrap()
            else {
                panic!()
            };
            assert_eq!(options.peer_limit, limit);
        }
        for limit in [0, 17] {
            assert!(parse(["--max-peers".into(), limit.to_string()]).is_err());
        }
        assert!(parse(["--tls-cert".into(), "chain.pem".into()]).is_err());
    }
}

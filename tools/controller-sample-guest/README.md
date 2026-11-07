# 共有制御則のWasmサンプル

機体・制御設定から受信したInitを使い、共有RustのPID・操縦入力合成・迎角保護を実行するABI 1の協調コントローラーである。外部コントローラーの完成した2尾翼要求を返し、ホストが共通の構造stop・slewと物理積分を適用する。

## ビルド

このディレクトリで次を実行する。

```powershell
cargo build --release --locked --lib --target wasm32-unknown-unknown
```

成果物は `target/wasm32-unknown-unknown/release/controller_sample_guest.wasm` に生成する。`CARGO_TARGET_DIR` を指定した場合はそのディレクトリを起点とする。

## Native stdio runner

同じディレクトリで `cargo build --release --locked --bin controller-sample-stdio` を実行する。Native接続文書の `program` に生成された実行ファイルの絶対パスを指定し、実行ファイルのSHA-256を `binary_sha256` に記載する。

stdin/stdoutは共通ABIのbinary frameだけを運び、各frameの応答後にflushする。診断はstderrへ出力する。EOFで終了し、serial用CRCはstdioへ付加しない。制御則と候補採用はWasmと同じ唯一のGuestを使う。

## ABIと状態

- エクスポート: `memory`、`controller_abi_version() -> i32`、`controller_buffer() -> i32`、`controller_exchange(i32) -> i32`。
- バッファ: `controller_buffer()` から4096 bytes。入力は先頭2048 bytes、応答はその直後2048 bytes。ホストは単一スレッドで呼び出し、各呼出し間にバッファを読み書きする。
- 候補評価: Evaluateで共有coreを一度呼び、最終角要求を返す。PID履歴はAcceptedで通知された実採用時間だけ進める。
- 境界と取消: ExistingBoundary・CancelledCandidateは候補を破棄し履歴を保持する。AppliedBoundaryは採用された構造・実舵角を照合し、0秒の履歴を保持する。Pauseで再Initしない。
- 共有経路: Initの設定構築、正規Euler分解、制御候補、構造阻害の照合、実時間の履歴採用は既存coreを使う。固定フレームはformatのcodecを使う。
- 所有: 状態とバッファは標準ライブラリのMutexが所有する。3つのABIエクスポート属性だけを明示的に許容し、メモリー操作には安全なRustを使う。

## 検証

`cargo test --locked --lib` は実codecのInit・Evaluate・Accepted・Finishを通し、取消、既存接触、同時刻の舵採用、部分区間、全区間の候補と履歴を共有coreの結果へ照合する。実Wasmのimport、export、メモリー、fuelと共有Wasmiでの動作はホストの受入で確認する。

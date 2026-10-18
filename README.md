# Inside（sysarium）

LinuxのCPU活動量と競合を，働くPCキャラクターとライブの数値・グラフで見るターミナルモニター．

```sh
cargo install --path apps/inside --locked
inside
```

Linux，Rust 1.88以降，UTF-8対応の端末が必要．SSHのPTYでも利用可能．`inside live`も同じ画面を開く．

![Split表示（合成fixture，80×24）](docs/validation/synthetic-b-80x24.svg)

画面例は合成データであり，実機測定の結果ではない．

| 操作 | 動作 |
|---|---|
| `Tab` / `1`・`2`・`3` | Living／Metrics／Splitへ切替 |
| `i` | 取得元・計算式・欠測理由のライブ詳細 |
| `?` | 操作・指標の説明 |
| `↑↓`・`PgUp`・`PgDn`・`Home` | 詳細・ヘルプをスクロール |
| `Esc` | 詳細・ヘルプを閉じる |
| `q` / `Ctrl-C` | 終了 |

最初はSplit表示．80×24以上ではキャラクターとメトリクスを横に並べ，狭い端末では縦並びにする．40×24以上で主要値を表示し，それ未満では全情報の同時表示を保証しない．観測は約1Hz，描画は最大10FPS，履歴は最大120観測．画面切替・リサイズで観測は増えない．

CPU活動量に応じてキャラクターの手が動き，CPU pressureに応じて書類が積まれる．書類は競合の強さの比喩で，待機タスクの個数ではない．説明は`?`，元の値と単位・根拠は`i`で確認できる．詳細も最新の観測に追随する．

初回・欠測・不正値は`N/A`や`?`で示し，ゼロに置き換えない．履歴は左が過去，右が最新，縦軸0〜100%．`▁`は計測できたゼロ，`?`は欠測．同じAnalysisからキャラクター・メトリクス・履歴を更新する．

## JSONでライブ観測する

```sh
inside --json > observations.ndjson
# Ctrl-Cで終了
```

約1秒ごとに1行1JSONを出力・flushする．`inside live --json`も同じ動作．初回は区間値が`unavailable`，取得できた参考値は保持する．終了シグナルで終了し，パイプの読取側が閉じた場合も正常終了する．単発の`status`／`inspect`コマンドは廃止した．

各レコードは`schema_version: 1`，`version`，`sample_id`，`sampled_at_unix_ms`，`scope`，`metrics`，`evidence`，`warnings`，`limitations`を持つ．各指標は`status`・`value`・`unit`，欠測時は`value: null`と構造化`reason`を保持する．割合は`ratio`（0〜1），avg10は`percent`，CPU時間は`USER_HZ_ticks`，PSI累積値はμs．stdoutはJSONのみ，診断はstderr．

`--help`と`--version`は観測せずに終了する．引数不正は終了コード2，端末・出力エラーは1，通常終了・欠測・パイプ切断は0．

## 指標と制約

- CPU：`Δ(user+nice+system+irq+softirq) / Δ(user+nice+system+idle+iowait+irq+softirq+steal)`．本プロジェクトで定義した活動量の近似値で，topのCPU表示と完全一致するとは限らない．guestは重複加算しない．
- CPU pressure：CPU PSI `some`の`Δtotal_us / 単調時計の経過μs`．avg10はカーネルの平滑化された傾向で，区間値とは異なる．
- Runnable：`procs_running`．実行中も含む参考値で，正確な待機数ではない．

カウンタ逆行，分母0，範囲外，欠測・不正は理由付き不明．一方の取得失敗で他方の正常値を捨てない．欠測復帰直後は差分なし，次の正常観測で再開する．

対象は可視の`/proc`によるsystem-view．コンテナ・VMで物理ホスト全体と一致するとは限らず，ファイルの読込は原子的ではない．PSI未対応・アクセス制限でも動作を継続する．書類の段階しきい値は暫定（0.5／2／5／10／25%），活動表示の平滑化はα=0.5．実機負荷比較による校正は未実施．

## 開発と検証

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 apps/inside/tests/verify_terminal.py
cargo build --release --locked -p inside
```

CoreはOS・Ratatui非依存，Linux crateだけがprocfsを読み，アプリが観測ループと表示を担当する3 crate構成．fixtureによる解析・欠測・Scene・描画・ストリーム出力のテストと，PTYによる操作・リサイズ・終了・シグナル・エラー／panic時の端末復旧の検証を用意している．高負荷ワークロードを自動起動しない．

GUI，Agent，Simulation，長期保存は対象外．[設計](docs/ARCHITECTURE.md)，[判断記録](docs/DECISIONS.md)，[検証報告](docs/P3-VALIDATION.md)，[出荷確認](docs/RELEASE.md)を参照．正式リリースは未公開．実端末・SSHでの見え方と，制限環境での手動負荷比較は引き続き確認が必要．

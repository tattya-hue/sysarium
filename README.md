# Inside（sysarium）

LinuxのCPU活動量と競合を観測するRustプロジェクト．P0〜P2の主要機能とP3の合成fixture・表示評価を実装済み．制限環境での手動負荷実験と利用者評価は未実施．

## 環境・検証

Rust 2024 edition対応のツールチェーン（Rust 1.88以降）が必要．Core・Linux crateの依存はWorkspace内のみ．アプリ側でJSON出力に`serde_json`，TUIに`ratatui`と`crossterm`，Unixの終了シグナル処理に`signal-hook`を使用する．実ファイル取得の対象はLinux，fixtureテストはホストの`/proc`に依存しない．

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

`crates/inside-core`はOS非依存の観測型・差分解析・欠測理由・意味的Scene，`crates/inside-linux`はパースとLinuxのファイル読込，`apps/inside`はCLI・時刻計測・出力・TUIを持つ．依存方向は`inside → inside-linux → inside-core`および`inside → inside-core`．

## コマンド（P1）

```sh
cargo run -p inside -- status
cargo run -p inside -- status --json
cargo run -p inside -- inspect cpu
```

各コマンドは2サンプルを約1秒間隔で取得する．`status`はCPU活動量，CPU PSI some区間割合・avg10，runnable参考値，観測スコープ，欠測理由を表示する．`inspect cpu`は同じ解析経路に加え，前回・今回の生データ，取得元，差分，計算規則と制限事項を表示する．`--help`と各コマンドの`--help`で使用方法，`--version`で製品バージョンを表示する．これらは観測や待機を行わない．

`status --json`のstdoutは1つのJSON文書のみ．診断はstderr．`schema_version: 1`，製品`version`，`sample_id`，`sampled_at_unix_ms`（Unix epochからのミリ秒），`scope`，`metrics`，`evidence`，`warnings`，`limitations`を含む．各指標は`status`，`value`，`unit`を持つ．欠測時は`status: "unavailable"`，`value: null`，構造化された`reason`となる．入力の取得不能理由には観測のsample_id・sourceと原因も含む．

JSONの割合の単位は`ratio`（0〜1），avg10は`percent`（0〜100），CPU時間・差分は`USER_HZ_ticks`，PSI累積値・差分はμs．区間は精度を落とさない`{secs, nanos}`形式．テキストでは割合を百分率で表示する．

観測不能があっても取得できた指標を出力して終了コード0を返す．引数不正は2，出力失敗などコマンド全体のエラーは1を返す．観測は通常権限で実行できるが，PSI未対応やアクセス制限時には理由付きN/Aとなる．

GUI，Agent，Simulationは未実装．P3の実機負荷検証は未実施，しきい値は暫定値を維持している．

## ライブ表示（P2）

```sh
cargo run -p inside -- live
```

stdinとstdoutが端末であるLinux環境で使用する（SSHのPTYでも利用可能）．`Tab`でLiving → Metrics → Split → Living，`q`または`Ctrl-C`で終了する．

`i`は表示中の観測の詳細，`?`は操作・指標のヘルプを開く．詳細は開いたsample_idの観測値・単位・差分・計算根拠・Scene・理由全文を保持する．背後の計測と履歴更新は継続し，閉じると最新表示に戻る．`↑↓`・`PgUp`・`PgDn`でスクロール，`Home`で先頭，`Esc`または同じキーで閉じる．詳細・ヘルプ表示中も`q`／`Ctrl-C`で終了できる．

UnixのSIGTERM・SIGHUP・SIGINTも正常終了要求として扱い，端末を復旧して終了コード0を返す．SIGKILLなど，プロセスが後処理を実行できない終了は対象外．

- Living：1体のASCII PCがCPU活動量に応じた頻度で仕事をする．表示強度は`α=0.5`の指数平滑化を新しい観測ごとに適用する．生の計測値と解析値は変更しない．欠測時は`?`・N/Aと理由を表示し，過去の動きを保持しない．
- 仕事の山：CPU PSI someの区間割合から0〜5の比喩的な段階を表示する．**待機タスクの個数ではない**．暫定しきい値は`<0.5%:0`，`0.5〜<2%:1`，`2〜<5%:2`，`5〜<10%:3`，`10〜<25%:4`，`>=25%:5`．Coreの`PRESSURE_THRESHOLDS`に集約しており，正常・異常判定の基準ではない．欠測を段階0へ置換せず，`? 不明`と理由を表示する．
- Metrics：CPU活動量，PSI some区間割合・avg10，runnable参考値，観測区間・取得元，短期履歴を表示する．履歴は最大120観測のメモリ上のバッファ．グラフの`?`は欠測であり，観測できた0の`▁`とは区別する．
- Split：同じAnalysisのsample_idに対応するSceneとMetricsを同時に表示する．幅100列以上では横並び，それ未満では縦並び，幅40列未満または高さ24行未満では簡略表示．40×24以上では小さなPC・山・主要指標・履歴を維持する．長い文は`…`で省略し，`i`で同じ観測の理由全文を確認できる．

観測は約1Hz，描画は最大10FPS．描画・表示切替・リサイズでサンプリングを増やさない．初回は差分なしと表示し，ファイルの欠測や不正形式が発生しても観測を継続する．通常終了，初期化途中・実行中のエラー，panicのunwind，UnixのSIGTERM・SIGHUP・SIGINTでraw mode・alternate screen・カーソルを復旧する．

Sceneは表示強度・比喩の段階・意味・source_sample_idのみを持ち，ASCII，フレーム，座標，色，Ratatuiへの依存を持たない．`Scene::from_analysis(analysis, previous_scene)`は決定論的な純粋関数であり，新しい観測時にだけ呼ぶ．

## 観測と計算

アプリが観測開始時に`Instant`と壁時計を記録する．壁時計は表示専用．`elapsed_since_previous`は単調時計の実測差分であり，固定の1秒や壁時計の差分を分母として使わない．`/proc/stat`と`/proc/pressure/cpu`は順番に読むため，観測は原子的なスナップショットではなく読込時点には小さなずれがある．

Coreの`analyze(previous, current)`はI/Oや時計取得を行わない純粋な解析API．根拠として両観測と計算規則・差分を保持する．

- CPU活動量：`Δ(user+nice+system+irq+softirq) / Δ(user+nice+system+idle+iowait+irq+softirq+steal)`．先頭8カウンタごとの差分を求めてから加算する．guest / guest_niceは重複するため加算しない．iowait・stealは分母に含むが活動に含めない．一般的なtop等と完全一致するとは限らない近似値．
- CPU PSI some区間割合：`Δsome.total_us / 単調時計の経過μs`．計算にはns精度を保つ．avg10は今回の値を平滑化傾向として併記し，区間割合とは区別する．
- 初回，前回または今回の欠測・不正，カウンタ逆行，CPU差分の分母0，経過時間0・欠測，集計の整数オーバーフロー，PSI区間割合の範囲外は理由付きUnavailable．範囲外を0〜1へ丸め込まない．

CPU活動と競合は独立した2軸．解釈は各軸の`zero / non_zero / unavailable`を表し，正常・異常を判定するしきい値は設けない．一方の取得失敗で他方の正常値を破棄しない．欠測から復帰した直後は差分を算出せず，その次の正常観測で再開する．古い正常値への遡及はしない．

## P0 APIと入力規則

- `inside_linux::cpu::parse_proc_stat(&str)`：aggregate `cpu`の先頭8項目（user，nice，system，idle，iowait，irq，softirq，steal）と`procs_running`を個別の`Result`で返す．CPU時間はUSER_HZの累積tickであり，秒への変換や総和・差分計算は行わない．guest / guest_nice以降の余剰項目，未知の行，cpuN行は無視する．8項目未満は欠測扱いとし，0で補完しない．
- `inside_linux::psi::parse_cpu_psi_some(&str)`：`some`行のavg10 / avg60 / avg300（0〜100の有限の百分率）とtotal（累積μs）を返す．キーの順序は自由．未知の`key=value`は無視し，壊れたトークンは拒否する．`full`は使用しない．
- `inside_linux::collector::collect()`：`/proc/stat`と`/proc/pressure/cpu`を1回ずつ読み，取得元付きの`RawCpuReadings`を返す．テスト等では`collect_from_proc(&Path)`でprocルートを指定できる（sourceは論理的な`/proc`のパス）．時刻やsample_idの管理はアプリ側で行う．

必須項目の欠損，対象行・既知キーの重複，負数，不正数値，整数オーバーフローは明示的なエラーとなる．ファイル欠測，権限不足，その他のI/Oエラーも区別する．利用不能は`Err(UnavailableReason)`として保持し，計測できた0と区別する．片方のファイル・レコードの失敗は他の正常値を失わせない．

PSIは待機タスクの個数ではなく競合による停滞時間の指標．avg値は平滑化された傾向．`procs_running`は実行中を含むrunnableの参考値であり，正確な待ち行列長ではない．`/proc`の可視範囲はコンテナやVMの環境に依存する．

入力仕様は[Linux kernel PSI](https://docs.kernel.org/accounting/psi.html)と[proc_stat(5)](https://man7.org/linux/man-pages/man5/proc_stat.5.html)を参照．

## テスト範囲

P0のパーサfixtureに加え，P1は`crates/inside-linux/tests/fixtures/p1`の2観測を使って，計算式，単位，根拠，初回，欠測，不正，逆行，0除算，範囲外，壁時計逆行，サブμs精度を検証する．CLIも同じfixtureと待機・取得境界の注入により，実機の`/proc`や1秒の待機に依存せず出力を検証する．

P2ではSceneの全しきい値境界・平滑化・欠測復帰，キー操作，同一sample_id，履歴上限，欠測グラフ，描画の独立性，広い画面から0×0を含む極小画面までのリサイズを自動テストする．

実PTYによる終了・復旧検証はLinuxで次を実行する（Python標準ライブラリのみ，通常の/proc取得以外の負荷は起動しない）．

```sh
python3 apps/inside/tests/verify_terminal.py
```

このスクリプトは実際の`inside live`にTab・リサイズ・qを入力し，端末属性・画面・カーソルの復旧を確認する．通常の`cargo test`ではPTY専用の2テストと画面書出し専用の1テストをignoreする．PTY検証スクリプト内でエラーとpanicを注入して2テストを個別に実行する．非TTY時の拒否も確認する．

## P3の検証状況

[検証報告・手動負荷手順](docs/P3-VALIDATION.md)に，合成A〜Dの取得→解析→Scene→画面の対応，しきい値の境界感度，平滑化の遅れ，画面サイズ別の結果を記載した．[合成画面記録](docs/validation/README.md)は実機測定結果とは区別する．

自動テストやPTY補助スクリプトは高負荷ワークロードを起動しない．実機負荷実験は，CPU・時間を制限した専用VMで人が手動実行する手順のみを用意した．実機での比較と利用者の理解度評価が残っているため，MVP全体の実証完了とはしていない．

## v0.1のビルドと追加検証

```sh
cargo build --release --locked -p inside
./target/release/inside --version
# PATHへインストールする場合
cargo install --path apps/inside --locked
```

Cargo manifestにも最小Rustバージョン1.88を指定している．今回の実行環境はRust 1.95.0であり，1.88そのものの動作試験は未実施．

CLIは実行ファイルを起動して，ヘルプ・バージョン，JSONのみのstdout，不正UTF-8引数・未知オプションの診断，非TTYのlive拒否を検証する．このプロセステストは通常の/proc読込だけを行い，ファイルがない環境では理由付きUnavailableを検証する．

PTY検証は詳細・ヘルプ・スクロール・リサイズ・q終了に加え，SIGTERM／SIGHUP／SIGINTの個別送信でも端末復旧を確認する．

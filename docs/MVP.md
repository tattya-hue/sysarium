# MVP v0.1 — Implementation Plan and Acceptance Criteria

## 対象コマンド

| コマンド | 振る舞い |
|---|---|
| `sysarium` / `sysarium live` | Bothを初期表示としてライブ監視．Tab／1・2・3で切替，qで終了． |
| `sysarium --json` / `sysarium live --json` | 約1HzのNDJSONストリーム．初回・欠測理由と根拠を保持する． |
| ライブ画面の`i` | 最新の観測元・計測値・差分・単位・欠測理由へ追随する詳細． |

利用者からの指摘に基づき単発のstatus／inspectは廃止した．以下のPhase計画はこのライブ中心の仕様へ更新している．`record`，`replay`，`sim`は提供しない．

## TUIの最低要件

- Computer：余白を持つ1体のUnicodeのモニター・スタンドを持つPCキャラクター．CPU usageに応じて画面色・表情・汗を変える．CPU競合で仕事の山（0〜5段階）が変化する．
- UIは英語．詳細はCPU usage・CPU waitの値と短い定義，PSIの意味と，Wait time（直近の観測時間内にCPUを待ったms）を示す．計算式・差分カウンタはNDJSONへ集約し，通常時は1画面に収める．
- 通常画面は数値・履歴・キャラクターを優先し，内部ID・取得元・説明文は詳細・ヘルプへ集約する．
- Charts：CPU活動量，CPU PSI some（1サンプル区間とavg10），参考runnable値，短い時系列を表示する．
- Both：同じ時点に対応するComputerとChartsを並列表示する．80列×24行未満ではキャラクターを縮小・切断せず，サイズ案内とライブの主要値に切り替える．
- 計測不能時は0や空の山に見せず `N/A` または `?` と表示し，人向けの理由はi，完全な原因はNDJSONで確認できるようにする．
- 端末リサイズに対応し，`q`と一般的なエラー終了時に端末状態を復旧する．
- 画面を派手にするために，計測値と無関係なCPU競合アニメーションを発生させない．

## Phase単位の実装順序

### P0：Workspaceとパーサ（最初のCodexタスク）

- 3 crateのCargo Workspaceを作る．
- `/proc/stat` のaggregate CPU時間と `procs_running` を読み取るパーサ．
- `/proc/pressure/cpu` の`some`パーサ．
- 欠測・不正形式・権限不足・フィールド増減を扱う．
- ローカルfixtureによるパーサ単体テスト．
- このPhaseではTUIやAgentを実装しない．

### P1：観測・Analysis・CLI

- 2サンプル差分と単調時計による区間測定．
- CoreにCPU活動量，PSI some区間割合，警告・根拠のモデル．
- ライブ観測に共通の解析・根拠出力．機械向けは`--json`の継続ストリーム．
- 0除算，カウンタ逆行，初回観測，PSI欠測のテスト．

### P2：SceneとTUI

- Sceneの決定論的生成．
- Computer / Charts / Both描画．
- 1Hz観測，最大10FPS描画，120サンプル以内の時系列バッファ．
- 入力操作，端末サイズの変化，終了時の復旧をテストする．

### P3：検証・磨き込み

- CPU活動量が高く競合の小さいケースと，活動量が高く競合の大きいケースを区別できるか検証．
- 閾値の適切さ，視覚表現と実測値の対応，端末サイズごとの見やすさを評価．
- 実機負荷実験は手動かつ制限環境のみ．自動テストから高負荷ワークロードを起動しない．

## 受け入れ基準（Definition of Done）

- `cargo test --workspace` が通る．
- `cargo fmt --all -- --check` が通る．
- `cargo clippy --workspace --all-targets -- -D warnings` が通る（使用ツールチェーンで実行可能な場合）．
- CoreはRatatuiとLinux専用の取得処理に依存しない．
- 正常なfixture，余分なフィールド，欠損フィールド，不正数値，PSIファイル欠測を検証している．
- 初回サンプルをCPU活動量0%として誤表示しない．
- CPU競合が欠測なら仕事の山を0と誤表示しない．
- `--json` は1行1JSONの機械可読で，stdoutにデバッグメッセージを混入しない．
- ComputerとChartsで同じsample_idの解析結果が対応する．
- `procs_running`を待ち行列の個数として表示しない．
- READMEに環境要件，主要コマンド，指標の定義，制約事項を記す．

## テスト例（合成データ．実測値ではない）

| ケース | CPU活動量 | PSI some区間割合 | 期待結果 |
|---|---:|---:|---|
| A：高活動・低競合 | 0.93 | 0.002 | 活発なキャラクター，小さな/ない山 |
| B：高活動・高競合 | 0.92 | 0.280 | 活発なキャラクター，大きな山 |
| C：低活動・低競合 | 0.08 | 0.001 | 落ち着いたキャラクター，小さな/ない山 |
| D：PSI欠測 | 0.85 | N/A | 活動は表示，山は不明と表示 |

上記の値は解析/表示ロジックの動作確認用fixtureであり，この組み合わせが任意の実機条件で必ず発生するという意味ではない．

## 計測上の注意

- PSI `some`は待っているタスクの**数**ではない．時間割合である．
- CPU活動量の定義は `docs/ARCHITECTURE.md` を参照．一般的な`top`のCPU使用率と完全一致するとは限らない．
- コンテナ/VMで `/proc` の可視範囲やstealなどの扱いに留意する．
- 観測対象のプロセスを特定する機能はMVP範囲外．

## 参考仕様（実装前に確認する）

- Linux kernel PSI：https://docs.kernel.org/accounting/psi.html
- Linux proc_stat(5)：https://man7.org/linux/man-pages/man5/proc_stat.5.html
- Ratatui：https://docs.rs/ratatui/latest/ratatui/

## 実装・検証状況（2026-10-18）

P0〜P2の主要機能は実装済み．P3では合成A〜Dの一貫したデータ経路，しきい値とEMA，画面サイズ別の表示内容，実PTYでの終了・復旧を検証し，小さい端末の表示を修正した．詳細は`docs/P3-VALIDATION.md`を参照．

実機の高活動・低競合／高競合の比較実験と利用者の理解度評価は未実施．手動・制限環境での手順を用意した段階であり，MVP全体の実証完了や暫定しきい値の実機校正済みとはしない．

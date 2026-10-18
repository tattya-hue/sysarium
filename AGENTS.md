# Codex project instructions — Inside

## Role

このリポジトリは，Linuxの実測値を直感的な比喩表現と技術的な数値表示で結び付けるRust製CLI/TUIを開発するものです．まず `VISION.md`，`docs/ARCHITECTURE.md`，`docs/MVP.md`，`docs/DECISIONS.md` を読み，その範囲で実装してください．

## Scope discipline

- ユーザーが指定したPhaseだけを実装する．全MVPを一度に作らない．
- Agent，IPC，GUI，eBPF，Simulation，長期DB，負荷実験の自動化，プラグインを先回り実装しない．
- 必要十分な依存だけ追加し，巨大な抽象化・不要なtrait階層・複雑な非同期ランタイムを避ける．
- 想定と実装が衝突したら，根拠・トレードオフを説明する．重要な設計変更は `docs/DECISIONS.md` に追記する．

## Core boundaries

- `inside-core`にLinux固有I/O，Ratatui，Tauriの依存を導入しない．
- 数値の計測，差分計算，解釈，Sceneへのマッピング，描画を分離する．
- 観測不能をゼロとして扱わない．`Unavailable`とその理由を示す．
- 実測値と比喩の表示強度を区別し，仕事の山を待機タスク**数**と誤表示しない．
- CPUの活動量と競合の2軸は独立して取り扱う．
- 同一sample_id/観測区間からLivingとMetricsの表示を導出する．

## Implementation and tests

- Rustfmt，Clippy，単体テストを優先する．
- パース・差分計算・欠測・カウンタ逆行・観測初回・表示段階境界のテストを合成fixtureで用意する．
- TUIでraw mode/alternate screenを使用する場合，終了・エラー時に復旧する．
- `--json`のstdoutはJSONのみとし，診断はstderrへ出す．
- 通常権限で動作する範囲を優先する．root前提にしない．
- 高負荷ワークロードを無断で自動起動しない．外部のネットワークや未承認環境への操作を行わない．
- 確認していないことを「テスト済み」と報告しない．

## Communication

- 各Phaseの開始時は対象範囲と実装方針を短く述べる．
- 完了時は変更ファイル，テスト結果，残課題，設計変更の有無を示す．
- ドキュメントと利用者向け説明は日本語を基本とし，句読点は「，．」を用いる．識別子・コード・CLIメッセージは実用性を優先する．

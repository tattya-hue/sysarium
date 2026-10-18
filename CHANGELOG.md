# Changelog

## 0.1.0 — 2026-10-18

Sysariumの最初のMVP．LinuxのCPU使用率とCPU実行待ちの競合をライブで表示する．

### 機能

- `sysarium`／`sysarium live`によるライブTUI．Animation／Charts／Bothを切り替えられる．
- CPU usageとCPU competition（PSI some）の現在値，10s avg，最大120区間の履歴，runnable参考値．
- CPU使用率を緑・暖色・赤のPCキャラクターと表情・丸い汗で表現．書類はCPU競合の比喩であり，タスク数ではない．
- 起動直後は中立な待機表情と`…`を表示し，履歴は最初の差分区間から開始．取得失敗・欠測・不正値は理由付き不明として区別する．
- `i`のライブ詳細，`?`のヘルプ，スクロール，端末リサイズ．通常表示は80×24以上を必要とし，小さい端末では案内と主要値を表示する．
- `sysarium --json`／`sysarium live --json`による約1HzのNDJSON．単位・取得元・差分・計算式・欠測理由を保持する．
- 終了・エラー・panic unwind・SIGTERM／SIGHUP／SIGINT時の端末復旧．
- Rust 2024の3 crate Workspace．CoreはOS・描画技術非依存とし，Linux固有I/OはCollectorへ分離する．

### 配布・対応範囲

- 正式タグ`v0.1.0`に対応するソース配布．Linux，Rust 1.88以上，UTF-8端末が必要．ライセンスはApache-2.0．
- crates.io公開・ビルド済みバイナリ配布は含まない．GUI・Agent・Simulation・長期保存・プロセス別の原因特定は対象外．

### 既知の制約

- CPU usageは本プロジェクトの定義による近似値．I/O waitとstealは活動に含めない．
- CPU competitionはCPU別に重み付けされたPSI someの割合であり，タスク別待ち時間の合計・平均ではない．
- `/proc`の可視範囲・PSIの利用可否はカーネル，コンテナ，権限に依存する．
- キャラクターと書類の境界値は暫定．平滑化には表示上の遅れがあり，赤い状態は処理遅延・温度・障害の実測判定ではない．
- 合成fixtureと実PTYで検証している．制限VMでの手動負荷比較・境界値の実機校正，端末やフォント全般の互換性評価は未実施．README画像は合成表示例．

操作方法は`README.md`，検証の詳細は`docs/P3-VALIDATION.md`を参照．

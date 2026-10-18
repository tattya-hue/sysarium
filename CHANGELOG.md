# Changelog

## 0.1.0（リリース候補・未公開）

公開日と正式タグは，`docs/RELEASE.md`の手動確認を終えてから確定する．

### 追加

- Rust 2024 editionの3 crate Workspace：OS非依存Core，Linux Collector，CLI/TUI．
- `/proc/stat`のaggregate CPU時間・runnable参考値，CPU PSI `some`のパースと利用可能性の判定．
- 単調時計による2観測の差分解析．CPU活動量と競合を独立した2軸として扱い，単位・観測元・根拠を保持．
- `inside status`，`inside status --json`，`inside inspect cpu`，`inside live`．
- Living／Metrics／Split，意味的Scene，最大120観測の履歴．欠測・初回・不正値・逆行は理由付き不明として表示．
- 同じsample_idの詳細表示，指標・操作のヘルプ，スクロール，リサイズ対応．
- 通常終了・エラー・panic unwind，UnixのSIGTERM／SIGHUP／SIGINTでの端末復旧．
- 合成fixture・CLIプロセス・実PTYのテストと，stable／Rust 1.88のCI定義．

### 対応範囲・既知の制約

- Linuxのsystem-viewのみ．初回提供はソースからのビルド・インストール．crates.io公開やOS別バイナリ配布は行わない．
- CPU活動量は本プロジェクトの定義による近似値．PSIの山は競合の比喩であり，待機タスク数ではない．
- PSI未対応・アクセス制限時は理由付きN/A．`/proc`の可視範囲はコンテナやVMに依存する．
- 山のしきい値は暫定値．境界付近では段階が往復する．活動強度の平滑化には表示上の遅れがある．
- 40×24未満の端末は簡略表示となり，全情報の同時表示は保証しない．
- 制限環境での手動負荷比較と利用者評価は，現時点では未実施．合成画面は実機測定結果ではない．

詳細は`README.md`と`docs/P3-VALIDATION.md`を参照．

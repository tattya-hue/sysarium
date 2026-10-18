# Changelog

## 0.1.0（リリース候補・未公開）

公開日と正式タグは，`docs/RELEASE.md`の手動確認を終えてから確定する．

### 追加

- Rust 2024 editionの3 crate Workspace：OS非依存Core，Linux Collector，CLI/TUI．
- `/proc/stat`のaggregate CPU時間・runnable参考値，CPU PSI `some`のパースと利用可能性の判定．
- 単調時計による2観測の差分解析．CPU活動量と競合を独立した2軸として扱い，単位・観測元・根拠を保持．
- `sysarium`／`sysarium live`のライブ監視と`sysarium --json`のNDJSONストリーム．単発status／inspectは廃止．
- 余白・色・UnicodeのPC顔ロボット（左右対称の胴体・腕・足），書類による競合表示と複数行の時系列グラフ．通常画面の説明文を絞り，初期表示をSplitへ変更．
- Living／Metrics／Split，意味的Scene，最大120観測の履歴．欠測・初回・不正値・逆行は理由付き不明として表示．
- 最新の同じsample_idへ追随する，指標ごとに整理したライブ詳細，指標・操作のヘルプ，スクロール，リサイズ対応．
- 通常終了・エラー・panic unwind，UnixのSIGTERM／SIGHUP／SIGINTでの端末復旧．
- 合成fixture・CLIプロセス・実PTYのテストと，stable／Rust 1.88のCI定義．

### 変更

- 製品とコマンドをSysarium／sysariumへ統一．旧insideからの置換となり，アプリはapps/sysariumへ移動．
- 通常画面の最小サイズを80×24へ変更し，縮小でキャラクターが切れる表示を廃止．
- avg10をpressureへまとめ，詳細の内部型・生JSONを除去．欠測理由を人向けに説明する．

### 対応範囲・既知の制約

- Linuxのsystem-viewのみ．初回提供はソースからのビルド・インストール．crates.io公開やOS別バイナリ配布は行わない．
- CPU活動量は本プロジェクトの定義による近似値．PSIの山は競合の比喩であり，待機タスク数ではない．
- PSI未対応・アクセス制限時は理由付きN/A．`/proc`の可視範囲はコンテナやVMに依存する．
- 山のしきい値は暫定値．境界付近では段階が往復する．活動強度の平滑化には表示上の遅れがある．
- 80×24未満の端末はサイズ案内とライブ主要値を表示．端末を広げると通常画面へ戻る．
- 制限環境での手動負荷比較と利用者評価は，現時点では未実施．合成画面は実機測定結果ではない．

詳細は`README.md`と`docs/P3-VALIDATION.md`を参照．

# v0.1.0 出荷確認

状態：利用者評価によりライブ中心の表示・CLIへ改訂．正式タグ・公開日は未確定．下表のclone・install確認は旧候補時点の記録で，新しい表示の利用者確認は継続中．

## 出荷内容

最初の配布は，Gitの正式タグに対応したソース配布とする．製品名・コマンドはInside／`inside`，リポジトリ名はsysarium．LinuxでRust 1.88以降を使い，`Cargo.lock`を固定してビルドする．GUI・Agent・Simulationや長期保存は含めない．公開候補の内容・制約は`CHANGELOG.md`にまとめる．

## 判定表

| 項目 | 旧候補時点の状態 | 根拠・残作業 |
|---|---|---|
| 実装・fixture・設計文書のGit登録 | 完了 | `77d1e33`に登録．最終的な公開対象は以降の出荷準備コミットも含む |
| クリーンなcloneからのstable検証 | 完了 | Rust 1.95.0でfmt，55件の通常テスト，Clippy，実PTYが成功 |
| 最小Rust 1.88の検証 | 完了 | Rust 1.88.0で55件の通常テスト・release buildが成功（`--locked`） |
| ソースからのインストール | 完了 | cloneから専用prefixへ導入．version・JSON・実PTY liveも成功 |
| CI定義 | 準備済み | `.github/workflows/ci.yml`．GitHub上ではまだ未実行 |
| 手動の実機負荷比較 | 未実施 | 下記記録欄と`P3-VALIDATION.md`の制限VM手順 |
| 通常Linux／SSH端末の見え方 | 未実施 | 日本語・リサイズ・終了後の復旧を人が確認 |
| 利用者の理解度 | 未実施 | 下記の短い確認項目 |
| CHANGELOG・対応範囲・インストール説明 | 準備済み | `CHANGELOG.md`，`README.md` |
| 正式タグ `v0.1.0` | 保留 | 手動確認を反映し，最終コミットで出荷条件を満たしてから作る |

未実施項目は成功扱いにしない．実機負荷を起動するCIやスクリプトは追加しない．

## 技術検証の再実行

公開対象コミットを新しいディレクトリへcloneしてから実行する．手元の未追跡ファイル・既存target・グローバルPATHのinsideへ依存しないことを確認する．

```sh
git status --porcelain
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 apps/inside/tests/verify_terminal.py
cargo +1.88.0 test --workspace --locked
cargo +1.88.0 build --release --locked -p inside
cargo install --path apps/inside --locked --root "$PWD/target/install-check"
./target/install-check/bin/inside --version
./target/install-check/bin/inside --json | head -n 2
git status --porcelain
```

最初と最後のstatusが空であることを確認する．`target`はGit対象外．上記の実行は通常の/proc観測までで，高負荷を生成しない．インストール後のliveは実端末で別途確認する．

## お願いする手動確認

### 制限した専用Linux VM

`docs/P3-VALIDATION.md`の手動手順を使用する．2 vCPU程度の専用VMと時間上限を使い，人が負荷を起動する．ケースA/BのCPU活動量が同程度で高く，BのPSI区間割合がAより大きいかを確認する．条件を作れなければ「比較条件未成立」と記録する．

記録：

- 実行者・日時：未記入
- Linuxディストリビューション・kernel：未記入
- VMのCPU上限・affinity・負荷時間：未記入
- 検証したcommit：未記入
- AのCPU活動量／PSI区間割合／山の段階：未記入
- BのCPU活動量／PSI区間割合／山の段階：未記入
- 判定・観測データの保存場所：未記入

### 端末と理解度（5〜10分程度）

実端末またはSSHのPTYで`inside live`を起動する．80×24以上から始め，40×24へ縮小して元へ戻す．Tab，i，?，スクロール，Esc，qを試す．

- 日本語とPC・山が崩れず，CPU・PSI・runnableの値を読めるか．
- LivingとMetricsが同時に変化する状態を対応付けられるか（sample_idはiで確認）．
- 山の段階を「待っているタスクの個数」と誤読しないか．
- N/A・`?`を「0」と誤読しないか．
- iでライブの根拠・欠測理由を読めて，通常画面へ戻れるか．
- q終了後に文字入力・改行・カーソル・元の画面が通常どおりか．

記録：

- 確認者・日時・commit：未記入
- 端末名・接続方法・画面サイズ：未記入
- 各項目の結果と分かりにくかった点：未記入

この結果を受けて必要な修正を行い，技術検証を再実行する．

## 最終化

1. 上記記録と既知の制約を更新し，出荷対象に残す重大な不具合がないことを確認する．
2. 公開対象のcommit・CI結果・CHANGELOGを揃える．CHANGELOGの「リリース候補」を公開日付きの0.1.0へ変更する．
3. 変更をコミットし，クリーンな状態で最終チェックを通す．
4. そのcommitに注釈付き`v0.1.0`タグを作る．未確認の候補に正式タグを付けない．
5. 公開先へcommitとタグをpushし，ソース配布のリリースノートを公開する．外部公開は最終結果を確認してから行う．

手動確認の未完了時点では，タグ作成・push・GitHub Release公開は完了扱いにしない．

## 旧候補のローカル実行記録（ライブ中心改訂前）

2026-10-18，Linux 6.18.54.1-microsoft-standard-WSL2で実行した．入力は実装commit `77d1e33514767572d4fc80e6b86911903f7ac5a0`のローカルclone．元の作業ツリーの未追跡ファイルをコピーせず，独立したtargetと専用インストールprefixを使用した．Rust 1.88.0は今回minimal toolchainを導入して検証した．

- stable（Rust 1.95.0）：fmt，通常テスト55件，Clippy（警告をエラー扱い），実PTYのキー操作・エラー／panic・SIGTERM／SIGHUP／SIGINT・端末復旧が成功．
- 最小バージョン（Rust 1.88.0）：通常テスト55件，releaseビルド，ソースインストールが成功．
- 専用prefixのinside：`--version`は0.1.0，`status --json`は単独のJSON文書としてパースでき，`live`も実PTYの操作・終了・復旧に成功．
- cloneのGit状態が空であることを確認．診断のためのPython importで生成された一時的なbytecodeは除去した．
- CI YAMLの構文とmatrixをローカルで確認．GitHub-hosted runnerでのCI実行はpush後に別途確認する．

出荷準備コミットではCIと公開前の文書を追加する．アプリの機能・暫定しきい値は変更しない．最終コミットのソースアーカイブとSHA-256は，作業ツリーの`target/release-artifacts/`へ生成する（生成物自体はコミットしない）．これを手動確認用候補として使えるが，正式版の公開を意味しない．

## ライブ中心改訂後の確認

`inside`でSplitを開き，`inside --json`はNDJSONを継続出力する．旧status／inspectは提供しない．新CLI・レイアウトの通常テスト，fmt，Clippy，最小Rust，releaseビルド，PTYの結果は`docs/P3-VALIDATION.md`を参照．旧候補のアーカイブは今回の改訂を含まないため，公開時には最終コミットから作り直す．

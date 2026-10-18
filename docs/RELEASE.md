# v0.1.0 ソースリリース

公開対象はSysariumの最初のMVP．利用者の動作確認と改善依頼を反映し，明示的な公開依頼に基づいてソースを配布する．専用VMでの負荷比較としきい値の実機校正は未実施の制約として公開ノートに残す．

## 配布物

- 注釈付きタグ：`v0.1.0`．公開対象の最終コミットへ付ける．
- ソース：`sysarium-v0.1.0-source.tar.gz`（展開先`sysarium-v0.1.0/`）．Cargo.lock・LICENSEを含むGit登録済みファイルのみ．
- SHA-256：`SHA256SUMS`．
- GitHub Release：`https://github.com/tattya-hue/sysarium/releases/tag/v0.1.0`．

Linux，Rust 1.88以上，UTF-8端末を対象とし，通常画面は80×24以上を必要とする．ビルド済みバイナリ・crates.io公開は含まない．公開内容と制約は`CHANGELOG.md`を正本とする．

## 最終検証

公開対象を別ディレクトリへクリーンcloneし，既存target・未追跡ファイル・PATH上のsysariumへ依存せず確認する．

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 apps/sysarium/tests/verify_terminal.py
cargo +1.88.0 test --workspace --locked
cargo +1.88.0 build --release --locked -p sysarium
cargo +1.88.0 install --path apps/sysarium --locked --root "$PWD/target/install-check"
./target/install-check/bin/sysarium --version
./target/install-check/bin/sysarium --json | head -n 2
git status --porcelain
```

通常テストは59件．PTYを要する2件と合成画像出力1件は通常実行ではignoreされ，PTY検証は上記スクリプトで別途実行する．画像は既存のfixture書出し手順で検証済み．高負荷ワークロードは起動しない．GitHub CIは公開対象SHAに対応するstable／1.88.0の結果を確認し，未確認を成功扱いにしない．

## 公開手順

1. 最終コミットのクリーンcloneで上記を検証し，GitHub CIの結果と公開対象SHAを確認する．
2. 注釈付き`v0.1.0`タグを作成し，そのコミットから`git archive`でソースを生成する．
3. アーカイブを展開し，Cargo.lock・LICENSE・commit IDとSHA-256を確認する．生成物は`target/release-artifacts/v0.1.0/`へ置き，Gitには登録しない．
4. コミットとタグをpushし，リリースノートと配布物をGitHub Releaseへ添付する．既存タグや既存Releaseは上書きしない．
5. 公開後にReleaseのURL，タグ，draft状態，添付ファイルとダウンロードしたSHA-256を再確認する．

この手順でGitHubのリポジトリ公開範囲は変更しない．未実施の手動負荷比較・実機校正を公開済みの検証結果として扱わない．

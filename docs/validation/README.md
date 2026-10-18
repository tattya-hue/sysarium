# P3 合成fixtureの画面記録

**このディレクトリの画面は合成データであり，実機負荷測定の結果ではない．**

`synthetic-{a,b,c,d}-{140x34,100x24,80x24,40x24}.txt`は，P3のfixtureをLinux Collector・Core・Scene・Ratatui TestBackendへ通したSplit画面．描画時刻は0，sample_idは2．UTF-8の端末セルを文字列化し，行末の空白と末尾の空行を除去している．詳細・ヘルプの追加3画面も同じ合成データから生成する．

- [A：高活動・低競合，80×24](synthetic-a-80x24.txt)
- [B：高活動・高競合，80×24](synthetic-b-80x24.txt)
- [B：100×24](synthetic-b-100x24.txt)
- [B：140×34](synthetic-b-140x34.txt)
- [D：PSI欠測，40×24](synthetic-d-40x24.txt)

値・しきい値の意味と再生成方法，未実施の手動検証は[P3検証報告](../P3-VALIDATION.md)を参照．

- [Dの詳細表示，40×24](synthetic-details-d-40x24.txt)
- [ヘルプ，80×24](synthetic-help-80x24.txt)

## ライブ中心の画面改訂

記録は改訂後のレイアウトへ再生成した．通常画面のsample_idと説明ラベルを除き，詳細は最新観測へ追随する．色付きSVGも同じRatatuiバッファから生成する．

- [A：高活動・低競合（色付き）](synthetic-a-80x24.svg)
- [B：高活動・高競合（色付き）](synthetic-b-80x24.svg)
- [D：欠測（色付き）](synthetic-d-80x24.svg)

SVGの見え方は閲覧側のフォントにも依存する．

## 全身・詳細の改訂

キャラクターは顔・胴体・腕・足を持つロボットへ変更した．40×24の通常画面は縮小キャラクターではなく必要サイズの案内となる．詳細は指標ごとに整理し，80×24の通常データも追加した．

- [指標の詳細，80×24](synthetic-details-b-80x24.txt)

## 英語UIと通常PCへの改訂

画面記録はView: Computer / Charts / Both，CPU usage / CPU waitへ更新した．キャラクターは通常のモニター・スタンド・キーボード．顔も含め左右対称とし，詳細は短い英語の値・定義へ整理している．過去のロボット・Splitという名称は旧表示の記録となる．

## README用の合成動画

[低競合](../assets/busy-low-wait.gif)／[高競合](../assets/busy-high-wait.gif)．A/Bの同じ観測（sample_id=2）を使い，実際のRendererの時刻だけを100msずつ24フレーム進める．観測値・書類は固定し，CPU usageに応じてキーだけが点滅する．実機負荷や記録・再生機能を追加しない．

再生成（任意の開発作業．PythonのPillowとDejaVu Sans Monoが必要．アプリ・通常テスト・CIには不要）：

```sh
INSIDE_REVIEW_SNAPSHOTS="$PWD/docs/validation" SYSARIUM_PREVIEW_FRAMES="$PWD/target/readme-previews" cargo test -p sysarium tui::tests::p3_export_review_snapshots -- --ignored --exact
python3 scripts/render_previews.py target/readme-previews docs/assets
```

`--font`でフォントパスを指定できる．中間SVGはGit対象外のtargetへ置き，GIFとPNGだけをREADMEで使う．合成画像は実測の負荷実験結果として扱わない．

## PC本体の忙しさと落ち着いた表示

現行のREADMEプレビューはPC本体の色・表情・汗でCPU usageを示す．キーボードは撤去した．既存のA/BにC（CPU8%・wait0.1%）を加え，同じ手順でrelaxed.pngも生成する．Cは汗が出ず静止するため，PNGだけを保存する．最新GIFのフレーム差分はPC周囲の汗だけで，数値・書類は固定．これ以前のキーだけが点滅する記録は旧表示を指す．

## 汗と表情の更新

A/B/Dの忙しい口を下向きへ変更．汗は青い小さなしずくが左右別の周期で下へ流れて消える．顔・モニター・スタンドは対称を保つ．READMEのA/B動画も更新し，Cの落ち着いた表示は維持する．

## 控えめな飛び散りと表示名

現行表示はScene / Charts / Both．汗は片側ずつ短く外へ飛び，薄くなって消える．最大2セルと休止を設け，下降動作を置き換えた．READMEのA/B動画とCの静止画を含め，同じ手順で更新する．過去のComputerという表示名と垂れる汗の記述は旧表示を指す．

## 汗の形と表示名の再調整

現在の表示名はAnimation / Charts / Both．汗は先の細い1セルの粒で，同じ形のまま外へ飛び，色を薄くして消える．片側300ms，その後900ms休止し，使用率の更新で動作の位相が飛ばないよう描画時計だけで周期を決める．READMEのA/B動画とCの静止画も更新した．過去のSceneという表示名と，点・斜線の汗は旧表示を指す．

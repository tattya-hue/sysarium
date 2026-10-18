# P3 合成fixtureの画面記録

**このディレクトリの画面は合成データであり，実機負荷測定の結果ではない．**

`synthetic-{a,b,c,d}-{140x34,100x24,80x24,40x24}.txt`は，P3のfixtureをLinux Collector・Core・Scene・Ratatui TestBackendへ通したSplit画面．描画時刻は0，sample_idは2．UTF-8の端末セルを文字列化し，行末の空白と末尾の空行を除去している．詳細・ヘルプの追加2画面も同じ合成データから生成する．

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

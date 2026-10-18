# Architecture v0.1

## 基本戦略

初期段階は「3 crateのCargo Workspace・1プロセス」のモジュラーモノリスとする．Agentを必須にしない．

```text
sysarium/ (repository)
├── Cargo.toml
├── Cargo.lock
├── AGENTS.md
├── VISION.md
├── CODEX_START.md
├── docs/
│   ├── ARCHITECTURE.md
│   ├── MVP.md
│   └── DECISIONS.md
├── crates/
│   ├── inside-core/src/
│   │   ├── lib.rs
│   │   ├── observation.rs
│   │   ├── analysis.rs
│   │   ├── scene.rs
│   │   └── explanation.rs
│   └── inside-linux/src/
│       ├── lib.rs
│       ├── collector.rs
│       ├── cpu.rs
│       └── psi.rs
└── apps/
    └── sysarium/src/
        ├── main.rs
        ├── output.rs
        ├── shutdown.rs
        └── tui/
            ├── mod.rs
            ├── app.rs
            ├── renderer.rs
            └── terminal.rs
```

実装中に名前や小規模なファイル分割を調整してよいが，責務と依存方向は保持する．

## 依存方向

```text
apps/sysarium ─┬──> inside-core
             └──> inside-linux ───> inside-core
```

- `inside-core`：OSやUIに依存しない観測値の型，差分からの状態解析，説明，意味的Sceneモデル．Ratatuiやprocfsパーサを取り込まない．
- `inside-linux`：Linuxの `/proc/stat` と `/proc/pressure/cpu` を読み，OS固有のパースと利用可能性の判定を行う．
- `apps/sysarium`：CLI・TUI，ループ，表示切替，履歴バッファ，エラーと端末のライフサイクルを持つ．

`inside-core`がOSファイルに直接アクセスしないことを優先する．抽象的なCollector traitを必要なく増やさず，テスト可能性がある最小APIに留める．

## 論理的なデータフロー

```text
Linux /proc
   ↓
Raw CPU counters / PSI values     [Linux Collector]
   ↓
Observation + sampling metadata    [Core model]
   ↓
Analysis + evidence + availability [Core]
   ├──────────────────┐
   ↓                  ↓
Semantic Scene     Metric series
   ↓                  ↓
Animation renderer   Charts renderer
   └────────┬─────────┘
            ↓
     Unified TUI
```

Charts表示とAnimation表示は別々にOSをサンプリングしない．同じ解析結果，観測時刻，履歴を使う．Sceneはレンダラー固有の座標・ASCII文字・色・画像パスを持たない．

## データ契約（概念．Rustの確定型定義ではない）

### Observation

- `sample_id`：セッション内で一意．
- `sampled_at_unix_ms`：表示・出力用の壁時計時刻．
- `elapsed_since_previous`：サンプリング間隔を単調時計で測った時間．初回はなし．
- `scope`：観測対象スコープ．初期は `/proc` 経由のsystem-viewのみ．コンテナ内で物理ホスト全体と同一とは限らない．
- `cpu_times`： `/proc/stat` のaggregate `cpu` から得た累積カウンタ．
- `cpu_psi_some`： `avg10`，`avg60`，`avg300`，`total_us`．取得不可ならUnavailable（理由付き）．
- `procs_running`：利用できれば参考値として保持．CPU待機タスクの正確な個数ではない．
- 各値の `source` と欠測理由．

### Analysis

- `cpu_active_ratio`：0.0〜1.0 またはUnavailable．
- `cpu_psi_some_interval_ratio`：直近サンプリング区間で `Δsome.total_us / Δelapsed_us` を求めた0.0〜1.0 またはUnavailable．
- `cpu_psi_some_avg10_pct`：0〜100の平滑化された傾向，またはUnavailable．
- `cpu_interpretation`：活動量と競合の2軸を独立に扱った状態．
- `evidence`：入力メトリクス，元ファイル，サンプリング区間，解析規則を追える情報．
- `warnings`：初回で差分が取れない，カウンタ異常，PSI未対応，スコープの留意点など．

### Scene

- `character.activity_intensity`：活動量の表示用正規化値0.0〜1.0，またはUnavailable．
- `task_stack.pressure_level`：0〜5の**比喩的な段階**，またはUnavailable．
- `task_stack.represents`：`cpu_contention_metaphor`を明記．
- `source_sample_id`：対応する観測結果．

0は計測できた上で低い状態．Unavailableは不明であり0に置換してはならない．アニメーションのフレーム番号と表示座標はRendererが持つ．

### View state（UI側）

- `view_mode`：Animation / Charts / Both．
- `selected_resource`：初期はCPUのみ．
- `selected_sample_id`：将来の時点選択向け．初期は最新サンプル固定でよい．
- `metric_history`：短いメモリ上のリングバッファ（初期上限120サンプル）．

## 計算の定義

`/proc/stat` の最初の8 CPU時間フィールド（user, nice, system, idle, iowait, irq, softirq, steal）を扱い，`guest` と `guest_nice` は user / nice と重複するため総和に追加しない．

- `total_delta`：上記8フィールドの総和の差分．
- `active_delta`：user + nice + system + irq + softirq の差分．
- `cpu_active_ratio = active_delta / total_delta`．

ここではiowaitやstealを「キャラクターが自ら仕事をした時間」に含めない．一般的な`top`等のCPU表示区分と完全一致を保証する指標ではなく，**本プロジェクトで定義したCPU活動量の近似値**としてラベル付けする．カウンタ逆行・分母0・初回の値はUnavailableとする．

CPU PSI `some`の`total`は累積μs． `Δtotal_us / Δelapsed_us` は直近区間の停滞時間割合を表す． `avg10`は10秒窓の単純な算術平均ではなく平滑化された傾向として扱う．system-level CPU `full`は指標として使用しない．

`procs_running`は実行中も含むrunnableの参考値であり，待ち行列の長さと等置しない．`procs_running - CPU数`で正確な待機タスク数を推定しない．

## Sceneへの初期マッピング

- PCの画面色・表情・汗：`cpu_active_ratio`を表示用に平滑化する．実測・解析値自体は改変しない．
- 仕事の山：`cpu_psi_some_interval_ratio`から0〜5段階に量子化する．しきい値は説明可能な暫定ルールとし，実測検証後に変更できるよう1か所で定義する．初回の差分待ちは `…`，欠測時は `?` などの不明表示にする．
- 例示の暫定しきい値（比喩の段階であり，問題判定の臨床的・運用上の基準ではない）：`<0.5%:0`，`0.5〜<2%:1`，`2〜<5%:2`，`5〜<10%:3`，`10〜<25%:4`，`>=25%:5`．
- チラつきを避けるため，Scene側にのみ簡単な平滑化またはヒステリシスを適用できる．MVPでは無理に複雑な状態機械を作らない．

## 実行モデル

- 観測：原則1 Hz．
- 描画：最大10 FPSを初期目安とし，アニメーションとサンプリングを分離．
- `sysarium`／`sysarium live`：約1Hzで継続観測し，最新解析と短期履歴で3表示を切り替える．初期表示はBoth．
- `sysarium --json`：同じ取得・解析経路の結果を約1Hzで1行1JSONとしてflushする．初回も理由付きで出力する．
- 単発のstatus／inspectは提供しない．取得元・単位・計算式・欠測理由はライブの詳細へ集約する．
- エラー発生時：画面を落とすのではなく計測不能を示す．致命的な端末エラーでは終了処理で端末状態を復旧する．

## 将来の拡張境界

Agent・IPC・GUI・記録再生・Simulationは**現段階で実装しない**．将来，GUI/TauriがCoreを直接利用する構成を可能にする．Agent導入時も同じ観測/解析モデルを転送対象にできるよう，不要なUI型やLinuxのファイルハンドルをCoreモデルへ混入させない．

## v0.1の詳細表示と終了処理

通常画面は現在値・履歴と比喩を優先し，sample_id・取得元・計算規則は`i`の詳細へ集約する．詳細表示も同じ最新AnalysisとSceneに追随し，スクロール位置は可能な範囲で維持する．詳細はCPU usage（working）とCPU wait（waiting）の値に短い定義を添え，PSIはLinuxのCPU待ち時間の計測と説明し，Wait timeを直近の観測時間内のmsで示す．計算式・生JSON・内部型はNDJSONへ集約する．完全な観測値・原因はNDJSONに保持する．ヘルプと詳細は表示セル幅で折り返す．履歴の時点選択・記録再生・Simulationは導入しない．

Unixの終了シグナルはアプリ側のフラグへ通知し，イベントループを終了して通常の端末復旧へ進む．signal-hookはapps/sysariumのみの依存とし，Core・Linux Collectorへ持ち込まない．シグナルハンドラ内で端末I/Oや解析は行わない．

## 表示サイズと命名

製品・アプリcrate・実行ファイルはsysarium，内部ライブラリは既存のinside-core／inside-linuxを維持する．アプリはapps/sysariumへ配置する．

通常画面の最小サイズは80列×24行．RendererはPCキャラクターに固定39×15セルの描画領域を使い，全身を配置する．それ未満の端末はサイズ案内とライブ主要値へ切り替え，観測・キー入力を継続する．端末ウィンドウ自体のリサイズを制限する操作はしない．大きさが戻れば通常描画を再開する．

Wait timeの表示は既存のpsi_total_delta_usとelapsed，区間割合がすべて有効な場合に限定し，μsからmsへ換算する．新しい計測やCoreの解析指標は増やさない．割合は「少なくとも1タスクがCPUを待っていた時間」の割合で，タスクごとの待ち時間の総和ではない．

Character::activity_level()は平滑化済みactivity_intensityからRelaxed（70%未満）・Busy（70%以上95%未満）・Strained（95%以上）を純粋に導出する．Rendererは緑・暖色・赤の配色と異なる表情に対応させ，Busy以上では頭上へ丸い2滴の汗を短く飛ばす．上部3行は汗のための余白とする．境界は表示用の暫定ルールで，処理遅延・正常異常・温度の判定ではない．PSIはPCの表情・汗に使わず，書類だけに対応させる．欠測時は灰色と?を表示し，低使用率と区別する．

初回観測には差分区間がないため，TUIの履歴は2回目の観測から記録する．FirstSampleは灰色の中立な待機表情とStarting…，値は…で示す．初回でも読込失敗は待機扱いにせずN/Aと?を維持する．以降の欠測は履歴の?として残す．Coreの初回UnavailableとJSONの理由は変更しない．

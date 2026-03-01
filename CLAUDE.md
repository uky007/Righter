# Righter - 開発引き継ぎドキュメント

Rust製Vimライクテキストエディタ。TUI（ratatui/crossterm）とGUI（egui/eframe）の2つのフロントエンドを持ち、エディタコアを共有する。rust-analyzer統合による型推論・診断・補完をサポート。

## ビルドと実行

```bash
# TUI版ビルド・実行（デフォルト、Rust edition 2024 / nightly必要）
cargo build
cargo run -- <filepath>

# GUI版ビルド・実行
cargo build --features gui --bin righter-gui
cargo run --features gui --bin righter-gui -- <filepath>

# 例: 自身のソースコードを開く
cargo build && cargo run -- src/editor/mod.rs
cargo run --features gui --bin righter-gui -- src/editor/mod.rs
```

## アーキテクチャ概要

### フロントエンド共有設計

単一クレート + feature フラグ (`tui` / `gui`) で2つのバイナリを生成。エディタコア（editor/, input/, lsp/, highlight/, buffer/）はフロントエンド非依存。3つの共有型で結合点を抽象化:

| 共有型 | 定義場所 | 用途 | TUI側の変換 |
|-------|---------|------|------------|
| `KeyInput` / `KeyCode` | `key.rs` | キー入力表現 | `From<crossterm::event::KeyEvent>` |
| `AreaRect` | `editor/pane.rs` | レイアウト矩形 | `From<ratatui::layout::Rect>` / `Into<Rect>` |
| `SyntaxStyle` / `RgbColor` | `highlight/style.rs` | 構文ハイライトスタイル | `to_ratatui_style()` (ui/editor_view.rs) |

```
# 共有コア
key.rs                         # KeyInput / KeyCode — フロントエンド非依存のキー表現
editor/mod.rs (Editor)         # 全エディタ状態。カーソル、モード、バッファ、レジスタ、マクロ等
  editor/document.rs           # ropey::Rope ラッパー。ファイルI/O、テキスト変更操作
  editor/selection.rs          # Position { row, col } 構造体
  editor/view.rs               # ビューポート(スクロールオフセット、表示領域サイズ)
  editor/history.rs            # undo/redo スナップショットベース
  editor/pane.rs               # Pane, PaneNode, AreaRect, PaneRenderData — ウィンドウ分割
  editor/wrap.rs               # 行折り返し計算ユーティリティ（WrapSegment, wrap_count, build_screen_map）
input/mod.rs                   # execute() : Command → Editor メソッドのディスパッチ + .repeat追跡
  input/command.rs             # Command enum (100+バリアント) + Motion enum
  input/mode.rs                # Mode enum (Normal/Insert/Visual/VisualLine/Command/Search)
  input/keymap.rs              # KeyInput→Command変換。モード別マッピング + pending key処理
lsp/mod.rs (LspClient)        # rust-analyzer と JSON-RPC通信。リクエスト/レスポンス/通知の処理
  lsp/transport.rs             # Content-Length ヘッダベースの読み書き
buffer/mod.rs                  # 行の表示幅計算、word文字判定ユーティリティ
config.rs                      # Config { tab_width, scroll_off }
highlight/mod.rs               # tree-sitter ベースの構文ハイライト
  highlight/style.rs           # SyntaxStyle / RgbColor — フロントエンド非依存のスタイル
  highlight/theme.rs           # トークン→SyntaxStyleのマッピング

# TUI フロントエンド (#[cfg(feature = "tui")])
main.rs                        # TUIエントリポイント。ターミナルセットアップ → App::run()
app.rs (App)                   # TUIイベントループ。tokio::select! で多重化
ui/mod.rs                      # render()。全UIコンポーネントをレイヤ描画
  ui/editor_view.rs            # メインエディタ領域 + SyntaxStyle→ratatui::Style変換
  ui/status_line.rs            # モード表示 + カーソル位置 + ファイル名
  ui/command_line.rs           # `:` コマンド / `/` 検索入力
  ui/completion.rs             # LSP補完ポップアップ
  ui/hover.rs                  # LSPホバー情報ポップアップ
  ui/references.rs             # LSP参照一覧ポップアップ
  ui/code_actions.rs           # LSP Code Actionsポップアップ
  ui/diagnostics.rs            # 診断一覧ポップアップ
  ui/file_finder.rs            # ファジーファイルファインダー
  ui/tab_bar.rs                # マルチバッファのタブバー

# GUI フロントエンド (#[cfg(feature = "gui")])
gui_main.rs                    # GUIエントリポイント。eframe::run_native()
gui_app.rs (GuiApp)            # eframe::App実装。tokio::runtime::Runtime + mpscブリッジでLSP統合
gui/mod.rs                     # render()。egui CentralPanel + TopBottomPanel でレイアウト
  gui/editor_view.rs           # Painter API でテキスト描画 + SyntaxStyle→Color32変換
  gui/status_line.rs           # モード表示 + カーソル位置 + ファイル名
  gui/command_line.rs          # `:` コマンド / `/` 検索入力
  gui/completion.rs            # LSP補完ポップアップ
  gui/hover.rs                 # LSPホバー情報ポップアップ
  gui/references.rs            # LSP参照一覧ポップアップ
  gui/code_actions.rs          # LSP Code Actionsポップアップ
  gui/diagnostics.rs           # 診断一覧ポップアップ
  gui/file_finder.rs           # ファジーファイルファインダー
  gui/tab_bar.rs               # マルチバッファのタブバー
```

## 主要な設計パターン

### コマンドディスパッチ

キー入力からエディタ操作までの流れ（両フロントエンド共通）:

```
# TUI: crossterm::KeyEvent → KeyInput::from()
# GUI: egui::Key + Modifiers → egui_key_to_key_input()
KeyInput
  → keymap::map_key()     # Mode別にCommandを決定。pending_keysで複数キー操作を処理
  → input::execute()      # CommandをEditorメソッドにディスパッチ
  → Editor::method()      # 実際のテキスト操作やモード変更を実行
```

### pending_keys (複数キー操作)

`d`, `c`, `y`, `g`, `f`, `]`, `[` 等の1キー目をpending_keysに保存し、2キー目で確定:
- `dd` → DeleteLine, `dw` → DeleteMotion(WordForward)
- `gd` → GotoDefinition, `ga` → CodeAction, `gE` → DiagnosticList
- `]d` → DiagnosticNext, `[d` → DiagnosticPrev

### DeferredAction (非同期操作)

同期的にCommandをディスパッチした後、非同期処理が必要な場合はDeferredActionを返す:
- `PlayMacro(char)`, `FormatDocument`, `OpenFile(String)`, `ShellCommand(String)` 等
- TUI: `app.rs`の`handle_deferred()`が処理
- GUI: `gui_app.rs`の`handle_deferred()`が処理（`ShellCommand`は出力キャプチャ→ステータスメッセージ）

### LSPリクエスト/レスポンスパターン

1. キーマップが`Command::GotoDefinition`等を生成
2. `input::execute()`では空処理（`{}`）
3. イベントループでフラグチェック → LSPリクエスト送信
   - TUI: `app.rs` で `request_xxx().await`
   - GUI: `gui_app.rs` で `self.runtime.block_on(lsp.xxx())`
4. `pending_xxx_id`にリクエストIDを保存
5. `handle_lsp_message()`でID一致時にレスポンスを処理

### GUI固有: 非同期ブリッジ

GUIではeframeの同期`update()`ループ内でLSPの非同期処理を行うため、以下のブリッジを使用:
- `tokio::runtime::Runtime` — `block_on()`でasync LSPメソッドを同期呼び出し
- `std::sync::mpsc` — LSP受信メッセージを非同期→同期に橋渡し（`try_recv()`でポーリング）
- `ctx.request_repaint_after(100ms)` — 定期再描画でLSPメッセージを拾う

### ポップアップUIパターン

全ポップアップは同一パターン:
- `Editor`に`showing_xxx: bool`, `xxx_index: usize`, データ`Vec<T>`
- `keymap.rs`の`map_normal`冒頭で`showing_xxx`チェック → j/k/Enter/Escをインターセプト
- TUI: `ui/xxx.rs`で`Widget` traitを実装、`showing_xxx == false`なら即return
- GUI: `gui/xxx.rs`で`draw_xxx()`関数、Painter APIで描画、`showing_xxx == false`なら即return
- 各`render()`でEditorView上にオーバーレイ描画

## キーバインド一覧

### Normal Mode

| キー | コマンド | 説明 |
|------|---------|------|
| `h/j/k/l` | MoveLeft/Down/Up/Right | カーソル移動 |
| `w/b/e` | MoveWordForward/Backward/End | word単位移動 |
| `W/B/E` | MoveWORDForward/Backward/End | WORD単位移動（空白区切り） |
| `0/$` | MoveLineStart/End | 行頭/行末 |
| `^` | MoveFirstNonBlank | 最初の非空白文字 |
| `{/}` | MoveParagraph | 段落移動（空行区切り） |
| `gg/G` | GotoTop/Bottom | ファイル先頭/末尾 |
| `H/M/L` | ViewportHigh/Middle/Low | 画面上端/中央/下端 |
| `%` | MatchBracket | 対応括弧ジャンプ |
| `f/F/t/T` + char | Find/TillChar | 行内文字検索 |
| `i/a/A/I/o/O` | Insert系 | Insertモード開始 |
| `d` + motion | DeleteMotion | モーション範囲削除 |
| `c` + motion | ChangeMotion | モーション範囲変更 |
| `y` + motion / `yy` | YankMotion/Line | ヤンク |
| `p/P` | PasteAfter/Before | ペースト |
| `x` | DeleteCharForward | 文字削除 |
| `dd` | DeleteLine | 行削除 |
| `D/C` | Delete/Change to EOL | 行末まで削除/変更 |
| `J` | JoinLines | 行結合 |
| `r` + char | ReplaceChar | 文字置換 |
| `u` / `Ctrl-R` | Undo/Redo | 元に戻す/やり直し |
| `.` | RepeatLastChange | 最後の変更を繰り返し |
| `~` | ToggleCaseChar | 大文字小文字トグル |
| `gu/gU/g~` + motion | CaseChange | ケース変更 |
| `Ctrl-A/X` | Increment/Decrement | 数値増減 |
| `v/V` | EnterVisual/VisualLine | Visualモード |
| `:/` | CommandMode/SearchMode | コマンド/検索（正規表現対応、インクリメンタル検索） |
| `n/N` | SearchNext/Prev | 検索次/前 |
| `*/＃` | SearchWordForward/Backward | カーソル下の単語検索（`\b`ワード境界付き） |
| `Ctrl-D/U` | HalfPageDown/Up | 半ページスクロール |
| `Ctrl-F/B` | FullPageDown/Up | 全ページスクロール |
| `zz/zt/zb` | ScrollCenter/Top/Bottom | スクロール位置調整 |
| `>>/<<` | IndentLine/DedentLine | インデント |
| `Ctrl-P` | OpenFileFinder | ファイルファインダー |
| `Ctrl-O/I` | JumpBack/Forward | ジャンプリスト |
| `K` | Hover | LSPホバー情報 |
| `gj`/`gk` | MoveDocumentLineDown/Up | ドキュメント行単位移動（wrap時） |
| `gd` | GotoDefinition | 定義ジャンプ |
| `gr` | FindReferences | 参照検索 |
| `ga` | CodeAction | コードアクション（quick fix） |
| `gE` | DiagnosticList | 診断一覧ポップアップ |
| `gt/gT` | NextBuffer/PrevBuffer | バッファ切替 |
| `]d/[d` | DiagnosticNext/Prev | 次/前の診断へジャンプ |
| `q` + char | StartMacro | マクロ記録開始 |
| `q`（記録中） | StopMacro | マクロ記録停止 |
| `@` + char | PlayMacro | マクロ再生 |
| `@@` | PlayLastMacro | 最後のマクロ再生 |
| `"` + char | SelectRegister | レジスタ選択 |
| `Ctrl-W v` | SplitVertical | 垂直分割 |
| `Ctrl-W s` | SplitHorizontal | 水平分割 |
| `Ctrl-W h/j/k/l` | PaneLeft/Down/Up/Right | ペイン移動 |
| `Ctrl-W w` | PaneNext | 次のペインへ |
| `Ctrl-W q` | PaneClose | ペインを閉じる |
| `Ctrl-C` | Quit | 終了 |

### コマンドモード (`:`)

| コマンド | 説明 |
|---------|------|
| `:w` | 保存 |
| `:q` / `:q!` | 終了 / 強制終了 |
| `:wq` | 保存して終了 |
| `:e <path>` | ファイルを開く |
| `:rename <name>` | LSPリネーム |
| `:format` | LSPフォーマット |
| `:<number>` | 指定行にジャンプ |
| `:s/old/new/[g][i]` | 現在行の置換（正規表現対応、`g`:全置換、`i`:大文字小文字無視） |
| `:%s/old/new/[g][i]` | 全行の置換（正規表現対応、キャプチャグループ `$1`, `$2` 対応） |
| `:set wrap` | 行折り返し表示を有効化 |
| `:set nowrap` | 行折り返し表示を無効化 |
| `:!<command>` | シェルコマンド実行 |
| `:split` / `:sp` | 水平分割（`:split <file>` でファイル指定可） |
| `:vsplit` / `:vs` | 垂直分割（`:vsplit <file>` でファイル指定可） |
| `:bn` / `:bp` | 次/前バッファ |

## 依存クレート

| クレート | 用途 | feature |
|---------|------|---------|
| `ratatui 0.29` | TUIフレームワーク | `tui`（デフォルト） |
| `crossterm 0.28` | ターミナルI/O + イベント | `tui`（デフォルト） |
| `eframe 0.31` | eguiウィンドウフレームワーク | `gui` |
| `egui 0.31` | 即時モードGUIライブラリ | `gui` |
| `tokio 1` (full) | 非同期ランタイム | 常時 |
| `ropey 1.6` | Ropeデータ構造（テキストバッファ） | 常時 |
| `tree-sitter 0.24` | インクリメンタルパーサー | 常時 |
| `tree-sitter-rust 0.23` | Rust文法 | 常時 |
| `serde_json 1` | LSP JSON-RPC | 常時 |
| `anyhow 1` | エラーハンドリング | 常時 |
| `unicode-width 0.2` | Unicode文字幅 | 常時 |
| `regex 1` | 正規表現（検索・置換） | 常時 |

## 検索と置換

### 検索 (`/`)
- **正規表現対応**: `/fn\s+\w+` のようなパターンが使用可能
- **スマートケース**: クエリが全小文字なら大文字小文字を無視、大文字が含まれるとcase-sensitive
- **`\c` / `\C` サフィックス**: 明示的にcase-insensitive (`\c`) / case-sensitive (`\C`) を指定
- **インクリメンタル検索**: 入力中にリアルタイムでマッチ位置にカーソルジャンプ
- **Escで復元**: 検索キャンセル時にカーソルが元の位置に戻る
- **リテラルフォールバック**: 不正な正規表現はリテラル文字列として検索

### 置換 (`:s///`, `:%s///`)
- **正規表現対応**: `:s/fn (\w+)/fn renamed_$1/g` のようなキャプチャグループ対応
- **フラグ**: `g` (行内全置換), `i` (大文字小文字無視)
- **例**: `:%s/foo/bar/gi` — 全行でfoo→barをcase-insensitive全置換

## LSP統合 (rust-analyzer)

サポート機能:
- **補完** (`Ctrl-Space` / 自動): `textDocument/completion`
- **定義ジャンプ** (`gd`): `textDocument/definition`（ファイル跨ぎ対応）
- **ホバー** (`K`): `textDocument/hover`
- **参照検索** (`gr`): `textDocument/references`
- **リネーム** (`:rename`): `textDocument/rename`
- **フォーマット** (`:format`): `textDocument/formatting`
- **診断表示**: `textDocument/publishDiagnostics`（リアルタイム通知）
- **Code Actions** (`ga`): `textDocument/codeAction`（quick fix適用）

### 診断の表示

- エラー行: 行番号が赤色、ガターに `●`、行背景が暗赤
- 警告行: 行番号が黄色、ガターに `▲`、行背景が暗黄
- `]d`/`[d` で診断間をジャンプ（ステータスバーにメッセージ表示）
- `gE` で全診断の一覧ポップアップ表示 → `j/k` で選択、`Enter` でジャンプ

## 新機能の追加パターン

### 新しいコマンドの追加

1. `input/command.rs` の `Command` enum にバリアント追加
2. `input/keymap.rs` にキーバインド追加（対応するモードの関数を編集）
3. `editor/mod.rs` に実装メソッド追加
4. `input/mod.rs` の `execute()` にディスパッチ追加
5. テキスト変更を伴う場合は `track_change()` にも追加

### 新しいLSP機能の追加

1. `lsp/mod.rs` にリクエストメソッド + レスポンスパーサー追加
2. `input/command.rs` にCommand追加
3. イベントループでコマンドフラグをチェック → LSPリクエスト呼び出し
   - TUI: `app.rs` に `request_xxx().await` 追加
   - GUI: `gui_app.rs` に `request_xxx()` メソッド追加（`runtime.block_on()` 使用）
4. `handle_lsp_message()` に `pending_xxx_id` マッチング追加（TUI: `app.rs`、GUI: `gui_app.rs`）
5. 必要に応じてEditorにフィールド追加

### 新しいポップアップUIの追加

1. `editor/mod.rs` に状態フィールド追加 (`showing_xxx`, `xxx_index`, データ)
2. TUI: `ui/xxx.rs` に `Widget` trait実装を新規作成（`ui/references.rs` をテンプレートに）
3. GUI: `gui/xxx.rs` に `draw_xxx()` 関数を新規作成（`gui/references.rs` をテンプレートに）
4. 各 `mod.rs` にモジュール登録 + `render()` で描画呼び出し
5. `keymap.rs` の `map_normal` 冒頭に `showing_xxx` のキーインターセプト追加
6. `editor/mod.rs` の `dismiss_popup()` にクリア処理追加

## 現在の状態と既知の制限

### ビルド状態
- `cargo build`（TUI版）は成功（警告のみ、エラーなし）
- `cargo build --features gui --bin righter-gui`（GUI版）は成功（警告のみ、エラーなし）
- `cargo test` は成功（テスト0件、コンパイルエラーなし）

### 既知の警告
- `editor/mod.rs`: `negative` 変数の未読（数値パース処理内）
- `config.rs`: `tab_width` フィールドの未使用
- いくつかの構造体フィールドの未使用警告（LSP型の一部フィールド）

### テストフィクスチャ
- `test_fixtures/test_diag.rs` — 意図的にコンパイルエラーを含むファイル（LSP診断表示のデモ用）。`examples/` に置くと `cargo test` が失敗するため fixture 化

### 制限事項
- Rust のみ対応（tree-sitter-rust, rust-analyzer固定）
- クリップボードは macOS (`pbcopy`/`pbpaste`) のみ
- LSPのWorkspaceEdit適用は現在のファイルのみ（マルチファイル編集は未対応）
- コマンドモードのコマンドは限定的
- GUI版: シェルコマンド (`:!`) は出力の1行目をステータスメッセージに表示（alternate screenなし）
- GUI版: 定義ジャンプで別ファイルへの遷移はステータスメッセージ表示のみ（同一ファイル内は動作）

### 改善候補
- 他言語対応（tree-sitter-xxx / language server 設定可能化）
- 設定ファイル（`.righterrc` 等）
- LSP workspace/symbol 検索
- マルチカーソル
- GUI版: マウス操作（クリックでカーソル移動、スクロール）
- GUI版: フォント設定（サイズ、ファミリー）

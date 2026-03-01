# Righter

Rust製のVimライクテキストエディタ。同一のエディタコアをTUI（ターミナル）とGUI（デスクトップウィンドウ）の2つのフロントエンドで共有する設計。

rust-analyzerとのLSP統合により、補完・定義ジャンプ・ホバー・参照検索・診断・コードアクション・リネーム・フォーマットをサポート。

## 必要環境

- Rust nightly（edition 2024）
- rust-analyzer（LSP機能を使用する場合、PATHに存在すること）

## ビルドと実行

```bash
# TUI版（デフォルト）
cargo build
cargo run -- <filepath>

# GUI版
cargo build --features gui --bin righter-gui
cargo run --features gui --bin righter-gui -- <filepath>
```

## 機能

### Vim操作

- **モード**: Normal / Insert / Visual / Visual Line / Command / Search
- **モーション**: `h/j/k/l`, `w/b/e/W/B/E`, `0/$`, `^`, `{/}`, `gg/G`, `H/M/L`, `%`, `f/F/t/T`
- **編集**: `d/c/y` + モーション, `dd/cc/yy`, `p/P`, `x`, `J`, `r`, `~`, `gu/gU/g~`, `Ctrl-A/X`
- **Undo/Redo**: `u` / `Ctrl-R`
- **繰り返し**: `.`（ドットリピート）
- **マクロ**: `q{char}` で記録、`@{char}` で再生、`@@` で最後のマクロ再生
- **レジスタ**: `"{char}` でレジスタ選択
- **Visual選択**: `v` / `V`、選択範囲に対して `d/c/y` 等

### 検索・置換

- `/` で正規表現検索（インクリメンタル、スマートケース）
- `n/N` で次/前のマッチ、`*/#` でカーソル下の単語検索
- `:s/old/new/[g][i]` で行内置換、`:%s/old/new/[g][i]` で全体置換（キャプチャグループ対応）

### LSP統合（rust-analyzer）

| キー / コマンド | 機能 |
|----------------|------|
| 自動 / `Ctrl-Space` | 補完 |
| `gd` | 定義ジャンプ |
| `K` | ホバー情報 |
| `gr` | 参照検索 |
| `ga` | コードアクション |
| `gE` | 診断一覧 |
| `]d` / `[d` | 次/前の診断へジャンプ |
| `:rename <name>` | リネーム |
| `:format` | フォーマット |

### ウィンドウ・バッファ

- `Ctrl-W v/s` でペイン分割（垂直/水平）、`:split`/`:vsplit` でファイル指定分割
- `Ctrl-W h/j/k/l` でペイン移動、`Ctrl-W q` でペインを閉じる
- `gt/gT` または `:bn/:bp` でバッファ切替
- `Ctrl-P` でファジーファイルファインダー
- `:set wrap` / `:set nowrap` で行折り返し切替

## アーキテクチャ

```
┌──────────────────────────────────────────────────┐
│              共有エディタコア                       │
│  editor/  input/  lsp/  highlight/  buffer/       │
│  key.rs (KeyInput)                                │
│  highlight/style.rs (SyntaxStyle)                 │
│  editor/pane.rs (AreaRect)                        │
└───────────┬──────────────────┬────────────────────┘
            │                  │
   ┌────────▼────────┐  ┌─────▼──────────┐
   │  TUI フロントエンド │  │ GUI フロントエンド │
   │  ratatui/crossterm│  │  egui/eframe    │
   │  main.rs, app.rs │  │  gui_main.rs    │
   │  ui/             │  │  gui_app.rs     │
   └─────────────────┘  │  gui/           │
                         └────────────────┘
```

単一クレートにfeatureフラグ（`tui` / `gui`）で2つのバイナリを生成。エディタコアはフロントエンド非依存で、3つの共有型（`KeyInput`, `AreaRect`, `SyntaxStyle`）で結合点を抽象化。

## 制限事項

- Rust専用（tree-sitter-rust / rust-analyzer 固定）
- クリップボードは macOS のみ（`pbcopy`/`pbpaste`）
- LSP WorkspaceEdit は現在のファイルのみ対応

## ライセンス

MIT

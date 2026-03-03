# Righter

A Vim-like text editor written in Rust.
Righter uses a shared editor core with two frontends: TUI (terminal) and GUI (desktop window).

With rust-analyzer LSP integration, it supports completion, go-to-definition, hover, references, diagnostics, code actions, rename, and formatting.

## Requirements

- Rust nightly (edition 2024)
- `rust-analyzer` available in `PATH` (for LSP features)

## Build and Run

```bash
# TUI (default)
cargo build
cargo run -- <filepath>

# GUI
cargo build --features gui --bin righter-gui
cargo run --features gui --bin righter-gui -- <filepath>
```

## Features

### Vim Editing

- **Modes**: Normal / Insert / Visual / Visual Line / Command / Search
- **Motions**: `h/j/k/l`, `w/b/e/W/B/E`, `0/$`, `^`, `{/}`, `gg/G`, `H/M/L`, `%`, `f/F/t/T`
- **Editing**: `d/c/y` + motion, `dd/cc/yy`, `p/P`, `x`, `J`, `r`, `~`, `gu/gU/g~`, `Ctrl-A/X`
- **Undo/Redo**: `u` / `Ctrl-R`
- **Repeat**: `.` (dot repeat)
- **Macros**: `q{char}` to record, `@{char}` to play, `@@` to replay last macro
- **Registers**: `"{char}` to select a register
- **Visual Selection**: `v` / `V`, then apply operations like `d/c/y`

### Search and Replace

- `/` for regex search (incremental, smart case)
- `n/N` for next/prev match, `*/#` for word-under-cursor search
- `:s/old/new/[g][i]` for line replacement
- `:%s/old/new/[g][i]` for file-wide replacement (with capture groups)

### LSP (rust-analyzer)

| Key / Command | Action |
|---------------|--------|
| auto / `Ctrl-Space` | Completion |
| `gd` | Go to definition |
| `K` | Hover |
| `gr` | Find references |
| `ga` | Code actions |
| `gE` | Diagnostics list |
| `]d` / `[d` | Next/previous diagnostic |
| `:rename <name>` | Rename symbol |
| `:format` | Format document |

### Windows and Buffers

- `Ctrl-W v/s` to split panes (vertical/horizontal), `:split` / `:vsplit` with optional file
- `Ctrl-W h/j/k/l` to move between panes, `Ctrl-W q` to close a pane
- `gt/gT` or `:bn/:bp` to switch buffers
- `Ctrl-P` for fuzzy file finder
- `:set wrap` / `:set nowrap` to toggle line wrapping
- `:set fontsize=N` to change GUI font size (8–48, default 14)

## Architecture

```text
┌──────────────────────────────────────────────────┐
│                Shared Editor Core                │
│   editor/  input/  lsp/  highlight/  buffer/     │
│   key.rs (KeyInput)                              │
│   highlight/style.rs (SyntaxStyle)               │
│   editor/pane.rs (AreaRect)                      │
└───────────┬──────────────────┬───────────────────┘
            │                  │
   ┌────────▼────────┐  ┌──────▼─────────┐
   │   TUI Frontend   │  │   GUI Frontend  │
   │ ratatui/crossterm│  │   egui/eframe   │
   │ main.rs, app.rs  │  │   gui_main.rs   │
   │ ui/              │  │   gui_app.rs    │
   └──────────────────┘  │   gui/          │
                         └─────────────────┘
```

A single crate builds two binaries behind feature flags (`tui` / `gui`).
The editor core stays frontend-agnostic via three shared types: `KeyInput`, `AreaRect`, `SyntaxStyle`.

## Limitations

- Rust-focused (`tree-sitter-rust` / `rust-analyzer`)
- Clipboard support is currently macOS-only (`pbcopy` / `pbpaste`)
- LSP WorkspaceEdit currently applies only to the active file

## License

MIT

---

## 日本語

Rust製のVimライクテキストエディタ。
同一のエディタコアを TUI（ターミナル）と GUI（デスクトップウィンドウ）の2つのフロントエンドで共有する設計です。

rust-analyzer との LSP 統合により、補完・定義ジャンプ・ホバー・参照検索・診断・コードアクション・リネーム・フォーマットをサポートします。

### 必要環境

- Rust nightly（edition 2024）
- rust-analyzer（LSP機能を使う場合、PATHに存在すること）

### ビルドと実行

```bash
# TUI版（デフォルト）
cargo build
cargo run -- <filepath>

# GUI版
cargo build --features gui --bin righter-gui
cargo run --features gui --bin righter-gui -- <filepath>
```

### 主な機能

- Vim操作（モード/モーション/演算子/マクロ/レジスタ）
- 検索・置換（正規表現、インクリメンタル検索、スマートケース）
- LSP統合（補完、定義ジャンプ、ホバー、参照検索、診断、コードアクション、リネーム、フォーマット）
- ペイン分割・バッファ切替・ファイルファインダー

### 制限事項

- Rust専用（tree-sitter-rust / rust-analyzer 固定）
- クリップボードは macOS のみ（`pbcopy` / `pbpaste`）
- LSP WorkspaceEdit は現在のファイルのみ対応

### ライセンス

MIT

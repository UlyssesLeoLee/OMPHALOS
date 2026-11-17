# 12 WBS 開発計画

| WBS | 作業項目 | 入力 | 成果 | 完了条件 | 状態 |
|---|---|---|---|---|---|
| 1.1 | 要件文書収集 | 既存要件 | source path config | 権威書確定 | 実装済（利用者作業待ち） |
| 1.2 | 旧書式変換 | Office/PDF | 原本↔抽出対応 | ロスなし校閲 | 未実装 |
| 1.3 | 初回批准 | 候補条項 | approved.json | 人間TTY・Owner承認 | 実装済（実行未検証） |
| 2.1 | Rust Parser | .md/.txt | Requirement[] | UT-001〜003 | 実装済・未実行 |
| 2.2 | 比較器 | baseline+prompt | Assessment | UT-004〜006 | 実装済・未実行 |
| 2.3 | Cache/FTS | SQLite | index/cache | 追加UT | 部分実装 |
| 2.4 | Hook | 宿主event | block/context | IT-003/004 | 実装済・未実機 |
| 2.5 | MCP | JSON-RPC | tools | IT-005/006 | 実装済・未実機 |
| 2.6 | Git差分/CI | staged/branch | drift候補 | IT-007/008 | 実装済・未実機 |
| 3.1 | ルールを厳密化 | 歴史要件変更 | recall/precision | 合意済み閾値を満足 | 未実施 |
| 3.2 | AST+実行証拠 | Rust/Java/TS | behavior候補 | 反例テストが通過 | 未実装 |
| 3.3 | 宿主真値テスト | Claude/Codex | Hook実測 | fail-closed確認 | 未実施 |
| 3.4 | Branch保護 | GitHub repo | 強制CI/Owner | 管理者迂回対策 | 利用者設定待ち |
| 4.1 | セキュリティ評価 | threat cases | 結果報告 | 高重大欠陥解消 | 未実施 |
| 4.2 | p95/Tokenベンチ | 実履歴 | 性能証跡 | 目標値を合意 | 未実施 |
| 4.3 | UAT | 真の要件書 | 受入承認 | UAT全件判定 | 未実施 |

**注記：**「実装済」はソースが書かれている意味であり、コンパイル/合格とは異なる。工数とスケジュールは実環境、承認者と対象文書形式が確定してから見積もる。

# 部署与验收操作手册（Win11 / WSL2 / Linux）

## A. 目标环境
需要 Rust stable 工具链、Cargo、Git；插件以本地/私有方式分发。Linux CI 使用 GitHub Actions。本包没有预编译 exe，也不允许将静态校验视为已编译。

## B. 构建
PowerShell：`cargo build --release; cargo test`，确认 `target/release/concept-guard.exe`。若有权限 `cargo install --path .` 安装在 Cargo bin 目录并将该目录加入 PATH。
Linux：`cargo build --release && cargo test && cargo install --path .`。

## C. 目标项目的现有要件
在项目根目录执行 `concept-guard setup`，编辑 `.concept-guard/config.toml`：
```toml
[project]
name = 'target-project'
[requirements]
paths = ['docs/requirements', 'specifications/要件定義.md']
[guard]
top_k = 3
min_score = 2
```
注意 `paths` 必须真实存在，且当前原生支持 UTF-8 .md/.txt。要件中每个实质条款建议标题为 `## [REQ-ORDER-001] 意义明确的条款名`。不能让系统自行批准机器切分的ID。运行 `concept-guard init`，校阅 `.concept-guard/pending.json`，然后 **人类交互终端** `concept-guard approve`。

## D. 插件集成
### Claude Code
1. 在项目中允许插件、将 `plugins/claude` 作为本地插件路径加载（具体命令依据宿主版本）。
2. 确认 hook信任授权和 `concept-guard` PATH。
3. 测试 UserPromptSubmit 与 PreToolUse 的禁止规则和 `onFailure:block`。
4. 测试 `.mcp.json` 的 `concept-guard` server 及四个tools。

### Codex
1. 使用支持本地插件和生命周期Hook的 Codex 桌面环境，注册 `plugins/codex` 私有插件目录。
2. 审查并信任 Hook；未信任时宿主可能不执行。
3. 通过 `commandWindows` 或 PATH 确认Windows的 `concept-guard.exe`。
4. MCP本地stdio需由桌面执行环境发起；不要把包含Hook的包作为公共目录提交。

## E. CI 与权限
复制 `.github/workflows/concept-guard.yml` 到目标仓库；确认仓库主分支 required checks 启用，且 `config.toml`/`approved.json` 随仓库受版本控制。将 `docs/operations/CODEOWNERS.example` 内容移入目标仓库 `.github/CODEOWNERS` 并替换 Owner。要求非本人批准、限制管理员或Agent的绕过方式。**缺少这些仓库侧设置，Hook无法实现不可绕过的正式门禁**。

## F. 检查及讨论
`concept-guard check '新需求'` 用于调试比较器。Hook第一次发现乖离时显示相关要件ID、原文、选项。人工选择方向后必须通过本地 `concept-guard decision A --reason '...'` 记录；B涉及要件修改仍需另建新baseline并通过PR批准。复查 `concept-guard check-baseline`。

## G. 实机红绿测试
红：冲突指令被阻断，未批准要件编辑被阻断、CI拒绝未授权更新。绿：正常完善需求能够继续、合法源码变化无额外LLM调用。独立做出10条红/20条绿样本以识别误报与漏报。所有测试记录 branch/commit/host版本/源码指纹。

## H. Office/PDF要件原本がある場合（任意）
`python scripts/convert_requirements.py original.docx target-requirements.md`。`.xlsx` は worksheet+row、`.docx` は段落番号、`.pdf` は `pdftotext` が抽出した頁を記録し、元ファイル SHA256 を付与する。変換にはレイアウト、テーブル、スキャン画像の喪失リスクがあるため、**要件所有者による原本突合と承認を必須**とする。これは補助ツールであり、本体Rust比較器の要件権威ではない。

# Concept Guard 0.1 — 要件定义唯一锚点治理

**定位：** 每条新用户需求到达 Agent 前，使用本地已批准的要件定义作为唯一受保护锚点，进行低成本关联检索、候选冲突识别与强制 Concept 讨论。基本设计、详细设计、源码、测试不是被锁定锚点；它们只是乖离检测的证据对象。

> 本仓库提供**可审查的 Rust 源码与插件原型**，不是经过正式 IPA 认证的产品。IPA 共通フレーム2013 是过程参考，并非固定的交付物格式。当前制作环境**没有 Rust/Cargo 编译器**，因此未运行 `cargo check/test`；代码编译与目标 Agent Hook 的端到端验证列入部署验收。禁止将静态核验冒充编译通过。

## 1. 快速启动（项目已有要件定义）

安装 Rust stable，然后在本包根目录：

```bash
cargo build --release
cargo test
cargo install --path .
```

在**你的目标项目根目录**运行：

```bash
concept-guard setup
# 修改 .concept-guard/config.toml 中 requirements.paths 指向现有要件文件/目录
concept-guard init
concept-guard approve  # 人类交互终端明确批准，非交互运行会失败
concept-guard status
concept-guard check '中国侧导入时增加 OCR 识别'
concept-guard check-baseline
```

Windows PowerShell：

```powershell
cargo install --path .
concept-guard.exe setup
# 编辑 .concept-guard/config.toml
concept-guard.exe init
concept-guard.exe approve
concept-guard.exe check '中国侧导入时增加 OCR 识别'
```

演示项目在 `examples/demo-project`，无需改变你的真实文件。上述命令带 `--root examples/demo-project` 可在演示目录运行。`init` 不会直接批准要件；`approve` 必须由人类在有 TTY 的本机确认。

## 2. 单入口插件 + Hook + MCP

- **Claude Code**：使用 `plugins/claude/` 插件，附带 `hooks/hooks.json`、`.mcp.json` 和 `skills/concept-guard/SKILL.md`。
- **Codex**：本地/私有安装 `plugins/codex/`（根 `plugin.json` 与 `mcp.json`），附带 `hooks/hooks.json` 与 Skill。含生命周期 Hook 的包不应作为公共 Plugin Directory 提交。
- **所有客户端**：需确保 `concept-guard` 位于 `PATH`、命令 cwd 为目标项目根目录，或将 `CONCEPT_GUARD_PROJECT_ROOT` 指向目标项目。
- **守门边界**：Agent Hook 的信任、失败处理和可用性依赖宿主；受保护分支 + required CI + 人工 CODEOWNERS 审核提供最终合并门禁。**MCP 不会自行安装 Hook**。

## 3. 实际实现与能力边界

| 状态 | 实现 |
|---|---|
| 要件来源 | 现有 `.md/.markdown/.txt`，配置 `requirements.paths`，按 `##` 条款切分；支持 `[REQ-...]` 或自动 ID |
| 正式锚点 | `approved.json`，内容指纹和单条来源位置；仅要件层 |
| 检索 | Rust 本地 CJK 二/三元词与 ASCII Token，含别名配置、SQLite 缓存；可选 FTS5 预备表 |
| 冲突判断 | 当前是显式 `MUST_NOT` + 意图词的启发式候选判定；**不是通用语义证明** |
| 动态代码检测 | Git staged/branch 增量文本候选差异；需要 AST、实际调用路径和测试增强才可证明行为 |
| Hook | UserPromptSubmit、PreToolUse；重大候选阻断或讨论，列出来源条款和选项 |
| MCP | `requirements_get`、`requirements_check`、`concept_discuss`、`concept_decide`，后者只生成提案 |
| 人类批准 | `approve` 与 `decision` 需要交互终端。对于伪造本地身份的恶意程序仍不能提供身份真实性，必须结合服务端保护规则 |
| 其他格式 | 目前不原生解析 DOCX/XLSX/PDF；需转换为可追溯 Markdown/TXT 后纳入要件路径 |
| 模型 Token | 默认检查不调用 LLM；Hook 对无命中返回零上下文，命中最多两条短摘要 |
| 语言 | 索引支持中文/日文/英文混合词元；跨语言语义匹配仍需扩展和实测 |

## 4. 阻断与协商

1. 输入命中禁止性要件：Hook 返回证据摘要和 A/B/C/D 选项并阻断。`讨论 Concept` 可以重新进入会话进行**讨论**，仍禁止修改。
2. 人类在本地终端执行 `concept-guard decision A --reason '...'` 记录讨论结果。A 表示维持要件、C 表示实验隔离；两者解除待讨论状态**但不允许原冲突需求直接执行**。B 经过人类确认后仅允许相关要件源在10分钟内编辑，但仍保留待决状态；D 继续保留待处理锁。
3. 真正修改要件时，先按变更流程编辑文件，运行 `init` 生成候选基线，在受控审批流程确认后运行 `approve`；必须经受保护分支的指定 Owner 审核才能成为正式版本。
4. 任何非受控的修改应由 `check-baseline` 拦截。Git 本地 Hook 可被绕过，不应作为最终控制。

## 5. 文件目录

- `src/`：Rust CLI、解析器、检索比较器、SQLite、Hook、MCP。
- `docs/ipa/`：要件定义、基本/详细设计、UT/IT/ST、验收、非功能、变更管理、保守运营。
- `docs/architecture/`：Mermaid 架构图、ER 图、序列图、状态图源码。
- `plugins/claude/`、`plugins/codex/`：宿主适配包。
- `.github/workflows/`：持续集成模板，复制到目标项目并确保基线文件存在。
- `examples/demo-project/`：不应被视为真实项目的权威要件。
- `artifacts/要件トレーサビリティ.xlsx`：需求到设计/测试的追踪矩阵模板。
- `scripts/verify_package.py`：静态 JSON 与目录校验，**不能替代编译**。

## 6. 最低交付验收

1. 真正接入目标要件并人工确认 baseline；
2. 目标机器 `cargo test`、`cargo build --release` 成功；
3. 两种宿主至少各进行一次真实 Hook 输入检查和阻断；
4. 未批准变更要件必须被 CI 拦截；
5. 10 条故意违背要件的回归样本不能因为缓存或分支差异被跳过；
6. 记录真实 recall/false positive/token/latency 数据后，才能定量声称低成本优势。

## 参考

- IPA 要件定义指南：https://www.ipa.go.jp/archive/publish/qv6pgp0000000wrt-att/000079352.pdf
- IPA 共通フレーム2013 过程：https://www.ipa.go.jp/archive/files/000027415.pdf
- MCP 2025-11-25 stdio：https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2025-11-25/basic/transports.mdx
- Claude Code Hooks：https://code.claude.com/docs/en/hooks
- Claude Code Plugin：https://code.claude.com/docs/en/plugins-reference
- OpenAI Plugins：https://developers.openai.com/plugins/build/plugins

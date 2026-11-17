#!/usr/bin/env python3
"""Generate the machine-readable requirement clause index from the authoritative
requirement definition (docs/ipa/01_要件定義書.md).

Why this exists
---------------
concept-guard parses a requirement set into clauses by `##` headings only, and
detects a prohibition only when a line STARTS with 禁止/不得/不允许/must not/
shall not. The authoritative 01_要件定義書.md states 16 functional requirements
and 9 non-functional requirements as Markdown TABLE ROWS, and states its
prohibitions as the Japanese prose line "対象外：…". Fed to the tool directly it
yields 10 loose AUTO-xxx clauses and ZERO detectable prohibitions, which
docs/operations/INSTALLATION.md §C explicitly forbids ("不能让系统自行批准机器切分的ID").

So this script derives a clause-form index:
  - FR-001..FR-016 / NFR-001..NFR-009 : parsed mechanically from the tables
  - 背景/利害関係者/対象範囲/業務フロー/受入基準/留保 : curated, each item pinned
    to a needle string that must still exist in the source (drift detection)

The index is DERIVED. The authoritative document stays docs/ipa/01_要件定義書.md.
Every derived clause keeps an explicit REQ-* ID and a source line reference.

Usage:
  python scripts/build_requirements_index.py            # write the index
  python scripts/build_requirements_index.py --check    # fail if out of sync (CI)
"""
from pathlib import Path
import argparse
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'docs' / 'ipa' / '01_要件定義書.md'
INDEX = ROOT / 'docs' / 'requirements' / 'REQ-ConceptGuard-0.1.md'
PROVENANCE = ROOT / 'docs' / 'requirements' / 'README.md'

# ---------------------------------------------------------------------------
# Curated clauses. Each entry: (id, title, source_line, items, needles)
#   items  : list of (kind, text) where kind is MUST / MUST_NOT / 普通
#   needles: substrings that must still be present in the source document
# ---------------------------------------------------------------------------
CURATED = [
    ('REQ-BG-001', '背景と目的', 4, [
        ('MUST', '現行承認済みの要件定義だけを固定アンカーとする'),
        ('MUST', '各新規指示に対して自動的に関連要件を検索する'),
        ('MUST', '明示的な不一致は短い証拠で人間と協議する'),
    ], ['現行承認済みの要件定義だけを固定アンカー', '短い証拠で人間と協議する']),

    ('REQ-STK-001', '利害関係者と権限分離', 7, [
        ('MUST', 'プロジェクトオーナー（要件批准者）、人間開発者、複数AI Agent、レビュー担当、CI運用者を明示する'),
        ('MUST', '所有者が別である場合、批准権限とコード承認権限を明示的に分離する'),
    ], ['批准権限とコード承認権限を明示的に分離する']),

    ('REQ-SCOPE-001', '対象範囲と対象外', 11, [
        ('MUST', '対象は既存 .md/.txt 要件、承認スナップショット、条項ID/内容指紋、プロンプト比較、差分候補、Hook、MCP、監査記録、CIである'),
        ('MUST_NOT', '基本設計・詳細設計・コードを独立アンカーとして扱わない'),
        ('MUST_NOT', '厳密な全量意味証明を主張しない'),
        ('MUST_NOT', '未観測の思考を推定しない'),
        ('MUST_NOT', '要件定義書を自動作成しない'),
        ('MUST_NOT', 'PDF/DOCX/XLSX を直接解析しない'),
        ('MUST_NOT', 'Web UI を提供しない'),
        ('MUST_NOT', '汎用RAG を導入しない'),
    ], ['対象外：基本設計/詳細設計/コードの独立アンカー', '汎用RAG']),

    ('REQ-FLOW-001', '運用フロー', 14, [
        ('MUST', '起動前に既存要件ソースを設定し、条項を自動抽出して、利用者が初回批准し、指紋付き baseline を記録する'),
        ('MUST', '運用中はユーザー指示に対して Hook がローカル検索と規則比較を行い、許可または候補乖離の短い対話上官となる'),
        ('MUST', '必要に応じて要件を改訂し、CI承認を得る'),
    ], ['起動前：既存要件ソースを設定', '運用中：ユーザー指示', '必要に応じ要件改訂とCI承認']),

    ('REQ-AC-001', '受入基準', 53, [
        ('MUST', '初期デモの3条項を批准でき、原文改変時のチェックが失敗する'),
        ('MUST', '既知の衝突指示は根拠付きでブロックされる'),
        ('MUST', '禁止操作の解除要求が同じ理由で誤ブロックされない'),
        ('MUST', 'MCPが4ツールを提示する'),
        ('MUST', '非TTYで baseline 批准が失敗する'),
        ('MUST', 'Codex/Claude実機でHookの信頼と失敗設定を確認する'),
        ('MUST', '精度指標は評価セットで実測する'),
        ('MUST_NOT', 'すべての意味的相違を検知できると保証しない'),
    ], ['初期デモの3条項を批准', '非TTYで baseline 批准が失敗する', 'すべての意味的相違を検知できるとの保証はしない']),

    ('REQ-RSV-001', '実装済み範囲の留保', 62, [
        ('MUST', '未実装または要強化の箇所は既知の限界として記録する'),
        ('MUST_NOT', '未実装の機能を完成と混同しない'),
    ], ['MVP実装には未実装/要強化の箇所がある', '完成と混同しない']),
]

KIND_LABEL = {'MUST': 'MUST', 'MUST_NOT': 'MUST_NOT', '普通': '普通'}

TABLE_RE = re.compile(r'^\|\s*(FR|NFR)-(\d+)\s*\|(.+?)\|(.+?)\|(.+?)\|\s*$')


def read_source():
    text = SOURCE.read_text(encoding='utf-8')
    return text, text.splitlines()


def parse_tables(lines):
    """Return [(id, title_text, priority, acceptance, line_no)] for FR/NFR rows."""
    rows = []
    for i, line in enumerate(lines, 1):
        m = TABLE_RE.match(line.strip())
        if not m:
            continue
        kind, num, req, prio, acc = m.groups()
        rows.append((f'{kind}-{num}', req.strip(), prio.strip(), acc.strip(), i))
    return rows


def build_index():
    text, lines = read_source()
    tables = parse_tables(lines)

    out = []
    # No H1 / preamble: the parser turns any content before the first `##` into
    # an AUTO-xxx clause, and docs/operations/INSTALLATION.md §C forbids approving
    # machine-split IDs. The index therefore starts directly with clause 1.

    # ---- background / scope / flow / acceptance (curated) -------------------
    for cid, title, line_no, items, needles in CURATED:
        for needle in needles:
            if needle not in text:
                raise SystemExit(
                    f'漂移検出: {SOURCE.name} に "{needle}" が見つかりません'
                    f'（{cid} / L{line_no}）。原文を先に更新してください。')
        out.append(f'## [{cid}] {title}')
        for kind, item in items:
            out.append(f'- {KIND_LABEL[kind]}: {item}')
        out.append('')
    # ---- FR / NFR tables (mechanical) --------------------------------------
    for cid, req, prio, acc, line_no in tables:
        out.append(f'## [{cid}] {req}')
        out.append(f'- MUST: {req}')
        out.append(f'- 受入条件: {acc}')
        out.append(f'- 優先度: {prio}')
        out.append('')

    return '\n'.join(out).rstrip() + '\n', tables


def build_provenance(tables):
    lines = ['# 要件条項索引 REQ-ConceptGuard-0.1 — 由来と運用', '',
             '対象ファイル: `docs/requirements/REQ-ConceptGuard-0.1.md`（機械可読条項索引・派生）', '',
             '本ディレクトリは **派生索引** です。正本（権威）は `docs/ipa/01_要件定義書.md`。',
             '改正は正本の編集 → `python scripts/build_requirements_index.py` → 差分確認 →',
             'concept-guard による新ベースライン承認、の順で行います。', '',
             '## なぜ索引が必要か', '',
             'concept-guard の条項抽出は `##` 見出し単位でしか分割できません。',
             '正本の機能要件・非機能要求は Markdown の表行であり、禁止事項は',
             '`対象外：` で始まる日本語の散文です。正本をそのまま `requirements.paths` に',
             '指すと、条項は10個の緩い AUTO-xxx に潰れ、検出できる MUST_NOT は0件になります。',
             '`docs/operations/INSTALLATION.md` §C は「システム自行生成の ID を批准しない」ことを',
             '要求しているため、明示 ID を持つ条項索引を別途用意します。', '',
             '## 索引に含まれないもの', '',
             '- `docs/ipa/02`〜`13`（基本設計・詳細設計・試験仕様・運用計画）: '
             '対象範囲の対象外であり、乖離検出の証拠対象であってアンカーではない。',
             '- `docs/ipa/07`（用語定義）: 定義であって規範ではない。', '',
             '## 条項索引 ↔ 正本の対応', '',
             '| 条項ID | 正本の位置 | 種別 |', '|---|---|---|']
    for cid, title, line_no, items, _n in CURATED:
        lines.append(f'| {cid} | `01_要件定義書.md` L{line_no} | 散文（手動変換） |')
    for cid, req, prio, acc, line_no in tables:
        lines.append(f'| {cid} | `01_要件定義書.md` L{line_no} | 表行（機械変換） |')
    lines += ['', '## 再生成とCI', '',
              '```bash',
              'python scripts/build_requirements_index.py          # 生成',
              'python scripts/build_requirements_index.py --check  # 同期していなければ失敗（CI用）',
              '```', '',
              '`--check` は、索引が正本から再生成できる状態か、散文由来の条項の由来文が',
              '正本から消えていないかを検査します。', '']
    return '\n'.join(lines)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--check', action='store_true', help='fail if the index is out of sync')
    args = ap.parse_args()

    index_text, tables = build_index()
    provenance_text = build_provenance(tables)

    if args.check:
        problems = []
        for path, expected in ((INDEX, index_text), (PROVENANCE, provenance_text)):
            if not path.exists():
                problems.append(f'{path} が存在しません')
            elif path.read_text(encoding='utf-8') != expected:
                problems.append(f'{path} が正本と同期していません')
        if problems:
            for p in problems:
                print(f'FAIL: {p}', file=sys.stderr)
            print('python scripts/build_requirements_index.py を実行してください。', file=sys.stderr)
            return 1
        print(f'PASS: {INDEX.relative_to(ROOT)} は正本と同期（{len(tables)} 表条項 + {len(CURATED)} 散文条項）')
        return 0

    INDEX.parent.mkdir(parents=True, exist_ok=True)
    INDEX.write_text(index_text, encoding='utf-8')
    PROVENANCE.write_text(provenance_text, encoding='utf-8')
    print(f'Wrote {INDEX.relative_to(ROOT)} ({len(tables)} table clauses, {len(CURATED)} curated clauses)')
    print(f'Wrote {PROVENANCE.relative_to(ROOT)}')
    return 0


if __name__ == '__main__':
    sys.exit(main())

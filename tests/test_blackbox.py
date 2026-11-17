#!/usr/bin/env python3
"""Linux-only fixture smoke: simulates TTY approval in a disposable demo copy.
This is for tests; it is NOT production identity authentication.
Requires built binary at target/release/concept-guard.
"""
from pathlib import Path
import json
import os
import pty
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / 'target' / 'release' / 'concept-guard'


def run(project, *args, stdin=None):
    return subprocess.run([str(EXE), '--root', str(project), *args], input=stdin, text=True, capture_output=True)


def tty_approve(project):
    master, slave = pty.openpty()
    p = subprocess.Popen([str(EXE), '--root', str(project), 'approve'], stdin=slave, stdout=slave, stderr=slave)
    os.close(slave)
    all_text = b''
    while b'Type APPROVE' not in all_text:
        chunk = os.read(master, 4096)
        if not chunk: raise AssertionError('approval prompt not emitted')
        all_text += chunk
    m = re.search(rb'Type APPROVE ([0-9a-f]{12})', all_text)
    assert m, all_text
    os.write(master, b'APPROVE ' + m.group(1) + b'\n')
    p.wait(timeout=10)
    os.close(master)
    assert p.returncode == 0


def main():
    assert EXE.exists(), 'cargo build --release first'
    with tempfile.TemporaryDirectory() as t:
        p = Path(t)
        shutil.copytree(ROOT / 'examples/demo-project', p / 'demo')
        project = p / 'demo'
        assert run(project, 'init').returncode == 0
        # Any CI automation of *this fixture* uses PTY to test a human-only UI;
        # do not apply it to actual project baseline approvals.
        tty_approve(project)
        assert run(project, 'check-baseline').returncode == 0
        result = run(project, 'check', '中国侧导入时新增 OCR 识别')
        assert result.returncode == 0, result.stderr
        parsed = json.loads(result.stdout)
        assert parsed['action'] == 'block', parsed
        assert parsed['hits'][0]['requirement_id'] == 'REQ-QC-001', parsed
        allowed = run(project, 'check', '中国侧导入时移除 OCR')
        assert json.loads(allowed.stdout)['action'] != 'block'
        with (project / 'docs/requirements/qc.md').open('a', encoding='utf-8') as f:
            f.write('\n## [REQ-QC-099] 测试用新增条款\n- MUST: 必须保留审计历史\n')
        assert run(project, 'check-baseline').returncode != 0
        print('PASS fixture smoke: approval, conflict, nonconflict, baseline tamper gate')


if __name__ == '__main__': main()

"""Static packaging validation that does NOT replace cargo check/test."""
import json
from pathlib import Path
root = Path(__file__).resolve().parents[1]
files = sorted(root.rglob('*.json'))
for file in files:
    json.loads(file.read_text(encoding='utf-8'))
for name in ['src/main.rs','src/model.rs','src/engine.rs','src/parse.rs','src/store.rs','src/hooks.rs','src/mcp.rs',
             'plugins/claude/hooks/hooks.json','plugins/codex/hooks/hooks.json',
             'plugins/claude/.mcp.json','plugins/codex/mcp.json','Cargo.toml']:
    assert (root / name).exists(), name
print(f'PASS static package structure: {len(files)} JSON files parsed. Rust compilation NOT checked.')

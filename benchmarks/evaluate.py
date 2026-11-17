#!/usr/bin/env python3
"""Regression evaluator; run only AFTER building a Rust binary and human-approved demo baseline.
Reports lexical recall, exact outcome accuracy, and model-free runtime.
Does not assert the implementation is semantically complete.
"""
import csv
import json
import subprocess
import sys
import time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
EXE=ROOT/'target'/'release'/('concept-guard.exe' if sys.platform=='win32' else 'concept-guard')
PROJECT=ROOT/'examples'/'demo-project'
if not EXE.exists():
    raise SystemExit('Build first: cargo build --release')
if not (PROJECT/'.concept-guard/approved.json').exists():
    raise SystemExit('Demo needs HUMAN TTY approval: concept-guard --root examples/demo-project init / approve')
rows=list(csv.DictReader((ROOT/'benchmarks/evaluation.csv').open(encoding='utf-8')))
found=0; correct=0; durations=[]
for row in rows:
    start=time.perf_counter()
    p=subprocess.run([str(EXE),'--root',str(PROJECT),'check',row['prompt']],capture_output=True,text=True,check=True)
    durations.append((time.perf_counter()-start)*1000)
    result=json.loads(p.stdout)
    req=row['expected_requirement']; ids=[hit['requirement_id'] for hit in result['hits']]
    hit=(req in ids) if req else not ids
    found+=hit
    correct+=result['outcome']==row['expected_outcome']
    print(('PASS' if hit else 'MISS'),row['prompt'],'->',result['outcome'],'hits',ids)
print(f'top-k relatedness accuracy={found}/{len(rows)} exact outcome accuracy={correct}/{len(rows)}; '
      f'mean wall ms={sum(durations)/len(durations):.2f}; p95 wall ms={sorted(durations)[int((len(durations)-1)*.95)]:.2f}')

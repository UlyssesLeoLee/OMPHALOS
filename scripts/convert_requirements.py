#!/usr/bin/env python3
"""Optional, REVIEW-REQUIRED converter of existing DOCX/XLSX/PDF requirements to Markdown.

Preserves origin path, SHA256 and source paragraph/sheet-row/page in generated text.
Never auto-approves converted requirements. It may lose tables, formatting, formulas,
merged cells and scan images; owners MUST compare against original before baseline approval.
Requires only Python stdlib; PDF additionally requires `pdftotext` executable.

Usage: python scripts/convert_requirements.py INPUT.docx OUT.md
"""
from pathlib import Path
from hashlib import sha256
from xml.etree import ElementTree as ET
from zipfile import ZipFile
import argparse
import re
import shutil
import subprocess

W = '{http://schemas.openxmlformats.org/wordprocessingml/2006/main}'
S = '{http://schemas.openxmlformats.org/spreadsheetml/2006/main}'
R = '{http://schemas.openxmlformats.org/officeDocument/2006/relationships}'
P = '{http://schemas.openxmlformats.org/package/2006/relationships}'


def word_paragraphs(path):
    with ZipFile(path) as z:
        xml = ET.fromstring(z.read('word/document.xml'))
    for i, paragraph in enumerate(xml.iter(W + 'p'), 1):
        words = [t.text or '' for t in paragraph.iter(W + 't')]
        text = ''.join(words).strip()
        if text:
            yield f'Word paragraph {i}', text


def xlsx_rows(path):
    with ZipFile(path) as z:
        shared = []
        if 'xl/sharedStrings.xml' in z.namelist():
            root = ET.fromstring(z.read('xl/sharedStrings.xml'))
            for item in root.findall(S + 'si'):
                shared.append(''.join((n.text or '') for n in item.iter(S + 't')))
        book = ET.fromstring(z.read('xl/workbook.xml'))
        rels = ET.fromstring(z.read('xl/_rels/workbook.xml.rels'))
        links = {r.attrib['Id']: r.attrib['Target'].lstrip('/') for r in rels.findall(P + 'Relationship')}
        for sheet in book.findall(f'.//{S}sheets/{S}sheet'):
            name = sheet.attrib.get('name','Unknown')
            rid = sheet.attrib.get(R + 'id')
            target = links.get(rid, '')
            if not target:
                continue
            if target.startswith('xl/'):
                fname = target
            else:
                fname = 'xl/' + target
            xml = ET.fromstring(z.read(fname))
            for row in xml.findall(f'.//{S}sheetData/{S}row'):
                cells = []
                for cell in row.findall(S + 'c'):
                    val = cell.find(S + 'v')
                    if cell.attrib.get('t') == 'inlineStr':
                        texts = [node.text or '' for node in cell.iter(S + 't')]
                        value = ''.join(texts)
                    elif val is not None:
                        value = val.text or ''
                        if cell.attrib.get('t') == 's':
                            index = int(value)
                            value = shared[index] if index < len(shared) else ''
                    else:
                        value = ''
                    if value.strip():
                        cells.append(f"{cell.attrib.get('r','?')}: {value.strip()}")
                if cells:
                    yield f'Excel {name} row {row.attrib.get("r","?")}', ' | '.join(cells)


def pdf_pages(path):
    if not shutil.which('pdftotext'):
        raise RuntimeError('pdftotext not found; convert PDF externally with traceable original pages')
    result = subprocess.run(['pdftotext','-layout',str(path),'-'], capture_output=True, text=True, check=True)
    for i, text in enumerate(result.stdout.split('\f'), 1):
        normalized = re.sub(r'\n\s*\n+', '\n', text).strip()
        if normalized:
            yield f'PDF page {i}', normalized


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    src = args.input.resolve()
    dst = args.output.resolve()
    if src == dst: raise SystemExit('Input and output must differ')
    data = src.read_bytes()
    ext = src.suffix.lower()
    if ext == '.docx': rows = word_paragraphs(src)
    elif ext == '.xlsx': rows = xlsx_rows(src)
    elif ext == '.pdf': rows = pdf_pages(src)
    else: raise SystemExit('Only DOCX/XLSX/PDF supported')
    digest = sha256(data).hexdigest()
    chunks = [f'# Converted requirements — REVIEW REQUIRED\n',
              f'- Original source: `{src}`\n', f'- Original SHA256: `{digest}`\n',
              '- Fidelity warning: compare every item with the source; conversion is not approval.\n']
    for key, content in rows:
        chunks += [f'\n## {key}\n', f'<!-- original: {src.name}; location: {key} -->\n', content, '\n']
    dst.parent.mkdir(parents=True,exist_ok=True)
    dst.write_text('\n'.join(chunks), encoding='utf-8')
    print(f'Converted to {dst}. REVIEW WITH ORIGINAL before inclusion in approved baseline.')

if __name__ == '__main__': main()

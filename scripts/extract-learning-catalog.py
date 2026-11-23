"""Read-only, development-time extraction; the shipped app uses the embedded snapshot."""
import collections
import hashlib
import json
from pathlib import Path
import sys
import openpyxl

root = Path(__file__).resolve().parents[1]
source = Path(sys.argv[1]) if len(sys.argv) > 1 else root / 'SciDesk_学习网址完整分类.xlsx'
before = hashlib.sha256(source.read_bytes()).hexdigest()
book = openpyxl.load_workbook(source, read_only=True, data_only=True)
sheets = {}
for sheet in book:
    sheets[sheet.title] = list(sheet.values)

def records(name):
    rows = sheets[name]
    headers = rows[0]
    if len(set(headers)) != len(headers):
        raise ValueError(f'Duplicate column names: {name}')
    return [dict(zip(headers, row)) for row in rows[1:] if any(v is not None for v in row)]

all_rows = records('全部609条分类')
selected = records('建议导入与可选')
excluded = records('建议排除清单')
by_id = {r['序号']: r for r in all_rows}
assert len(by_id) == len(all_rows), 'Source sequence is not unique'
assert all(by_id[r['序号']] == r for r in selected + excluded), 'Sheet disagreement'
assert {r['序号'] for r in selected + excluded} == set(by_id)
assert all(r['是否建议导入学习'] in ('是', '可选') for r in selected)
assert all(r['是否建议导入学习'] == '否' for r in excluded)
overview = sheets['分类总览']
header_row = next(i for i, r in enumerate(overview) if r[0] == '一级分类')
overview_rows = [dict(zip(overview[header_row], r)) for r in overview[header_row+1:] if r[0]]
names = {r['一级分类'] for r in selected}
categories = [r['一级分类'] for r in overview_rows if r['一级分类'] in names]
assert set(categories) == names
fields = {'title':'书签标题','url':'原始网址','category1':'一级分类','category2':'二级分类','property':'资源属性初判','folder':'原始文件夹路径','duplicate':'重复链接标记','classification_note':'分类说明','domain':'网址域名'}
result = {
    'source': source.name, 'sha256': before,
    'sheets': [{'name': n, 'rows': len(rows), 'columns': max(map(len, rows), default=0)} for n, rows in sheets.items()],
    'categories': [{'name': n, 'children': list(dict.fromkeys(r['二级分类'] or '' for r in selected if r['一级分类'] == n))} for n in categories],
    'counts': dict(collections.Counter(r['是否建议导入学习'] for r in all_rows)),
    'records': [{'source_id': f'bookmarks-classified-v1:{r["序号"]}', 'sequence': r['序号'], 'decision': r['是否建议导入学习'], **{k: r[v] or '' for k,v in fields.items()}, 'raw': r} for r in selected + excluded],
}
assert hashlib.sha256(source.read_bytes()).hexdigest() == before
target = root / 'src-tauri' / 'learning_catalog.json'
target.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
print(json.dumps({k: result[k] for k in ('source','sha256','sheets','counts')}, ensure_ascii=False))

"""Read-only online SQLite backups for a private installer; never copy live WALs/caches."""
import argparse
import hashlib
import json
import pathlib
import shutil
import sqlite3
import os
from datetime import datetime, timezone

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE = ROOT / 'runtime' / 'desktop'

def select_source(explicit=None):
    if explicit:
        candidates = [explicit.resolve()]
    else:
        candidates = [SOURCE]
        if os.environ.get('LOCALAPPDATA'):
            candidates.append(pathlib.Path(os.environ['LOCALAPPDATA']) / 'com.scidesk.v2.desktop')
        if os.name == 'nt':
            import winreg
            try:
                with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r'Software\Microsoft\Windows\CurrentVersion\Uninstall\SciDesk 2.0') as key:
                    install = pathlib.Path(winreg.QueryValueEx(key, 'InstallLocation')[0].strip('"'))
                    if (install / 'scidesk.exe').is_file():
                        candidates.append(install / 'SciDeskData')
            except FileNotFoundError:
                pass
    valid = list(dict.fromkeys(p.resolve() for p in candidates if (p / 'database/scidesk.db').is_file()))
    if len(valid) != 1:
        raise RuntimeError(f'无法唯一确认当前第二版数据来源，请传入 --source；有效来源：{valid}')
    if valid[0] == pathlib.Path(r'E:\SciDeskData') or valid[0].is_relative_to(pathlib.Path(r'E:\SciDesk')):
        raise RuntimeError('禁止将第一版数据纳入第二版安装包')
    return valid[0]


def readonly(path):
    return sqlite3.connect(path.resolve().as_uri() + '?mode=ro', uri=True)


def snapshot(destination):
    destination = destination.resolve()
    destination.relative_to(ROOT / 'development')
    destination.mkdir(parents=True, exist_ok=False)
    pending = SOURCE / 'database' / 'operations'
    if pending.exists() and list(pending.glob('*.json')):
        raise RuntimeError('存在尚未恢复的项目操作，请在软件内处理后再打包')
    databases = ['database/scidesk.db', 'database/learning/learning.db',
                 'database/manuals/manuals.db', 'database/tools/tools.db',
                 'ideas/ideas.db', 'journals/journals.db']
    groups, counts = [], {}
    for rel in databases:
        source = SOURCE / rel
        if not source.exists():
            raise RuntimeError(f'当前桌面数据缺失：{rel}')
        target = destination / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        with readonly(source) as src, sqlite3.connect(target) as dst:
            src.backup(dst)
            assert dst.execute('PRAGMA quick_check').fetchone() == ('ok',), rel
            assert not dst.execute('PRAGMA foreign_key_check').fetchall(), rel
            # Pairing credentials/device identity are machine-specific, not user content.
            if rel == 'ideas/ideas.db':
                dst.execute('DELETE FROM idea_devices')
                dst.execute("DELETE FROM idea_settings WHERE key='identity'")
                dst.commit()
            dst.execute('PRAGMA journal_mode=DELETE')
            tables = [r[0] for r in dst.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")]
            counts[rel] = {t: dst.execute('SELECT COUNT(*) FROM "' + t + '"').fetchone()[0] for t in tables}
        group_target = rel if rel == 'database/scidesk.db' else str(pathlib.PurePosixPath(rel).parent)
        groups.append({'target': group_target, 'directory': rel != 'database/scidesk.db', 'files': []})

    def owned(rel):
        source, target = SOURCE / rel, destination / rel
        # Copy only explicitly referenced application-owned files, never arbitrary folders.
        source.resolve().relative_to(SOURCE.resolve())
        if source.is_symlink() or not source.is_file():
            raise RuntimeError(f'关联的应用内文件不可用：{rel}')
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)

    with readonly(destination / 'journals/journals.db') as db:
        avatar = db.execute('SELECT avatar_file FROM scholar_profile WHERE id=1').fetchone()[0]
        if avatar:
            if pathlib.Path(avatar).name != avatar or not avatar.startswith('avatar-'):
                raise RuntimeError('头像路径不合法')
            owned('journals/' + avatar)
    with readonly(destination / 'database/tools/tools.db') as db:
        for icon, in db.execute('SELECT DISTINCT icon_file FROM tools WHERE icon_file IS NOT NULL'):
            if pathlib.Path(icon).name != icon:
                raise RuntimeError('工具图标路径不合法')
            owned('database/tools/icons/' + icon)
    with readonly(destination / 'ideas/ideas.db') as db:
        for aid, in db.execute('SELECT id FROM idea_attachments'):
            if not aid.isalnum() and not all(c.isalnum() or c in '-_' for c in aid):
                raise RuntimeError('想法附件标识不合法')
            owned('ideas/attachments/' + aid + '.bin')
    capture = SOURCE / 'tools/screenshot-settings.json'
    if capture.exists():
        owned('tools/screenshot-settings.json')
        groups.append({'target': 'tools', 'directory': True, 'files': []})
    for group in groups:
        target = destination / group['target']
        files = sorted(target.rglob('*')) if group['directory'] else [target]
        for path in files:
            if path.is_file():
                data = path.read_bytes()
                group['files'].append({'path': path.relative_to(destination).as_posix(),
                                       'size': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
    manifest = {'version': 1, 'created_at': datetime.now(timezone.utc).isoformat(),
                'source': 'SciDesk 2.0 personal snapshot', 'source_directory': str(SOURCE), 'groups': groups,
                'counts': counts, 'external_files_included': False}
    (destination / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'snapshot': str(destination), 'counts': counts}, ensure_ascii=False))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('destination', type=pathlib.Path)
    parser.add_argument('--source', type=pathlib.Path)
    args = parser.parse_args()
    SOURCE = select_source(args.source)
    snapshot(args.destination)

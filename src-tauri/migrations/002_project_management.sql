ALTER TABLE projects ADD COLUMN icon_name TEXT NOT NULL DEFAULT 'folder';
ALTER TABLE projects ADD COLUMN progress_enabled INTEGER NOT NULL DEFAULT 0 CHECK(progress_enabled IN (0,1));
ALTER TABLE projects ADD COLUMN source_type TEXT NOT NULL DEFAULT 'created' CHECK(source_type IN ('created','imported'));
ALTER TABLE projects ADD COLUMN root_key TEXT;
CREATE UNIQUE INDEX projects_live_root_key ON projects(root_key) WHERE deleted_at IS NULL AND root_key IS NOT NULL;
INSERT INTO settings(key,value) VALUES('projects.operation_lock','0') ON CONFLICT(key) DO NOTHING;

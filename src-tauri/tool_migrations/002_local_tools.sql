ALTER TABLE tools ADD COLUMN kind TEXT NOT NULL DEFAULT 'website' CHECK(kind IN ('website','local'));
ALTER TABLE tools ADD COLUMN local_json TEXT;
ALTER TABLE tools ADD COLUMN manifest_id TEXT;
CREATE UNIQUE INDEX tools_local_identity ON tools(manifest_id) WHERE kind='local' AND removed=0;
PRAGMA user_version=2;

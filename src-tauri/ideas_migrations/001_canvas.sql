CREATE TABLE idea_records(id TEXT PRIMARY KEY, data TEXT NOT NULL CHECK(json_valid(data)), version INTEGER NOT NULL, field_versions TEXT NOT NULL CHECK(json_valid(field_versions)));
CREATE TABLE idea_changes(seq INTEGER PRIMARY KEY AUTOINCREMENT, idea_id TEXT NOT NULL, version INTEGER NOT NULL);
CREATE TABLE idea_operations(id TEXT PRIMARY KEY, request_hash TEXT NOT NULL, result TEXT NOT NULL CHECK(json_valid(result)));
CREATE TABLE idea_attachments(id TEXT PRIMARY KEY, idea_id TEXT NOT NULL REFERENCES idea_records(id), name TEXT NOT NULL, media_type TEXT NOT NULL, size INTEGER NOT NULL, sha256 TEXT NOT NULL, created_at INTEGER NOT NULL, deleted_at INTEGER);
CREATE TABLE idea_conflicts(id TEXT PRIMARY KEY, idea_id TEXT NOT NULL, attempted TEXT NOT NULL, current TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE idea_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL CHECK(json_valid(value)));
CREATE TABLE idea_devices(id TEXT PRIMARY KEY, name TEXT NOT NULL, token_hash TEXT NOT NULL UNIQUE, created_at INTEGER NOT NULL, last_seen INTEGER NOT NULL, revoked_at INTEGER, disconnected_at INTEGER);
INSERT INTO idea_settings(key,value) VALUES('canvas','{"x":0,"y":0,"zoom":1,"version":0}');
PRAGMA user_version=1;

CREATE TABLE categories(id TEXT PRIMARY KEY,parent_id TEXT NOT NULL DEFAULT '',name TEXT NOT NULL,sort_order INTEGER NOT NULL,UNIQUE(parent_id,name));
CREATE TABLE resources(id TEXT PRIMARY KEY,title TEXT NOT NULL,url TEXT NOT NULL,category1 TEXT NOT NULL,category2 TEXT NOT NULL DEFAULT '',property TEXT NOT NULL DEFAULT '',tier TEXT NOT NULL CHECK(tier IN ('core','optional')),origin TEXT NOT NULL CHECK(origin IN ('excel','manual')),source_id TEXT UNIQUE,folder TEXT NOT NULL DEFAULT '',classification_note TEXT NOT NULL DEFAULT '',duplicate TEXT NOT NULL DEFAULT '',notes TEXT NOT NULL DEFAULT '',tags TEXT NOT NULL DEFAULT '',created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,removed INTEGER NOT NULL DEFAULT 0);
CREATE TABLE source_records(source_id TEXT PRIMARY KEY,decision TEXT NOT NULL,raw_json TEXT NOT NULL,issue TEXT NOT NULL DEFAULT '');
CREATE TABLE import_batches(sha256 TEXT PRIMARY KEY,source TEXT NOT NULL,report_json TEXT NOT NULL,imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
CREATE TABLE backups(id TEXT PRIMARY KEY,resource_id TEXT NOT NULL REFERENCES resources(id) ON DELETE CASCADE,path TEXT NOT NULL,path_key TEXT NOT NULL,UNIQUE(resource_id,path_key));
CREATE INDEX resources_category ON resources(category1,category2,removed,tier);
PRAGMA user_version=1;

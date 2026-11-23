-- Extend the existing independent Ideas store without recreating any records.
ALTER TABLE idea_records ADD COLUMN kind TEXT NOT NULL DEFAULT 'idea' CHECK(kind IN ('idea','room','edge'));
UPDATE idea_records SET data=json_set(data,'$._entity','idea','$.room_id',NULL);
CREATE INDEX idea_records_kind ON idea_records(kind);
CREATE TABLE idea_canvases(scope_id TEXT PRIMARY KEY, data TEXT NOT NULL CHECK(json_valid(data)));
INSERT INTO idea_canvases(scope_id,data) SELECT 'hall',value FROM idea_settings WHERE key='canvas';
PRAGMA user_version=2;

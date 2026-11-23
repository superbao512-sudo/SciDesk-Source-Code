ALTER TABLE journals ADD COLUMN series TEXT NOT NULL DEFAULT '自定义';
ALTER TABLE scholar_profile ADD COLUMN avatar_file TEXT;
CREATE TABLE journal_library_state(id INTEGER PRIMARY KEY CHECK(id=1),initialized INTEGER NOT NULL DEFAULT 0);
INSERT INTO journal_library_state(id) VALUES(1);

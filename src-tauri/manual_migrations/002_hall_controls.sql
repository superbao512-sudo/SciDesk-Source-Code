ALTER TABLE manuals ADD COLUMN icon_name TEXT NOT NULL DEFAULT 'book-open';
ALTER TABLE manuals ADD COLUMN manual_color TEXT NOT NULL DEFAULT '#4779bd';
ALTER TABLE manuals ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
UPDATE manuals SET sort_order=rowid-1;
CREATE TABLE preferences(id INTEGER PRIMARY KEY CHECK(id=1),name_font_size INTEGER NOT NULL CHECK(name_font_size BETWEEN 14 AND 28));
INSERT INTO preferences(id,name_font_size) VALUES(1,18);
PRAGMA user_version=2;

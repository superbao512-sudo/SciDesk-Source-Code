-- Preserve all existing papers, subscriptions and reading history.
CREATE TABLE scholar_profile (
 id INTEGER PRIMARY KEY CHECK(id=1),
 snapshot_json TEXT CHECK(snapshot_json IS NULL OR json_valid(snapshot_json)),
 last_attempt_at TEXT,
 last_success_at TEXT,
 last_error TEXT
);
INSERT INTO scholar_profile(id) VALUES(1);
CREATE TABLE journal_feed_batch (
 paper_id TEXT PRIMARY KEY REFERENCES papers(id) ON DELETE CASCADE,
 push_item_id TEXT NOT NULL UNIQUE REFERENCES push_items(id) ON DELETE CASCADE,
 delivered_at TEXT NOT NULL
);
CREATE TABLE journal_feed_batch_state (
 id INTEGER PRIMARY KEY CHECK(id=1), updated_at TEXT, count INTEGER NOT NULL DEFAULT 0
);
INSERT INTO journal_feed_batch_state(id) VALUES(1);
CREATE INDEX journal_papers_published_idx ON papers(published_at DESC,id);

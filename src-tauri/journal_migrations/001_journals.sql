-- Independent v2 journal metadata only. Derived from v1 schemas 026-029; no user/demo seeds.
CREATE TABLE papers (
 id TEXT PRIMARY KEY, title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 500),
 authors TEXT NOT NULL DEFAULT '', journal TEXT NOT NULL DEFAULT '',
 year INTEGER CHECK(year IS NULL OR year BETWEEN 1000 AND 9999),
 doi TEXT NOT NULL DEFAULT '', abstract TEXT NOT NULL DEFAULT '',
 pdf_path TEXT, zotero_key TEXT,
 favorite INTEGER NOT NULL DEFAULT 0 CHECK(favorite IN (0,1)),
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
ALTER TABLE papers ADD COLUMN source_type TEXT NOT NULL DEFAULT 'remote';
ALTER TABLE papers ADD COLUMN display_name TEXT;
ALTER TABLE papers ADD COLUMN document_title TEXT;
-- Phase 1 paper push foundation. Existing papers remain the canonical local
-- literature object; these additions only add provider metadata and matching.
ALTER TABLE papers ADD COLUMN canonical_key TEXT;
ALTER TABLE papers ADD COLUMN abstract_text TEXT;
ALTER TABLE papers ADD COLUMN arxiv_id TEXT;
ALTER TABLE papers ADD COLUMN arxiv_version INTEGER;
ALTER TABLE papers ADD COLUMN journal_id TEXT;
ALTER TABLE papers ADD COLUMN publisher TEXT;
ALTER TABLE papers ADD COLUMN published_at TEXT;
ALTER TABLE papers ADD COLUMN source_updated_at TEXT;
ALTER TABLE papers ADD COLUMN article_url TEXT;
ALTER TABLE papers ADD COLUMN pdf_url TEXT;
ALTER TABLE papers ADD COLUMN primary_source TEXT;
ALTER TABLE papers ADD COLUMN abstract_status TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE papers ADD COLUMN fingerprint TEXT;
ALTER TABLE papers ADD COLUMN first_seen_at TEXT;
ALTER TABLE papers ADD COLUMN last_seen_at TEXT;
ALTER TABLE papers ADD COLUMN metadata_json TEXT NOT NULL DEFAULT '{}';

CREATE TABLE journals (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  short_name TEXT NOT NULL,
  issn_print TEXT,
  issn_online TEXT,
  publisher TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE(name), UNIQUE(short_name)
);

CREATE TABLE paper_authors (
  paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
  author_index INTEGER NOT NULL,
  given_name TEXT,
  family_name TEXT,
  display_name TEXT NOT NULL,
  orcid TEXT,
  PRIMARY KEY (paper_id, author_index)
);

CREATE TABLE paper_source_records (
  id TEXT PRIMARY KEY,
  paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
  source TEXT NOT NULL CHECK(source IN ('arxiv','crossref')),
  source_key TEXT NOT NULL,
  source_version INTEGER NOT NULL DEFAULT 0,
  raw_json TEXT,
  fetched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE(source, source_key, source_version)
);

CREATE TABLE sync_state (
  source TEXT PRIMARY KEY CHECK(source IN ('arxiv','crossref')),
  last_success_at TEXT,
  last_attempt_at TEXT,
  last_error_json TEXT,
  cursor TEXT,
  run_count INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE topics (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
  required_terms_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(required_terms_json)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE topic_keywords (
  topic_id TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  phrase TEXT NOT NULL,
  weight REAL NOT NULL CHECK(weight > 0),
  field TEXT NOT NULL DEFAULT 'both' CHECK(field IN ('title','abstract','both')),
  PRIMARY KEY (topic_id, phrase)
);

CREATE TABLE paper_matches (
  paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
  topic_id TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  score REAL NOT NULL,
  tier TEXT NOT NULL CHECK(tier IN ('high','medium','none')),
  reason_json TEXT NOT NULL CHECK(json_valid(reason_json)),
  matched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY (paper_id, topic_id)
);

CREATE TABLE push_items (
  id TEXT PRIMARY KEY,
  paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
  topic_id TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  match_score REAL NOT NULL,
  status TEXT NOT NULL DEFAULT 'unread' CHECK(status IN ('unread','read','dismissed')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE(paper_id, topic_id)
);

CREATE UNIQUE INDEX papers_doi_uq ON papers(lower(trim(doi))) WHERE trim(doi) <> '';
CREATE UNIQUE INDEX papers_arxiv_uq ON papers(lower(trim(arxiv_id))) WHERE trim(coalesce(arxiv_id,'')) <> '';
CREATE UNIQUE INDEX papers_fingerprint_uq ON papers(fingerprint) WHERE trim(coalesce(fingerprint,'')) <> '';
CREATE INDEX paper_source_records_paper_idx ON paper_source_records(paper_id, source);
CREATE INDEX paper_matches_topic_idx ON paper_matches(topic_id, score DESC, matched_at DESC);
CREATE INDEX push_items_feed_idx ON push_items(status, created_at DESC);

-- Phase 1.3: versioned matching metadata and feed query indexes.
ALTER TABLE paper_matches ADD COLUMN algorithm_version TEXT NOT NULL DEFAULT 'keyword-v1';

CREATE INDEX paper_matches_feed_idx
  ON paper_matches(topic_id, tier, score DESC, paper_id);
CREATE INDEX push_items_created_feed_idx
  ON push_items(created_at DESC, id DESC);
CREATE INDEX paper_source_records_source_paper_idx
  ON paper_source_records(source, paper_id);
-- SCI subscription management: user-configurable topics, sources, journals,
-- synchronization history and in-app notification preferences. Existing paper
-- identities and source records remain canonical; this migration only adds
-- configuration and observability metadata.

ALTER TABLE topics ADD COLUMN description TEXT NOT NULL DEFAULT '';
ALTER TABLE topics ADD COLUMN match_logic TEXT NOT NULL DEFAULT 'any' CHECK(match_logic IN ('all','any'));
ALTER TABLE topics ADD COLUMN match_algorithm_version TEXT NOT NULL DEFAULT 'keyword-v2';

ALTER TABLE topic_keywords ADD COLUMN kind TEXT NOT NULL DEFAULT 'boost' CHECK(kind IN ('required','boost','exclude'));
ALTER TABLE topic_keywords ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1));
ALTER TABLE topic_keywords ADD COLUMN notes TEXT NOT NULL DEFAULT '';

ALTER TABLE journals ADD COLUMN website_url TEXT;
ALTER TABLE journals ADD COLUMN icon_url TEXT;
ALTER TABLE journals ADD COLUMN metadata_status TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE journals ADD COLUMN metadata_json TEXT NOT NULL DEFAULT '{}';
-- SQLite rejects expression defaults on ALTER TABLE ADD COLUMN. Existing rows
-- are backfilled below; new rows use the same safe non-null sentinel until
-- their first metadata update.
ALTER TABLE journals ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
UPDATE journals SET updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE updated_at='';

CREATE TABLE source_registry (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK(kind IN ('journal','preprint')),
  display_name TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
  config_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(config_json)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE topic_source_selections (
  topic_id TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  source_id TEXT NOT NULL REFERENCES source_registry(id) ON DELETE CASCADE,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY(topic_id,source_id)
);

CREATE TABLE journal_subscriptions (
  id TEXT PRIMARY KEY,
  journal_id TEXT NOT NULL UNIQUE REFERENCES journals(id) ON DELETE CASCADE,
  display_name TEXT,
  group_name TEXT NOT NULL DEFAULT '重点关注',
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
  sync_enabled INTEGER NOT NULL DEFAULT 1 CHECK(sync_enabled IN (0,1)),
  last_success_at TEXT,
  last_attempt_at TEXT,
  last_status TEXT NOT NULL DEFAULT 'never',
  last_error TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE topic_journal_subscriptions (
  topic_id TEXT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  subscription_id TEXT NOT NULL REFERENCES journal_subscriptions(id) ON DELETE CASCADE,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY(topic_id,subscription_id)
);

CREATE TABLE sync_runs (
  id TEXT PRIMARY KEY,
  source TEXT NOT NULL REFERENCES source_registry(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK(status IN ('running','success','partial_success','not_configured','network_error','http_error','parse_error','rate_limited','timeout','cancelled','failed')),
  query_count INTEGER NOT NULL DEFAULT 0,
  fetched_count INTEGER NOT NULL DEFAULT 0,
  parsed_count INTEGER NOT NULL DEFAULT 0,
  inserted_count INTEGER NOT NULL DEFAULT 0,
  updated_count INTEGER NOT NULL DEFAULT 0,
  matched_count INTEGER NOT NULL DEFAULT 0,
  pushed_count INTEGER NOT NULL DEFAULT 0,
  error_type TEXT,
  error_message TEXT,
  cursor TEXT,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

ALTER TABLE sync_state ADD COLUMN status TEXT NOT NULL DEFAULT 'never';
ALTER TABLE sync_state ADD COLUMN last_error_type TEXT;
ALTER TABLE sync_state ADD COLUMN last_error_message TEXT;
ALTER TABLE sync_state ADD COLUMN last_query_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sync_state ADD COLUMN last_fetched_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sync_state ADD COLUMN last_parsed_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sync_state ADD COLUMN last_inserted_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sync_state ADD COLUMN last_updated_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sync_state ADD COLUMN last_matched_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sync_state ADD COLUMN last_pushed_count INTEGER NOT NULL DEFAULT 0;

CREATE INDEX topic_keywords_enabled_idx ON topic_keywords(topic_id,enabled,kind);
CREATE INDEX topic_source_selections_source_idx ON topic_source_selections(source_id,enabled,topic_id);
CREATE INDEX topic_journal_subscriptions_topic_idx ON topic_journal_subscriptions(topic_id,enabled);
CREATE INDEX journal_subscriptions_sync_idx ON journal_subscriptions(sync_enabled,enabled,journal_id);

CREATE INDEX sync_runs_source_time_idx ON sync_runs(source,started_at DESC);

INSERT OR IGNORE INTO source_registry(id,kind,display_name,config_json)
VALUES ('arxiv','preprint','arXiv','{"provider":"arxiv"}'),
       ('crossref','journal','Crossref','{"provider":"crossref"}');
-- Phase 2.3-B: per-subscription Crossref synchronization observability.
-- The subscription row is the durable cursor/status record for one journal;
-- papers and their source files remain untouched by this migration.
ALTER TABLE journal_subscriptions ADD COLUMN last_error_type TEXT;
ALTER TABLE journal_subscriptions ADD COLUMN last_query_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE journal_subscriptions ADD COLUMN last_fetched_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE journal_subscriptions ADD COLUMN last_parsed_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE journal_subscriptions ADD COLUMN last_inserted_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE journal_subscriptions ADD COLUMN last_updated_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE journal_subscriptions ADD COLUMN last_matched_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE journal_subscriptions ADD COLUMN last_pushed_count INTEGER NOT NULL DEFAULT 0;

CREATE INDEX journal_subscriptions_status_idx
  ON journal_subscriptions(journal_id, enabled, sync_enabled, last_status);

INSERT INTO sync_state(source) VALUES('arxiv'),('crossref');

-- Metadata schema required by the original home SearchService.
-- Derived from v1 migrations 001-012 (excluding AI); no seed records.
-- Business screens and write services are deliberately not enabled.
CREATE TABLE projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 120),
  description TEXT NOT NULL DEFAULT '',
  root_path TEXT NOT NULL DEFAULT '',
  cover TEXT NOT NULL DEFAULT 'crystal',
  status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('idea','active','paused','writing','submitted','completed','archived')),
  progress INTEGER NOT NULL DEFAULT 0 CHECK(progress BETWEEN 0 AND 100),
  tags TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tags)),
  favorite INTEGER NOT NULL DEFAULT 0 CHECK(favorite IN (0,1)),
  objective TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
, cover_path TEXT, project_color TEXT NOT NULL DEFAULT '#2878e6' CHECK(length(project_color)=7 AND project_color GLOB '#[0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F]'), sort_order INTEGER NOT NULL DEFAULT 0, progress_mode TEXT NOT NULL DEFAULT 'manual' CHECK(progress_mode IN ('manual','automatic')), current_focus TEXT NOT NULL DEFAULT '', next_step TEXT NOT NULL DEFAULT '', deleted_at TEXT);

CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 240),
  status TEXT NOT NULL DEFAULT 'todo' CHECK(status IN ('todo','completed')),
  priority TEXT NOT NULL DEFAULT 'medium' CHECK(priority IN ('low','medium','high','critical')),
  due_date TEXT NOT NULL DEFAULT (date('now','localtime')),
  scheduled_time TEXT NOT NULL DEFAULT '',
  category TEXT NOT NULL DEFAULT 'other' CHECK(category IN ('reading','calculation','analysis','figure','writing','experiment','meeting','other','question')),
  linked_object TEXT,
  notes TEXT NOT NULL DEFAULT '',
  tags TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tags)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
, question_status TEXT NOT NULL DEFAULT 'open' CHECK(question_status IN ('open','investigating','resolved')));

CREATE INDEX tasks_project_idx ON tasks(project_id,category,status);

CREATE INDEX tasks_due_idx ON tasks(due_date);

CREATE TABLE ideas (
 id TEXT PRIMARY KEY,
 project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
 title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 240),
 content TEXT NOT NULL DEFAULT '',
 status TEXT NOT NULL DEFAULT 'inbox' CHECK(status IN ('inbox','active','to_verify','validated','rejected','completed')),
 priority TEXT NOT NULL DEFAULT 'medium' CHECK(priority IN ('low','medium','high','critical')),
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

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

CREATE TABLE notes (
 id TEXT PRIMARY KEY, project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
 title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 240),
 content TEXT NOT NULL DEFAULT '',
 note_type TEXT NOT NULL DEFAULT 'general' CHECK(note_type IN ('general','literature','theory','calculation','meeting','idea','writing')),
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE tags (
 id TEXT PRIMARY KEY, name TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK(length(trim(name)) BETWEEN 1 AND 60 AND name=trim(name)),
 color TEXT CHECK(color IS NULL OR (length(color)=7 AND color GLOB '#[0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F]'))
);

CREATE INDEX ideas_project_idx ON ideas(project_id,status,priority);

CREATE INDEX notes_project_idx ON notes(project_id,note_type);

CREATE INDEX papers_title_idx ON papers(title COLLATE NOCASE);

CREATE TABLE calculations (
 id TEXT PRIMARY KEY,
 project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
 name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 240),
 description TEXT NOT NULL DEFAULT '',
 software TEXT NOT NULL DEFAULT 'custom' CHECK(software IN ('custom','python','matlab','vasp','qe','epw','phonopy','shengbte','lammps')),
 execution_type TEXT NOT NULL DEFAULT 'local' CHECK(execution_type IN ('local','hpc')),
 working_directory TEXT NOT NULL,
 command TEXT NOT NULL,
 status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','queued','running','finished','failed','cancelled')),
 parameters_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(parameters_json)),
 result_summary TEXT NOT NULL DEFAULT '',
 start_time TEXT, end_time TEXT, exit_code INTEGER, stdout_path TEXT, stderr_path TEXT,
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
, calculation_type TEXT NOT NULL DEFAULT 'generic', record_status TEXT NOT NULL DEFAULT 'unknown' CHECK(record_status IN ('unknown','prepared','running_external','completed','failed','incomplete')), parameter_sources_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(parameter_sources_json)), main_script TEXT);

CREATE INDEX calculations_project_idx ON calculations(project_id,updated_at DESC);

CREATE INDEX calculations_status_idx ON calculations(status,updated_at DESC);

CREATE TABLE datasets (
 id TEXT PRIMARY KEY,
 project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
 name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 500),
 path TEXT NOT NULL,
 category TEXT NOT NULL DEFAULT 'raw' CHECK(category IN ('raw','processed','derived')),
 format TEXT NOT NULL DEFAULT 'unknown',
 source TEXT NOT NULL DEFAULT 'manual' CHECK(source IN ('manual','calculation','imported','generated')),
 size INTEGER NOT NULL DEFAULT 0 CHECK(size>=0),
 description TEXT NOT NULL DEFAULT '',
 metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),
 hash TEXT,
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX datasets_project_idx ON datasets(project_id,updated_at DESC);

CREATE INDEX calculations_record_status_idx ON calculations(record_status,updated_at DESC);

CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE manuals (
 id TEXT PRIMARY KEY,
 title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 240),
 slug TEXT NOT NULL,
 path TEXT NOT NULL UNIQUE,
 folder_path TEXT NOT NULL,
 summary TEXT NOT NULL DEFAULT '',
 manual_type TEXT NOT NULL DEFAULT 'custom' CHECK(manual_type IN ('concept','tutorial','method','software','workflow','literature_review','custom')),
 category TEXT NOT NULL DEFAULT '',
 status TEXT NOT NULL DEFAULT 'not_started' CHECK(status IN ('not_started','learning','review','mastered')),
 progress INTEGER NOT NULL DEFAULT 0 CHECK(progress BETWEEN 0 AND 100),
 favorite INTEGER NOT NULL DEFAULT 0 CHECK(favorite IN (0,1)),
 origin TEXT NOT NULL DEFAULT 'created' CHECK(origin IN ('created','linked','copied')),
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 last_opened_at TEXT
);

CREATE INDEX manuals_status_idx ON manuals(status,updated_at DESC);

CREATE INDEX manuals_category_idx ON manuals(category,manual_type);

CREATE INDEX manuals_opened_idx ON manuals(last_opened_at DESC);

CREATE TABLE figures (
 id TEXT PRIMARY KEY,
 project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
 title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 240),
 description TEXT NOT NULL DEFAULT '',
 caption TEXT NOT NULL DEFAULT '',
 figure_type TEXT NOT NULL DEFAULT 'draft' CHECK(figure_type IN ('paper','analysis','draft','presentation','other')),
 status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','review','final')),
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX figures_project_idx ON figures(project_id,updated_at DESC);

CREATE TABLE manuscripts (
 id TEXT PRIMARY KEY,
 project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
 title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 500),
 journal TEXT NOT NULL DEFAULT '',
 manuscript_type TEXT NOT NULL DEFAULT 'paper' CHECK(manuscript_type IN ('paper','review','thesis','report','presentation')),
 status TEXT NOT NULL DEFAULT 'idea' CHECK(status IN ('idea','draft','revision','submitted','published')),
 path TEXT NOT NULL,
 description TEXT NOT NULL DEFAULT '',
 progress INTEGER NOT NULL DEFAULT 0 CHECK(progress BETWEEN 0 AND 100),
 submission_date TEXT,
 revision_deadline TEXT,
 decision TEXT NOT NULL DEFAULT '',
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX manuscripts_project_idx ON manuscripts(project_id,updated_at DESC);

CREATE TABLE manuscript_sections (
 id TEXT PRIMARY KEY,
 manuscript_id TEXT NOT NULL REFERENCES manuscripts(id) ON DELETE CASCADE,
 title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 240),
 section_type TEXT NOT NULL CHECK(section_type IN ('abstract','introduction','method','results','discussion','conclusion','supplementary','custom')),
 file_path TEXT NOT NULL,
 order_index INTEGER NOT NULL CHECK(order_index>=0),
 status TEXT NOT NULL DEFAULT 'todo' CHECK(status IN ('todo','draft','review','complete')),
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 UNIQUE(manuscript_id,file_path)
);

CREATE INDEX manuscript_sections_order_idx ON manuscript_sections(manuscript_id,order_index);

CREATE TABLE "object_tags" (
 id TEXT PRIMARY KEY,
 object_type TEXT NOT NULL CHECK(object_type IN ('project','idea','paper','note','task','calculation','dataset','manual','figure','manuscript')),
 object_id TEXT NOT NULL, tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
 UNIQUE(object_type,object_id,tag_id)
);

CREATE INDEX object_tags_object_idx ON object_tags(object_type,object_id);

CREATE INDEX object_tags_tag_idx ON object_tags(tag_id);

CREATE TABLE "relations" (
 id TEXT PRIMARY KEY,
 source_type TEXT NOT NULL CHECK(source_type IN ('project','idea','paper','note','task','calculation','dataset','manual','figure','manuscript')),
 source_id TEXT NOT NULL,
 target_type TEXT NOT NULL CHECK(target_type IN ('project','idea','paper','note','task','calculation','dataset','manual','figure','manuscript')),
 target_id TEXT NOT NULL,
 relation_type TEXT NOT NULL CHECK(relation_type IN ('belongs_to','related_to','supports','supported_by','contradicts','inspired_by','cites','discusses','generated_by')),
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
 CHECK(source_type != target_type OR source_id != target_id),
 UNIQUE(source_type,source_id,target_type,target_id,relation_type)
);

CREATE INDEX relations_source_idx ON relations(source_type,source_id);

CREATE INDEX relations_target_idx ON relations(target_type,target_id);

CREATE UNIQUE INDEX relations_symmetric_idx ON relations(
 min(source_type || ':' || source_id,target_type || ':' || target_id),
 max(source_type || ':' || source_id,target_type || ':' || target_id),relation_type
) WHERE relation_type IN ('related_to','contradicts');

CREATE INDEX projects_visibility_order_idx ON projects(deleted_at,status,sort_order);

CREATE VIEW knowledge_objects AS
 SELECT 'project' AS object_type,id,name AS title,description AS content,id AS project_id,updated_at FROM projects
 UNION ALL SELECT 'idea',id,title,content,project_id,updated_at FROM ideas
 UNION ALL SELECT 'paper',id,title,authors || ' ' || journal || ' ' || doi || ' ' || abstract,NULL,updated_at FROM papers
 UNION ALL SELECT 'note',id,title,content,project_id,updated_at FROM notes
 UNION ALL SELECT 'task',id,title,notes,project_id,updated_at FROM tasks
 UNION ALL SELECT 'calculation',id,name,description || ' ' || command || ' ' || parameters_json || ' ' || result_summary,project_id,updated_at FROM calculations
 UNION ALL SELECT 'dataset',id,name,description || ' ' || path || ' ' || metadata_json,project_id,updated_at FROM datasets
 UNION ALL SELECT 'manual',id,title,summary || ' ' || category || ' ' || COALESCE((SELECT group_concat(tags.name,' ') FROM object_tags ot JOIN tags ON tags.id=ot.tag_id WHERE ot.object_type='manual' AND ot.object_id=manuals.id),''),NULL,updated_at FROM manuals
 UNION ALL SELECT 'figure',id,title,description || ' ' || caption || ' ' || COALESCE((SELECT group_concat(tags.name,' ') FROM object_tags ot JOIN tags ON tags.id=ot.tag_id WHERE ot.object_type='figure' AND ot.object_id=figures.id),''),project_id,updated_at FROM figures
 UNION ALL SELECT 'manuscript',id,title,journal || ' ' || description || ' ' || COALESCE((SELECT group_concat(tags.name,' ') FROM object_tags ot JOIN tags ON tags.id=ot.tag_id WHERE ot.object_type='manuscript' AND ot.object_id=manuscripts.id),'') || ' ' || COALESCE((SELECT group_concat(title,' ') FROM manuscript_sections WHERE manuscript_id=manuscripts.id),''),project_id,updated_at FROM manuscripts;

CREATE VIEW project_objects AS
 SELECT project_id,object_type,id AS object_id FROM knowledge_objects WHERE project_id IS NOT NULL AND object_type!='project'
 UNION SELECT source_id,target_type,target_id FROM relations WHERE source_type='project'
 UNION SELECT target_id,source_type,source_id FROM relations WHERE target_type='project';

CREATE VIRTUAL TABLE knowledge_fts USING fts5(object_type UNINDEXED,object_id UNINDEXED,title,content,tokenize='trigram');

CREATE TRIGGER fts_project_insert AFTER INSERT ON projects BEGIN
 DELETE FROM knowledge_fts WHERE object_type='project' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='project' AND id=NEW.id;
END;

CREATE TRIGGER fts_project_update AFTER UPDATE ON projects BEGIN
 DELETE FROM knowledge_fts WHERE object_type='project' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='project' AND id=NEW.id;
END;

CREATE TRIGGER fts_project_delete AFTER DELETE ON projects BEGIN
 DELETE FROM knowledge_fts WHERE object_type='project' AND object_id=OLD.id;

END;

CREATE TRIGGER fts_idea_insert AFTER INSERT ON ideas BEGIN
 DELETE FROM knowledge_fts WHERE object_type='idea' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='idea' AND id=NEW.id;
END;

CREATE TRIGGER fts_idea_update AFTER UPDATE ON ideas BEGIN
 DELETE FROM knowledge_fts WHERE object_type='idea' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='idea' AND id=NEW.id;
END;

CREATE TRIGGER fts_idea_delete AFTER DELETE ON ideas BEGIN
 DELETE FROM knowledge_fts WHERE object_type='idea' AND object_id=OLD.id;

END;

CREATE TRIGGER fts_paper_insert AFTER INSERT ON papers BEGIN
 DELETE FROM knowledge_fts WHERE object_type='paper' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='paper' AND id=NEW.id;
END;

CREATE TRIGGER fts_paper_update AFTER UPDATE ON papers BEGIN
 DELETE FROM knowledge_fts WHERE object_type='paper' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='paper' AND id=NEW.id;
END;

CREATE TRIGGER fts_paper_delete AFTER DELETE ON papers BEGIN
 DELETE FROM knowledge_fts WHERE object_type='paper' AND object_id=OLD.id;

END;

CREATE TRIGGER fts_note_insert AFTER INSERT ON notes BEGIN
 DELETE FROM knowledge_fts WHERE object_type='note' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='note' AND id=NEW.id;
END;

CREATE TRIGGER fts_note_update AFTER UPDATE ON notes BEGIN
 DELETE FROM knowledge_fts WHERE object_type='note' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='note' AND id=NEW.id;
END;

CREATE TRIGGER fts_note_delete AFTER DELETE ON notes BEGIN
 DELETE FROM knowledge_fts WHERE object_type='note' AND object_id=OLD.id;

END;

CREATE TRIGGER fts_manual_insert AFTER INSERT ON manuals BEGIN
 DELETE FROM knowledge_fts WHERE object_type='manual' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='manual' AND id=NEW.id;
END;

CREATE TRIGGER fts_manual_update AFTER UPDATE ON manuals BEGIN
 DELETE FROM knowledge_fts WHERE object_type='manual' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='manual' AND id=NEW.id;
END;

CREATE TRIGGER fts_manual_delete AFTER DELETE ON manuals BEGIN
 DELETE FROM knowledge_fts WHERE object_type='manual' AND object_id=OLD.id;

END;

CREATE TRIGGER fts_dataset_insert AFTER INSERT ON datasets BEGIN
 DELETE FROM knowledge_fts WHERE object_type='dataset' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='dataset' AND id=NEW.id;
END;

CREATE TRIGGER fts_dataset_update AFTER UPDATE ON datasets BEGIN
 DELETE FROM knowledge_fts WHERE object_type='dataset' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='dataset' AND id=NEW.id;
END;

CREATE TRIGGER fts_dataset_delete AFTER DELETE ON datasets BEGIN
 DELETE FROM knowledge_fts WHERE object_type='dataset' AND object_id=OLD.id;

END;

CREATE TRIGGER fts_figure_insert AFTER INSERT ON figures BEGIN
 DELETE FROM knowledge_fts WHERE object_type='figure' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='figure' AND id=NEW.id;
END;

CREATE TRIGGER fts_figure_update AFTER UPDATE ON figures BEGIN
 DELETE FROM knowledge_fts WHERE object_type='figure' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='figure' AND id=NEW.id;
END;

CREATE TRIGGER fts_figure_delete AFTER DELETE ON figures BEGIN
 DELETE FROM knowledge_fts WHERE object_type='figure' AND object_id=OLD.id;

END;

CREATE TRIGGER fts_manuscript_insert AFTER INSERT ON manuscripts BEGIN
 DELETE FROM knowledge_fts WHERE object_type='manuscript' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='manuscript' AND id=NEW.id;
END;

CREATE TRIGGER fts_manuscript_update AFTER UPDATE ON manuscripts BEGIN
 DELETE FROM knowledge_fts WHERE object_type='manuscript' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='manuscript' AND id=NEW.id;
END;

CREATE TRIGGER fts_manuscript_delete AFTER DELETE ON manuscripts BEGIN
 DELETE FROM knowledge_fts WHERE object_type='manuscript' AND object_id=OLD.id;

END;

CREATE TRIGGER fts_task_insert AFTER INSERT ON tasks BEGIN
 DELETE FROM knowledge_fts WHERE object_type='task' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='task' AND id=NEW.id;
END;

CREATE TRIGGER fts_task_update AFTER UPDATE ON tasks BEGIN
 DELETE FROM knowledge_fts WHERE object_type='task' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='task' AND id=NEW.id;
END;

CREATE TRIGGER fts_task_delete AFTER DELETE ON tasks BEGIN
 DELETE FROM knowledge_fts WHERE object_type='task' AND object_id=OLD.id;

END;

CREATE TRIGGER fts_calculation_insert AFTER INSERT ON calculations BEGIN
 DELETE FROM knowledge_fts WHERE object_type='calculation' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='calculation' AND id=NEW.id;
END;

CREATE TRIGGER fts_calculation_update AFTER UPDATE ON calculations BEGIN
 DELETE FROM knowledge_fts WHERE object_type='calculation' AND object_id=NEW.id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='calculation' AND id=NEW.id;
END;

CREATE TRIGGER fts_calculation_delete AFTER DELETE ON calculations BEGIN
 DELETE FROM knowledge_fts WHERE object_type='calculation' AND object_id=OLD.id;

END;

CREATE TRIGGER fts_section_insert AFTER INSERT ON manuscript_sections BEGIN
 DELETE FROM knowledge_fts WHERE object_type='section' AND object_id=NEW.id;
 INSERT INTO knowledge_fts VALUES('section',NEW.id,NEW.title,'');
 DELETE FROM knowledge_fts WHERE object_type='manuscript' AND object_id=NEW.manuscript_id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='manuscript' AND id=NEW.manuscript_id;
END;

CREATE TRIGGER fts_section_update AFTER UPDATE ON manuscript_sections BEGIN
 DELETE FROM knowledge_fts WHERE object_type='section' AND object_id=NEW.id;
 INSERT INTO knowledge_fts VALUES('section',NEW.id,NEW.title,'');
 DELETE FROM knowledge_fts WHERE object_type='manuscript' AND object_id=NEW.manuscript_id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='manuscript' AND id=NEW.manuscript_id;
END;

CREATE TRIGGER fts_section_delete AFTER DELETE ON manuscript_sections BEGIN
 DELETE FROM knowledge_fts WHERE object_type='section' AND object_id=OLD.id;

 DELETE FROM knowledge_fts WHERE object_type='manuscript' AND object_id=OLD.manuscript_id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type='manuscript' AND id=OLD.manuscript_id;
END;

CREATE TRIGGER fts_tag_binding_insert AFTER INSERT ON object_tags BEGIN
 DELETE FROM knowledge_fts WHERE object_type=NEW.object_type AND object_id=NEW.object_id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type=NEW.object_type AND id=NEW.object_id;
END;

CREATE TRIGGER fts_tag_binding_delete AFTER DELETE ON object_tags BEGIN
 DELETE FROM knowledge_fts WHERE object_type=OLD.object_type AND object_id=OLD.object_id;
 INSERT INTO knowledge_fts SELECT object_type,id,title,content FROM knowledge_objects WHERE object_type=OLD.object_type AND id=OLD.object_id;
END;

CREATE TRIGGER fts_tag_update AFTER UPDATE ON tags BEGIN
 DELETE FROM knowledge_fts WHERE (object_type,object_id) IN (SELECT object_type,object_id FROM object_tags WHERE tag_id=NEW.id);
 INSERT INTO knowledge_fts SELECT k.object_type,k.id,k.title,k.content FROM knowledge_objects k WHERE EXISTS(SELECT 1 FROM object_tags ot WHERE ot.tag_id=NEW.id AND ot.object_type=k.object_type AND ot.object_id=k.id);
END;

CREATE TABLE scholar_recommendations (
    scholar_id TEXT NOT NULL,
    interests TEXT NOT NULL,
    items TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(items)),
    warnings TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(warnings)),
    updated_at TEXT,
    next_check INTEGER NOT NULL DEFAULT 0,
    lease_until INTEGER NOT NULL DEFAULT 0,
    claim TEXT,
    PRIMARY KEY(scholar_id,interests)
);

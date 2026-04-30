CREATE TABLE IF NOT EXISTS companies (
  id INTEGER PRIMARY KEY,
  name TEXT UNIQUE NOT NULL,
  size TEXT,
  summary TEXT
);

CREATE TABLE IF NOT EXISTS jobs (
  url TEXT PRIMARY KEY,
  company_id INTEGER REFERENCES companies(id),
  title TEXT NOT NULL,
  desc TEXT,
  req_hard TEXT,
  req_nice TEXT,
  platform TEXT NOT NULL,
  score INTEGER,
  rationale TEXT,
  user_action TEXT,
  ts INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS jobs_ts ON jobs(ts);
CREATE INDEX IF NOT EXISTS jobs_score ON jobs(score);

CREATE TABLE IF NOT EXISTS job_attempts (
  url TEXT PRIMARY KEY,
  platform TEXT NOT NULL,
  status TEXT NOT NULL,
  started_ts INTEGER NOT NULL,
  finished_ts INTEGER,
  error TEXT,
  extraction_json TEXT,
  score INTEGER,
  rationale TEXT
);

CREATE INDEX IF NOT EXISTS job_attempts_started_ts ON job_attempts(started_ts);
CREATE INDEX IF NOT EXISTS job_attempts_status ON job_attempts(status);

CREATE TABLE IF NOT EXISTS runs (
  id INTEGER PRIMARY KEY,
  status TEXT NOT NULL,
  started_ts INTEGER NOT NULL,
  finished_ts INTEGER,
  config_hash TEXT,
  error TEXT
);

CREATE TABLE IF NOT EXISTS stage_items (
  id INTEGER PRIMARY KEY,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  stage TEXT NOT NULL,
  status TEXT NOT NULL,
  input_ref TEXT,
  claim_id TEXT,
  attempts INTEGER NOT NULL DEFAULT 0,
  result_meta TEXT,
  created_ts INTEGER NOT NULL,
  updated_ts INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS stage_items_run_stage_status
ON stage_items(run_id, stage, status, id);

CREATE TABLE IF NOT EXISTS stage_events (
  id INTEGER PRIMARY KEY,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  stage TEXT NOT NULL,
  level TEXT NOT NULL,
  message TEXT NOT NULL,
  meta TEXT,
  ts INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS page_snapshots (
  id INTEGER PRIMARY KEY,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  platform TEXT NOT NULL,
  tab_url TEXT NOT NULL,
  fingerprint TEXT,
  snapshot_text TEXT,
  snapshot_ref TEXT,
  captured_ts INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS job_seeds (
  id INTEGER PRIMARY KEY,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  platform TEXT NOT NULL,
  canonical_url TEXT NOT NULL,
  title TEXT,
  company TEXT,
  location TEXT,
  snippet TEXT,
  source_snapshot_id INTEGER REFERENCES page_snapshots(id),
  rank INTEGER,
  status TEXT NOT NULL,
  reason TEXT,
  UNIQUE(run_id, canonical_url)
);

CREATE TABLE IF NOT EXISTS detail_snapshots (
  id INTEGER PRIMARY KEY,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  seed_id INTEGER NOT NULL REFERENCES job_seeds(id),
  canonical_url TEXT NOT NULL,
  fetch_status TEXT NOT NULL,
  snapshot_text TEXT,
  snapshot_ref TEXT,
  error TEXT,
  captured_ts INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT
);

INSERT OR IGNORE INTO meta(key, value) VALUES
  ('jobs_since_last_learn', '0'),
  ('next_cadence_idx', '0');

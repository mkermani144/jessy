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

CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT
);

INSERT OR IGNORE INTO meta(key, value) VALUES
  ('jobs_since_last_learn', '0'),
  ('next_cadence_idx', '0');

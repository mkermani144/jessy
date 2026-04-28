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

CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT
);

INSERT OR IGNORE INTO meta(key, value) VALUES
  ('jobs_since_last_learn', '0'),
  ('next_cadence_idx', '0');

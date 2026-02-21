CREATE TABLE IF NOT EXISTS jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    dedupe_key TEXT NOT NULL UNIQUE,
    canonical_url TEXT NOT NULL,
    company TEXT NOT NULL,
    title TEXT NOT NULL,
    location TEXT,
    work_mode TEXT,
    employment_type TEXT,
    posted_text TEXT,
    compensation_text TEXT,
    visa_policy_text TEXT,
    description TEXT NOT NULL,
    requirements_json TEXT NOT NULL,
    first_seen TEXT NOT NULL,
    last_seen TEXT NOT NULL,
    source_tab_url TEXT NOT NULL,
    source_page_index INTEGER NOT NULL,
    status TEXT NOT NULL,
    status_reason TEXT NOT NULL,
    requirements_summary TEXT NOT NULL,
    company_summary TEXT NOT NULL,
    company_size TEXT
);

CREATE INDEX IF NOT EXISTS idx_jobs_canonical_url ON jobs(canonical_url);
CREATE INDEX IF NOT EXISTS idx_jobs_last_seen ON jobs(last_seen);

CREATE TABLE IF NOT EXISTS run_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    status TEXT NOT NULL,
    total_scanned INTEGER NOT NULL,
    new_jobs INTEGER NOT NULL,
    opportunities INTEGER NOT NULL,
    not_opportunities INTEGER NOT NULL,
    error TEXT
);

CREATE TABLE IF NOT EXISTS run_job_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL,
    job_id INTEGER NOT NULL,
    is_new INTEGER NOT NULL,
    status TEXT NOT NULL,
    FOREIGN KEY (run_id) REFERENCES run_logs(id) ON DELETE CASCADE,
    FOREIGN KEY (job_id) REFERENCES jobs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS job_observations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL,
    run_id INTEGER NOT NULL,
    source_tab_url TEXT NOT NULL,
    page_index INTEGER NOT NULL,
    observed_at TEXT NOT NULL,
    FOREIGN KEY (job_id) REFERENCES jobs(id) ON DELETE CASCADE,
    FOREIGN KEY (run_id) REFERENCES run_logs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS search_page_fingerprints (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tab_key TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    page_index INTEGER NOT NULL,
    seen_at TEXT NOT NULL,
    UNIQUE(tab_key, fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_search_fingerprint_tab_key ON search_page_fingerprints(tab_key);

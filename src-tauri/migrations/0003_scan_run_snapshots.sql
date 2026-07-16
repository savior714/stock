BEGIN IMMEDIATE;

ALTER TABLE scan_runs ADD COLUMN preset_snapshot_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE scan_runs ADD COLUMN symbols_snapshot_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE scan_runs ADD COLUMN retry_of_run_id TEXT REFERENCES scan_runs(id) ON DELETE SET NULL;

CREATE INDEX idx_scan_runs_retry_of ON scan_runs(retry_of_run_id);

PRAGMA user_version = 3;
COMMIT;

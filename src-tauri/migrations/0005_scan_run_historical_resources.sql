-- Migrate scan_runs to use historical (non-FK) watchlist_id and preset_id.
-- This allows runs to persist and be retried even after the original
-- Watchlist/Preset resources are deleted.

PRAGMA foreign_keys = OFF;

BEGIN IMMEDIATE;

-- Create new table without live resource foreign keys
CREATE TABLE scan_runs_new (
    id TEXT PRIMARY KEY,
    watchlist_id TEXT NOT NULL,
    preset_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'cancelled', 'failed')),
    base_trade_date TEXT,
    total_symbols INTEGER NOT NULL DEFAULT 0 CHECK (total_symbols >= 0),
    succeeded_symbols INTEGER NOT NULL DEFAULT 0 CHECK (succeeded_symbols >= 0),
    failed_symbols INTEGER NOT NULL DEFAULT 0 CHECK (failed_symbols >= 0),
    started_at TEXT,
    finished_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    preset_snapshot_json TEXT NOT NULL DEFAULT '{}',
    symbols_snapshot_json TEXT NOT NULL DEFAULT '[]',
    retry_of_run_id TEXT REFERENCES scan_runs(id) ON DELETE SET NULL
);

-- Preserve all existing data
INSERT INTO scan_runs_new
    (id, watchlist_id, preset_id, status, base_trade_date,
     total_symbols, succeeded_symbols, failed_symbols,
     started_at, finished_at, created_at,
     preset_snapshot_json, symbols_snapshot_json, retry_of_run_id)
SELECT id, watchlist_id, preset_id, status, base_trade_date,
       total_symbols, succeeded_symbols, failed_symbols,
       started_at, finished_at, created_at,
       preset_snapshot_json, symbols_snapshot_json, retry_of_run_id
FROM scan_runs;

-- Drop old table and rename new one
DROP TABLE scan_runs;
ALTER TABLE scan_runs_new RENAME TO scan_runs;

-- Recreate indexes
CREATE INDEX idx_scan_runs_created_desc ON scan_runs(created_at DESC);
CREATE INDEX idx_scan_runs_retry_of ON scan_runs(retry_of_run_id);

-- Set schema version
PRAGMA user_version = 5;

COMMIT;

PRAGMA foreign_keys = ON;

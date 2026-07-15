BEGIN IMMEDIATE;

CREATE TABLE instruments (
    symbol TEXT PRIMARY KEY COLLATE NOCASE,
    provider_symbol TEXT NOT NULL,
    asset_type TEXT NOT NULL CHECK (asset_type IN ('stock', 'etf', 'adr')),
    exchange TEXT,
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE watchlists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE watchlist_symbols (
    watchlist_id TEXT NOT NULL,
    symbol TEXT NOT NULL COLLATE NOCASE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (watchlist_id, symbol),
    FOREIGN KEY (watchlist_id) REFERENCES watchlists(id) ON DELETE CASCADE,
    FOREIGN KEY (symbol) REFERENCES instruments(symbol) ON DELETE CASCADE
);

CREATE INDEX idx_watchlist_symbols_order
    ON watchlist_symbols(watchlist_id, sort_order, symbol);

CREATE TABLE scan_presets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    trigger_mode TEXT NOT NULL CHECK (trigger_mode IN ('current', 'cross')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE scan_preset_conditions (
    id TEXT PRIMARY KEY,
    preset_id TEXT NOT NULL,
    indicator TEXT NOT NULL CHECK (indicator IN ('bollinger', 'rsi', 'mfi')),
    side TEXT NOT NULL CHECK (side IN ('lower', 'upper')),
    period INTEGER NOT NULL CHECK (period > 0),
    threshold REAL,
    parameters_json TEXT NOT NULL DEFAULT '{}',
    is_enabled INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0, 1)),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (preset_id) REFERENCES scan_presets(id) ON DELETE CASCADE,
    UNIQUE (preset_id, indicator, side)
);

CREATE INDEX idx_scan_conditions_preset
    ON scan_preset_conditions(preset_id, is_enabled, sort_order);

CREATE TABLE daily_bars (
    symbol TEXT NOT NULL COLLATE NOCASE,
    trade_date TEXT NOT NULL,
    price_basis TEXT NOT NULL CHECK (price_basis IN ('raw', 'split_adjusted')),
    open REAL NOT NULL CHECK (open > 0),
    high REAL NOT NULL CHECK (high > 0),
    low REAL NOT NULL CHECK (low > 0),
    close REAL NOT NULL CHECK (close > 0),
    volume INTEGER NOT NULL CHECK (volume >= 0),
    provider TEXT NOT NULL DEFAULT 'yahoo',
    fetched_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (symbol, trade_date),
    FOREIGN KEY (symbol) REFERENCES instruments(symbol) ON DELETE CASCADE,
    CHECK (high >= open AND high >= low AND high >= close),
    CHECK (low <= open AND low <= high AND low <= close)
);

CREATE INDEX idx_daily_bars_symbol_date_desc
    ON daily_bars(symbol, trade_date DESC);

CREATE TABLE scan_runs (
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
    FOREIGN KEY (watchlist_id) REFERENCES watchlists(id),
    FOREIGN KEY (preset_id) REFERENCES scan_presets(id)
);

CREATE INDEX idx_scan_runs_created_desc
    ON scan_runs(created_at DESC);

CREATE TABLE scan_results (
    run_id TEXT NOT NULL,
    symbol TEXT NOT NULL COLLATE NOCASE,
    trade_date TEXT NOT NULL,
    current_price REAL NOT NULL CHECK (current_price > 0),
    rsi REAL,
    mfi REAL,
    bollinger_lower REAL,
    bollinger_middle REAL,
    bollinger_upper REAL,
    signal_flags_json TEXT NOT NULL DEFAULT '{}',
    matched_condition_count INTEGER NOT NULL DEFAULT 0 CHECK (matched_condition_count >= 0),
    all_conditions_matched INTEGER NOT NULL DEFAULT 0 CHECK (all_conditions_matched IN (0, 1)),
    any_condition_matched INTEGER NOT NULL DEFAULT 0 CHECK (any_condition_matched IN (0, 1)),
    data_stale INTEGER NOT NULL DEFAULT 0 CHECK (data_stale IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (run_id, symbol),
    FOREIGN KEY (run_id) REFERENCES scan_runs(id) ON DELETE CASCADE,
    FOREIGN KEY (symbol) REFERENCES instruments(symbol)
);

CREATE INDEX idx_scan_results_matches
    ON scan_results(run_id, all_conditions_matched, any_condition_matched);

CREATE TABLE scan_errors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    symbol TEXT COLLATE NOCASE,
    code TEXT NOT NULL,
    message TEXT NOT NULL,
    detail TEXT,
    retryable INTEGER NOT NULL DEFAULT 0 CHECK (retryable IN (0, 1)),
    attempt INTEGER NOT NULL DEFAULT 1 CHECK (attempt > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (run_id) REFERENCES scan_runs(id) ON DELETE CASCADE,
    FOREIGN KEY (symbol) REFERENCES instruments(symbol)
);

CREATE INDEX idx_scan_errors_run
    ON scan_errors(run_id, retryable, symbol);

PRAGMA user_version = 1;
COMMIT;

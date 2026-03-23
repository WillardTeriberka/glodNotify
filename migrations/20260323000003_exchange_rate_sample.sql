CREATE TABLE t_exchange_rate_sample (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  from_ccy TEXT NOT NULL,
  to_ccy TEXT NOT NULL,
  rate REAL NOT NULL,
  captured_at INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now', '+8 hours'))
);

CREATE INDEX idx_t_fx_sample_pair_captured
ON t_exchange_rate_sample (from_ccy, to_ccy, captured_at DESC);

-- 在 20240323000001 基础上扩展 t_gold_sample 字段，并新增汇率采样表。
-- 注意：不得再 CREATE t_workday / t_gold_sample / t_daily_extreme（已由上一迁移创建）。

ALTER TABLE t_gold_sample ADD COLUMN source_last_price REAL;
ALTER TABLE t_gold_sample ADD COLUMN field_1 REAL;
ALTER TABLE t_gold_sample ADD COLUMN field_2 REAL;
ALTER TABLE t_gold_sample ADD COLUMN high_price REAL;
ALTER TABLE t_gold_sample ADD COLUMN low_price REAL;
ALTER TABLE t_gold_sample ADD COLUMN prev_settlement REAL;
ALTER TABLE t_gold_sample ADD COLUMN open_price REAL;
ALTER TABLE t_gold_sample ADD COLUMN field_9 REAL;
ALTER TABLE t_gold_sample ADD COLUMN field_10 REAL;
ALTER TABLE t_gold_sample ADD COLUMN field_11 REAL;
ALTER TABLE t_gold_sample ADD COLUMN quote_time TEXT;
ALTER TABLE t_gold_sample ADD COLUMN quote_date TEXT;
ALTER TABLE t_gold_sample ADD COLUMN display_name TEXT;
ALTER TABLE t_gold_sample ADD COLUMN usd_cny_rate REAL;
ALTER TABLE t_gold_sample ADD COLUMN cny_per_gram REAL;

CREATE TABLE IF NOT EXISTS t_exchange_rate_sample (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  from_ccy TEXT NOT NULL,
  to_ccy TEXT NOT NULL,
  rate REAL NOT NULL,
  captured_at INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now', '+8 hours'))
);

CREATE INDEX IF NOT EXISTS idx_t_fx_sample_pair_captured
ON t_exchange_rate_sample (from_ccy, to_ccy, captured_at DESC);

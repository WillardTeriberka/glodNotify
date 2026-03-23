-- 全新初始化表结构（完整版本）
-- 说明：此文件仅输出完整建表结构，不包含 ALTER 语句。

CREATE TABLE t_workday (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  work_date TEXT NOT NULL,
  year INTEGER NOT NULL,
  is_workday INTEGER NOT NULL DEFAULT 1 CHECK (is_workday IN (0, 1)),
  date_name TEXT,
  create_by TEXT NOT NULL DEFAULT '',
  create_time TEXT NOT NULL DEFAULT (datetime('now', '+8 hours')),
  update_by TEXT NOT NULL DEFAULT '',
  update_time TEXT NOT NULL DEFAULT (datetime('now', '+8 hours')),
  remark TEXT,
  UNIQUE (work_date)
);

CREATE INDEX idx_t_workday_is_workday ON t_workday (is_workday);
CREATE INDEX idx_t_workday_year ON t_workday (year);

CREATE TABLE t_gold_sample (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  symbol TEXT NOT NULL,
  trade_date TEXT NOT NULL,
  captured_at INTEGER NOT NULL,
  last_price REAL NOT NULL,
  raw_quote TEXT NOT NULL,

  -- 原始接口当前价（未换算）
  source_last_price REAL,

  -- 来自 gds_AUTD 等接口字段的拆分
  field_1 REAL,
  field_2 REAL,
  high_price REAL,
  low_price REAL,
  prev_settlement REAL,
  open_price REAL,
  field_9 REAL,
  field_10 REAL,
  field_11 REAL,
  quote_time TEXT,
  quote_date TEXT,
  display_name TEXT,

  -- 汇率与换算结果（针对 hf_GC / hf_XAU）
  usd_cny_rate REAL,
  cny_per_gram REAL,

  created_at TEXT NOT NULL DEFAULT (datetime('now', '+8 hours'))
);

CREATE INDEX idx_t_gold_sample_trade_symbol ON t_gold_sample (trade_date, symbol);
CREATE INDEX idx_t_gold_sample_captured ON t_gold_sample (captured_at);

CREATE TABLE t_daily_extreme (
  trade_date TEXT NOT NULL,
  symbol TEXT NOT NULL,
  day_low REAL NOT NULL,
  day_high REAL NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now', '+8 hours')),
  PRIMARY KEY (trade_date, symbol)
);

CREATE INDEX idx_t_daily_extreme_trade_date ON t_daily_extreme (trade_date);

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

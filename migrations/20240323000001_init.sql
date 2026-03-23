-- 工作日缓存表（节假日接口落库）
-- 说明：SQLite 不支持 MySQL 的 COMMENT 字段注释语法，这里使用 SQL 注释表达字段含义。
CREATE TABLE t_workday (
  -- 主键ID
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  -- 日期（YYYY-MM-DD）
  work_date TEXT NOT NULL,
  -- 年份
  year INTEGER NOT NULL,
  -- 是否工作日：0-否，1-是
  is_workday INTEGER NOT NULL DEFAULT 1 CHECK (is_workday IN (0, 1)),
  -- 日期名称（如：周一、春节）
  date_name TEXT,
  -- 创建者
  create_by TEXT NOT NULL DEFAULT '',
  -- 创建时间
  create_time TEXT NOT NULL DEFAULT (datetime('now', '+8 hours')),
  -- 更新者
  update_by TEXT NOT NULL DEFAULT '',
  -- 更新时间（由应用更新）
  update_time TEXT NOT NULL DEFAULT (datetime('now', '+8 hours')),
  -- 备注
  remark TEXT,
  -- 每天唯一
  UNIQUE (work_date)
);

CREATE INDEX idx_t_workday_is_workday ON t_workday (is_workday);
CREATE INDEX idx_t_workday_year ON t_workday (year);

-- 黄金行情采样表（每10秒一条/品种）
CREATE TABLE t_gold_sample (
  -- 主键ID
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  -- 品种标识（如 gds_AUTD / hf_GC / hf_XAU）
  symbol TEXT NOT NULL,
  -- 交易日（YYYY-MM-DD）
  trade_date TEXT NOT NULL,
  -- 采样时间戳（Unix毫秒）
  captured_at INTEGER NOT NULL,
  -- 采样价（当前实现取CSV首个可解析浮点）
  last_price REAL NOT NULL,
  -- 原始CSV字符串（审计/排障）
  raw_quote TEXT NOT NULL,
  -- 入库时间
  created_at TEXT NOT NULL DEFAULT (datetime('now', '+8 hours'))
);

CREATE INDEX idx_t_gold_sample_trade_symbol ON t_gold_sample (trade_date, symbol);
CREATE INDEX idx_t_gold_sample_captured ON t_gold_sample (captured_at);

-- 日内极值表（每个交易日+品种一行）
CREATE TABLE t_daily_extreme (
  -- 交易日（YYYY-MM-DD）
  trade_date TEXT NOT NULL,
  -- 品种标识
  symbol TEXT NOT NULL,
  -- 当日最低价
  day_low REAL NOT NULL,
  -- 当日最高价
  day_high REAL NOT NULL,
  -- 最后更新时间
  updated_at TEXT NOT NULL DEFAULT (datetime('now', '+8 hours')),
  -- 联合主键：同一天同品种唯一
  PRIMARY KEY (trade_date, symbol)
);

CREATE INDEX idx_t_daily_extreme_trade_date ON t_daily_extreme (trade_date);

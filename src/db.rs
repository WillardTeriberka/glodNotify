use anyhow::Context;
use chrono::NaiveDate;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;

/// 连接 SQLite 并执行全部迁移。
///
/// 额外行为：
/// - 自动创建数据库父目录；
/// - 若数据库文件不存在则创建；
/// - 迁移失败会中止启动，避免“半初始化状态”运行。
pub async fn connect_and_migrate(database_path: &std::path::Path) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("create database parent dir {}", parent.display())
        })?;
    }

    let opts = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(opts)
        .await
        .with_context(|| format!("connect sqlite {}", database_path.display()))?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("run migrations")?;

    Ok(pool)
}

/// 统计指定年份在 `t_workday` 已缓存的记录数。
///
/// 用于“是否需要拉取当年节假日数据”的快速判定。
pub async fn count_workdays_for_year(pool: &SqlitePool, year: i32) -> anyhow::Result<i64> {
    let c: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM t_workday WHERE year = ?")
        .bind(year)
        .fetch_one(pool)
        .await?;
    Ok(c)
}

/// 判断给定日期是否为黄金交易日。
///
/// 规则：`t_workday` 中存在该日记录且 `is_workday = 1` 才返回 `true`。
pub async fn is_gold_trading_day(
    pool: &SqlitePool,
    day: NaiveDate,
) -> anyhow::Result<bool> {
    // 业务规则：当天有记录且 is_workday=1 才视为交易日。
    let s = day.format("%Y-%m-%d").to_string();
    let v: Option<i64> = sqlx::query_scalar(
        "SELECT is_workday FROM t_workday WHERE work_date = ?",
    )
    .bind(&s)
    .fetch_optional(pool)
    .await?;

    Ok(matches!(v, Some(1)))
}

#[derive(Debug, Clone, sqlx::FromRow)]
/// `t_daily_extreme` 查询结果（单日单品种极值）。
pub struct DailyExtremeRow {
    pub day_low: f64,
    pub day_high: f64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExchangeRateRow {
    pub rate: f64,
    pub captured_at: i64,
}

/// 查询指定交易日 + 品种的历史极值（若不存在返回 `None`）。
pub async fn fetch_daily_extreme(
    pool: &SqlitePool,
    trade_date: &str,
    symbol: &str,
) -> anyhow::Result<Option<DailyExtremeRow>> {
    let row = sqlx::query_as::<_, DailyExtremeRow>(
        "SELECT day_low, day_high FROM t_daily_extreme WHERE trade_date = ? AND symbol = ?",
    )
    .bind(trade_date)
    .bind(symbol)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// 写入单条行情采样。
///
/// 说明：
/// - `captured_at_ms` 保留机器可排序的毫秒时间戳；
/// - `created_at` 使用上海时区入库时间，便于人工排查。
pub async fn insert_gold_sample(
    pool: &SqlitePool,
    trade_date: &str,
    captured_at_ms: i64,
    q: &crate::gold_api::ParsedQuote,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"INSERT INTO t_gold_sample (
             symbol, trade_date, captured_at, last_price, raw_quote,
             source_last_price, field_1, field_2, high_price, low_price, prev_settlement, open_price,
             field_9, field_10, field_11, quote_time, quote_date, display_name, usd_cny_rate, cny_per_gram,
             created_at
           )
           VALUES (
             ?, ?, ?, ?, ?,
             ?, ?, ?, ?, ?, ?, ?,
             ?, ?, ?, ?, ?, ?, ?, ?,
             datetime('now', '+8 hours')
           )"#,
    )
    .bind(&q.symbol)
    .bind(trade_date)
    .bind(captured_at_ms)
    .bind(q.last_price)
    .bind(&q.raw_csv)
    .bind(q.source_last_price)
    .bind(q.field_1)
    .bind(q.field_2)
    .bind(q.high_price)
    .bind(q.low_price)
    .bind(q.prev_settlement)
    .bind(q.open_price)
    .bind(q.field_9)
    .bind(q.field_10)
    .bind(q.field_11)
    .bind(&q.quote_time)
    .bind(&q.quote_date)
    .bind(&q.display_name)
    .bind(q.usd_cny_rate)
    .bind(q.cny_per_gram)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_exchange_rate_sample(
    pool: &SqlitePool,
    from_ccy: &str,
    to_ccy: &str,
    rate: f64,
    captured_at_ms: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"INSERT INTO t_exchange_rate_sample (
             from_ccy, to_ccy, rate, captured_at, created_at
           ) VALUES (?, ?, ?, ?, datetime('now', '+8 hours'))"#,
    )
    .bind(from_ccy)
    .bind(to_ccy)
    .bind(rate)
    .bind(captured_at_ms)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fetch_latest_exchange_rate(
    pool: &SqlitePool,
    from_ccy: &str,
    to_ccy: &str,
) -> anyhow::Result<Option<ExchangeRateRow>> {
    let row = sqlx::query_as::<_, ExchangeRateRow>(
        r#"SELECT rate, captured_at
           FROM t_exchange_rate_sample
           WHERE from_ccy = ? AND to_ccy = ?
           ORDER BY captured_at DESC
           LIMIT 1"#,
    )
    .bind(from_ccy)
    .bind(to_ccy)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn upsert_daily_extreme(
    pool: &SqlitePool,
    trade_date: &str,
    symbol: &str,
    day_low: f64,
    day_high: f64,
) -> anyhow::Result<()> {
    // 使用 UPSERT 原子更新日内极值，便于后续比较是否触发告警。
    sqlx::query(
        r#"INSERT INTO t_daily_extreme (trade_date, symbol, day_low, day_high, updated_at)
           VALUES (?, ?, ?, ?, datetime('now', '+8 hours'))
           ON CONFLICT(trade_date, symbol) DO UPDATE SET
             day_low = excluded.day_low,
             day_high = excluded.day_high,
             updated_at = datetime('now', '+8 hours')"#,
    )
    .bind(trade_date)
    .bind(symbol)
    .bind(day_low)
    .bind(day_high)
    .execute(pool)
    .await?;
    Ok(())
}

/// 写入/更新工作日数据（按 `work_date` 幂等）。
///
/// 调用方会先将接口字段 `isOffDay` 反转为 `is_workday` 再传入。
pub async fn upsert_workday_row(
    pool: &SqlitePool,
    work_date: &str,
    year: i32,
    is_workday: bool,
    date_name: Option<&str>,
) -> anyhow::Result<()> {
    // 接口 isOffDay 与库中 is_workday 语义相反，调用方已完成转换。
    let flag: i64 = if is_workday { 1 } else { 0 };
    sqlx::query(
        r#"INSERT INTO t_workday (
             work_date, year, is_workday, date_name,
             create_by, update_by, update_time, remark
           ) VALUES (?, ?, ?, ?, 'system', 'system', datetime('now', '+8 hours'), NULL)
           ON CONFLICT(work_date) DO UPDATE SET
             year = excluded.year,
             is_workday = excluded.is_workday,
             date_name = excluded.date_name,
             update_by = 'system',
             update_time = datetime('now', '+8 hours')"#,
    )
    .bind(work_date)
    .bind(year)
    .bind(flag)
    .bind(date_name)
    .execute(pool)
    .await?;
    Ok(())
}

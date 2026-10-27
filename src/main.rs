// Windows release 构建为 GUI 子系统：启动后无控制台窗口、不占任务栏。
// 日志改写同目录 gold-notify.log（见 main 中 init_tracing）。
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod config;
mod db;
mod feishu;
mod gold_api;
mod workdays;

use anyhow::Context;
use chrono::{Datelike, FixedOffset, TimeZone, Utc};
use sqlx::SqlitePool;

const FX_REFRESH_INTERVAL_MS: i64 = 30 * 60 * 1000;

#[derive(Debug, Clone)]
struct FxCache {
    rate: f64,
    captured_at_ms: i64,
}

async fn resolve_usd_cny_rate(
    pool: &SqlitePool,
    http: &reqwest::Client,
    cache: &mut Option<FxCache>,
) -> anyhow::Result<f64> {
    let now_ms = Utc::now().timestamp_millis();
    let need_refresh = match cache {
        None => true,
        Some(v) => now_ms - v.captured_at_ms >= FX_REFRESH_INTERVAL_MS,
    };

    if need_refresh {
        match gold_api::fetch_usd_cny_rate(http).await {
            Ok(rate) => {
                let prev_rate = cache.as_ref().map(|x| x.rate);
                db::insert_exchange_rate_sample(pool, "USD", "CNY", rate, now_ms).await?;
                *cache = Some(FxCache {
                    rate,
                    captured_at_ms: now_ms,
                });
                tracing::info!(
                    from = "USD",
                    to = "CNY",
                    previous_rate = ?prev_rate,
                    new_rate = rate,
                    captured_at = %format_ts(now_ms),
                    "exchange rate refreshed and persisted"
                );
                return Ok(rate);
            }
            Err(e) => {
                tracing::warn!(error = %e, "fetch usd/cny failed, trying fallback");
            }
        }
    }

    if let Some(v) = cache {
        tracing::info!(
            from = "USD",
            to = "CNY",
            rate = v.rate,
            captured_at = %format_ts(v.captured_at_ms),
            "using cached exchange rate in memory"
        );
        return Ok(v.rate);
    }

    if let Some(v) = db::fetch_latest_exchange_rate(pool, "USD", "CNY").await? {
        *cache = Some(FxCache {
            rate: v.rate,
            captured_at_ms: v.captured_at,
        });
        tracing::info!(
            from = "USD",
            to = "CNY",
            rate = v.rate,
            captured_at = %format_ts(v.captured_at),
            "using fallback exchange rate from database"
        );
        return Ok(v.rate);
    }

    anyhow::bail!("no usd/cny rate available from api or database")
}

/// 处理一次行情批次：入库采样、更新极值、在创新高/新低时推送飞书。
async fn process_quotes(
    pool: &SqlitePool,
    http: &reqwest::Client,
    webhook: &str,
    trade_date: &str,
    captured_at_ms: i64,
    quotes: &[gold_api::ParsedQuote],
) -> anyhow::Result<()> {
    for q in quotes {
        db::insert_gold_sample(pool, trade_date, captured_at_ms, q).await?;

        let prev = db::fetch_daily_extreme(pool, trade_date, &q.symbol).await?;
        match prev {
            None => {
                // 当日首条样本仅初始化极值，不触发告警，避免启动即刷屏。
                db::upsert_daily_extreme(
                    pool,
                    trade_date,
                    &q.symbol,
                    q.last_price,
                    q.last_price,
                )
                .await?;
            }
            Some(ext) => {
                let mut low = ext.day_low;
                let mut high = ext.day_high;
                let new_low = q.last_price < ext.day_low;
                let new_high = q.last_price > ext.day_high;

                if new_low {
                    low = q.last_price;
                }
                if new_high {
                    high = q.last_price;
                }

                if new_low || new_high {
                    db::upsert_daily_extreme(pool, trade_date, &q.symbol, low, high).await?;
                }

                let name = q
                    .display_name
                    .clone()
                    .or_else(|| gold_api::extract_display_name(&q.raw_csv))
                    .unwrap_or_else(|| q.symbol.clone());

                let original_price = format_original_price(&q.symbol, q.source_last_price);
                let unified_suffix = format_unified_price_suffix(&q.symbol, q.last_price);
                if new_low {
                    let msg = format!(
                        "[黄金监控] {} ({}) 刷新当日新低：{}{} 日期 {} 采样 {}",
                        name,
                        q.symbol,
                        original_price,
                        unified_suffix,
                        trade_date,
                        format_ts(captured_at_ms)
                    );
                    feishu::send_text_webhook(http, webhook, &msg).await?;
                }
                if new_high {
                    let msg = format!(
                        "[黄金监控] {} ({}) 刷新当日新高：{}{} 日期 {} 采样 {}",
                        name,
                        q.symbol,
                        original_price,
                        unified_suffix,
                        trade_date,
                        format_ts(captured_at_ms)
                    );
                    feishu::send_text_webhook(http, webhook, &msg).await?;
                }
            }
        }
    }
    Ok(())
}

fn format_ts(ms: i64) -> String {
    let secs = ms / 1000;
    let nsecs = ((ms % 1000).max(0) as u32) * 1_000_000;
    let shanghai = FixedOffset::east_opt(8 * 3600).expect("valid +08:00 offset");
    match Utc.timestamp_opt(secs, nsecs) {
        chrono::LocalResult::Single(dt) => dt
            .with_timezone(&shanghai)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        _ => ms.to_string(),
    }
}

fn is_usd_oz_symbol(symbol: &str) -> bool {
    matches!(symbol, "hf_GC" | "hf_XAU")
}

fn format_original_price(symbol: &str, source_last_price: f64) -> String {
    if is_usd_oz_symbol(symbol) {
        format!("{:.2} 美元/盎司", source_last_price)
    } else {
        format!("{:.2} 元/克", source_last_price)
    }
}

fn format_unified_price_suffix(symbol: &str, unified_price: f64) -> String {
    if is_usd_oz_symbol(symbol) {
        format!("（统一价 {:.2} 元/克）", unified_price)
    } else {
        String::new()
    }
}

/// 固定以上海时区计算“今天”，避免容器本地时区差异影响交易日判断。
fn shanghai_today() -> chrono::NaiveDate {
    let shanghai = FixedOffset::east_opt(8 * 3600).expect("valid +08:00 offset");
    Utc::now().with_timezone(&shanghai).date_naive()
}

async fn poll_gold_once(
    pool: &SqlitePool,
    http: &reqwest::Client,
    webhook: &str,
    fx_cache: &mut Option<FxCache>,
) -> anyhow::Result<()> {
    // trade_date 以上海自然日切分，保证和工作日表一致。
    let today = shanghai_today();
    let trade_date = today.format("%Y-%m-%d").to_string();
    let body = gold_api::fetch_gold_index(http).await.context("fetch gold")?;
    let usd_cny_rate = resolve_usd_cny_rate(pool, http, fx_cache)
        .await
        .context("resolve usd/cny rate")?;
    let quotes = gold_api::parse_gold_js_body(&body, usd_cny_rate).context("parse gold body")?;
    if quotes.is_empty() {
        tracing::warn!("gold response contained no quotes");
        return Ok(());
    }

    let captured_at_ms = chrono::Utc::now().timestamp_millis();
    process_quotes(
        pool,
        http,
        webhook,
        &trade_date,
        captured_at_ms,
        &quotes,
    )
    .await?;

    Ok(())
}

fn default_env_filter() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("gold_notify=info,sqlx=warn"))
}

/// Windows release（GUI 子系统）下日志写入同目录 gold-notify.log。
#[cfg(all(target_os = "windows", not(debug_assertions)))]
fn init_tracing() -> anyhow::Result<()> {
    let log_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("gold-notify.log")))
        .unwrap_or_else(|| std::path::PathBuf::from("gold-notify.log"));
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("open log file {}", log_path.display()))?;
    tracing_subscriber::fmt()
        .with_env_filter(default_env_filter())
        .with_writer(std::sync::Mutex::new(file))
        .init();
    Ok(())
}

/// 其他环境（Linux / Windows debug）保持 stdout 输出。
#[cfg(not(all(target_os = "windows", not(debug_assertions))))]
fn init_tracing() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(default_env_filter()).init();
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing()?;

    let cfg = config::Config::from_env().context("load config")?;
    let pool = db::connect_and_migrate(&cfg.database_path)
        .await
        .context("database")?;

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .user_agent("gold-notify/0.1")
        .build()
        .context("build http client")?;

    tracing::info!(
        db = %cfg.database_path.display(),
        trading_poll_secs = cfg.trading_poll_secs,
        idle_poll_secs = cfg.idle_poll_secs,
        "gold-notify started"
    );

    let mut fx_cache: Option<FxCache> = None;

    loop {
        let today = shanghai_today();
        // 仅在本地缺失当年数据时拉取一次，平时走缓存。
        if let Err(e) = workdays::sync_year_if_needed(&pool, &http, today.year()).await {
            tracing::error!(error = %e, "sync workdays failed");
        }

        let trading = match db::is_gold_trading_day(&pool, today).await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "is_gold_trading_day");
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => break,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {},
                }
                continue;
            }
        };

        let sleep_dur = if trading {
            if let Err(e) = poll_gold_once(&pool, &http, &cfg.feishu_webhook_url, &mut fx_cache).await {
                tracing::error!(error = %e, "trading iteration failed");
            }
            cfg.trading_interval()
        } else {
            tracing::debug!(%today, "idle: not a gold trading day");
            cfg.idle_interval()
        };

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("shutdown");
                break;
            }
            _ = tokio::time::sleep(sleep_dur) => {},
        }
    }

    pool.close().await;
    Ok(())
}

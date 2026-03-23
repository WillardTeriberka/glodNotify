use std::env;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;

#[derive(Debug, Clone)]
/// 应用运行配置（来自环境变量）。
pub struct Config {
    pub database_path: PathBuf,
    pub feishu_webhook_url: String,
    /// Poll interval when today is a gold trading day (seconds).
    pub trading_poll_secs: u64,
    /// Sleep duration when not a trading day (seconds).
    pub idle_poll_secs: u64,
}

impl Config {
    /// 从环境变量加载配置。
    ///
    /// 必填：
    /// - `FEISHU_WEBHOOK_URL`
    ///
    /// 可选（带默认值）：
    /// - `DATABASE_PATH`（默认 `./data/gold.db`）
    /// - `TRADING_POLL_SECS`（默认 10）
    /// - `IDLE_POLL_SECS`（默认 3600）
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let database_path = env::var("DATABASE_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data/gold.db"));

        let feishu_webhook_url = env::var("FEISHU_WEBHOOK_URL")
            .context("FEISHU_WEBHOOK_URL is required")?;

        let trading_poll_secs = env::var("TRADING_POLL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        let idle_poll_secs = env::var("IDLE_POLL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3600);

        Ok(Config {
            database_path,
            feishu_webhook_url,
            trading_poll_secs,
            idle_poll_secs,
        })
    }

    /// 交易日轮询间隔。
    pub fn trading_interval(&self) -> Duration {
        Duration::from_secs(self.trading_poll_secs)
    }

    /// 非交易日休眠间隔。
    pub fn idle_interval(&self) -> Duration {
        Duration::from_secs(self.idle_poll_secs)
    }
}

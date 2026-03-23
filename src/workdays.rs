use std::collections::HashMap;

use anyhow::Context;
use reqwest::Client;
use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(Debug, Deserialize)]
/// 节假日接口单日数据结构。
struct DayEntry {
    date: String,
    name: String,
    #[serde(rename = "isOffDay")]
    is_off_day: bool,
}

/// 按“按年懒加载”策略同步工作日数据。
///
/// 行为：
/// - 若本地已有该年记录，直接返回；
/// - 若无记录，则请求节假日接口并逐条 UPSERT 到 `t_workday`。
///
/// 这样可以避免每次轮询都访问节假日接口，降低外部依赖压力。
pub async fn sync_year_if_needed(
    pool: &SqlitePool,
    http: &Client,
    year: i32,
) -> anyhow::Result<()> {
    let count = crate::db::count_workdays_for_year(pool, year).await?;
    if count > 0 {
        tracing::debug!(year, count, "workdays already cached");
        return Ok(());
    }

    let url = format!("https://api.jiejiariapi.com/v1/workdays/{}", year);
    tracing::info!(%url, "fetching workdays for year");

    let text = http
        .get(&url)
        .send()
        .await
        .context("workdays request")?
        .error_for_status()
        .context("workdays status")?
        .text()
        .await
        .context("workdays body")?;

    let map: HashMap<String, DayEntry> =
        serde_json::from_str(&text).context("parse workdays json")?;

    for (key, entry) in map.iter() {
        let work_date = key.as_str();
        let y: i32 = key
            .get(0..4)
            .and_then(|s| s.parse().ok())
            .unwrap_or(year);
        // 接口字段 isOffDay=true 表示休息日，因此这里取反得到 is_workday。
        let is_workday = !entry.is_off_day;
        crate::db::upsert_workday_row(
            pool,
            work_date,
            y,
            is_workday,
            Some(entry.name.as_str()),
        )
        .await
        .with_context(|| format!("upsert workday {}", work_date))?;
    }

    tracing::info!(year, rows = map.len(), "workdays synced");
    Ok(())
}

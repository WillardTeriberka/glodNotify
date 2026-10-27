use anyhow::Context;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;

/// 黄金接口解析后的标准结构。
///
/// - `symbol`：变量名去掉 `hq_str_` 前缀后的品种标识；
/// - `raw_csv`：原始 CSV（完整保留）；
/// - `last_price`：用于极值比较的采样价（当前策略为首个可解析浮点）。
pub struct ParsedQuote {
    pub symbol: String,
    pub raw_csv: String,
    /// 原始接口当前价（通常为美元/盎司或人民币计价，取决于品种）
    pub source_last_price: f64,
    /// 入库与日内极值比较使用的统一价格：
    /// - `hf_GC` / `hf_XAU`: 按汇率换算为人民币/克
    /// - 其他品种：沿用 source_last_price
    pub last_price: f64,
    pub field_1: Option<f64>,
    pub field_2: Option<f64>,
    pub high_price: Option<f64>,
    pub low_price: Option<f64>,
    pub prev_settlement: Option<f64>,
    pub open_price: Option<f64>,
    pub field_9: Option<f64>,
    pub field_10: Option<f64>,
    pub field_11: Option<f64>,
    pub quote_time: Option<String>,
    pub quote_date: Option<String>,
    pub display_name: Option<String>,
    pub usd_cny_rate: Option<f64>,
    pub cny_per_gram: Option<f64>,
}

const OZ_TO_GRAM: f64 = 31.103_476_8;
const USD_OZ_SYMBOLS: [&str; 2] = ["hf_GC", "hf_XAU"];

fn usd_oz_to_cny_gram(usd_per_oz: f64, usd_cny_rate: f64) -> f64 {
    usd_per_oz * usd_cny_rate / OZ_TO_GRAM
}

fn parse_f64(parts: &[&str], idx: usize) -> Option<f64> {
    parts.get(idx)?.trim().parse::<f64>().ok()
}

fn parse_non_empty(parts: &[&str], idx: usize) -> Option<String> {
    let s = parts.get(idx)?.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct ExchangeApiResponse {
    code: i32,
    msg: String,
    data: ExchangeApiData,
}

#[derive(Debug, Deserialize)]
struct ExchangeApiData {
    rate: f64,
}

/// 查询 USD->CNY 汇率。
pub async fn fetch_usd_cny_rate(client: &Client) -> anyhow::Result<f64> {
    let url = "https://v2.xxapi.cn/api/exchange?from=USD&to=CNY&amount=1&key=af801260badb0deb";
    let resp = client
        .get(url)
        .send()
        .await
        .context("exchange request")?
        .error_for_status()
        .context("exchange status")?
        .json::<ExchangeApiResponse>()
        .await
        .context("exchange json")?;
    if resp.code != 200 {
        anyhow::bail!("exchange api error: code={}, msg={}", resp.code, resp.msg);
    }
    Ok(resp.data.rate)
}

/// 解析黄金 API 返回的 JS 文本，提取全部 `hq_str_*` 行情。
///
/// 返回值会过滤掉无法解析出采样价的品种，避免污染后续极值逻辑。
pub fn parse_gold_js_body(body: &str, usd_cny_rate: f64) -> anyhow::Result<Vec<ParsedQuote>> {
    // 响应是形如 `var hq_str_xxx = "..."` 的 JS 文本。
    let re = Regex::new(r#"var\s+(hq_str_\w+)\s*=\s*"([^"]*)"\s*;?"#)
        .context("compile gold response regex")?;

    let mut out = Vec::new();
    for cap in re.captures_iter(body) {
        let full_var = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let inner = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let symbol = full_var
            .strip_prefix("hq_str_")
            .unwrap_or(full_var)
            .to_string();
        let raw_csv = inner.to_string();
        let (
            source_last_price,
            last_price,
            field_1,
            field_2,
            high_price,
            low_price,
            prev_settlement,
            open_price,
            field_9,
            field_10,
            field_11,
            quote_time,
            quote_date,
            display_name,
            cny_per_gram,
        ) = {
            let parts: Vec<&str> = raw_csv.split(',').map(|x| x.trim()).collect();
            let Some(source_last_price) = parse_f64(&parts, 0) else {
                tracing::warn!(%full_var, "skip quote: no numeric current price in csv[0]");
                continue;
            };
            let cny_per_gram = if USD_OZ_SYMBOLS.contains(&symbol.as_str()) {
                Some(usd_oz_to_cny_gram(source_last_price, usd_cny_rate))
            } else {
                None
            };
            let last_price = cny_per_gram.unwrap_or(source_last_price);
            let display_name =
                parse_non_empty(&parts, 13).or_else(|| extract_display_name(inner));
            let quote_date = parse_non_empty(&parts, 12);
            let quote_time = parse_non_empty(&parts, 6);
            (
                source_last_price,
                last_price,
                parse_f64(&parts, 1),
                parse_f64(&parts, 2),
                parse_f64(&parts, 3).or_else(|| parse_f64(&parts, 4)),
                parse_f64(&parts, 5),
                parse_f64(&parts, 7),
                parse_f64(&parts, 8),
                parse_f64(&parts, 9),
                parse_f64(&parts, 10),
                parse_f64(&parts, 11),
                quote_time,
                quote_date,
                display_name,
                cny_per_gram,
            )
        };

        out.push(ParsedQuote {
            symbol,
            raw_csv,
            source_last_price,
            last_price,
            field_1,
            field_2,
            high_price,
            low_price,
            prev_settlement,
            open_price,
            field_9,
            field_10,
            field_11,
            quote_time,
            quote_date,
            display_name,
            usd_cny_rate: cny_per_gram.map(|_| usd_cny_rate),
            cny_per_gram,
        });
    }

    Ok(out)
}

/// 请求黄金行情接口。
///
/// 参数 `t` 使用当前 Unix 毫秒，兼容目标接口的时间戳要求。
pub async fn fetch_gold_index(client: &Client) -> anyhow::Result<String> {
    let t = chrono::Utc::now().timestamp_millis();
    let url = format!(
        "https://www.huilvbiao.com/api/gold_indexApi?t={}",
        t
    );
    let text = client
        .get(&url)
        .send()
        .await
        .context("gold_index request")?
        .error_for_status()
        .context("gold_index status")?
        .text()
        .await
        .context("gold_index body")?;
    Ok(text)
}

/// 从原始 CSV 尾部提取展示名称（尽力而为）。
///
/// 例如：黄金延期、纽约黄金、伦敦金（现货黄金）。
pub fn extract_display_name(raw_csv: &str) -> Option<String> {
    for p in raw_csv.split(',').rev() {
        let s = p.trim();
        if s.is_empty() || s == "0" {
            continue;
        }
        if s.parse::<f64>().is_ok() {
            continue;
        }
        if s.contains(':') && s.len() <= 12 {
            let colon = s.matches(':').count();
            if colon == 1 {
                continue;
            }
        }
        return Some(s.to_string());
    }
    None
}

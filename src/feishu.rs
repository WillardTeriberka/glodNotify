use anyhow::Context;
use reqwest::Client;
use serde::Serialize;

#[derive(Serialize)]
/// 飞书机器人 text 消息体。
struct FeishuTextBody<'a> {
    msg_type: &'static str,
    content: FeishuTextContent<'a>,
}

#[derive(Serialize)]
/// 飞书 text 内容载体。
struct FeishuTextContent<'a> {
    text: &'a str,
}

/// 发送飞书文本消息。
///
/// 行为：
/// - 固定 `msg_type = "text"`，满足飞书 Webhook 最低要求；
/// - 最多重试 3 次（线性退避）；
/// - 非 2xx 状态码会读取响应体并返回错误，便于定位失败原因。
pub async fn send_text_webhook(
    client: &Client,
    webhook_url: &str,
    text: &str,
) -> anyhow::Result<()> {
    let body = FeishuTextBody {
        msg_type: "text",
        content: FeishuTextContent { text },
    };

    let mut last_err = None;
    // 简单线性退避重试，覆盖瞬时网络波动。
    for attempt in 0..3u32 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(400 * u64::from(attempt))).await;
        }
        match client
            .post(webhook_url)
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let bytes = resp.bytes().await.context("read feishu body")?;
                if status.is_success() {
                    tracing::debug!(?status, body = %String::from_utf8_lossy(&bytes), "feishu ok");
                    return Ok(());
                }
                last_err = Some(anyhow::anyhow!(
                    "feishu http {}: {}",
                    status,
                    String::from_utf8_lossy(&bytes)
                ));
            }
            Err(e) => {
                last_err = Some(anyhow::anyhow!(e));
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("feishu failed")))
}

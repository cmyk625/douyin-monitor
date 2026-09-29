use crate::models::NotifyResult;
use crate::util::now_ts;
use anyhow::{anyhow, Result};
use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use serde_json::json;
use sha2::Sha256;

/// 飞书自定义机器人签名：
/// 以 `timestamp + "\n" + secret` 作为 HMAC-SHA256 的密钥，对空内容做摘要，再 Base64 编码。
pub fn gen_sign(timestamp: i64, secret: &str) -> String {
    let string_to_sign = format!("{timestamp}\n{secret}");
    let mut mac = Hmac::<Sha256>::new_from_slice(string_to_sign.as_bytes())
        .expect("HMAC 支持任意长度的密钥");
    mac.update(b"");
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

pub fn is_configured(webhook: &str) -> bool {
    webhook.starts_with("https://") && webhook.contains("/open-apis/bot")
}

pub async fn send_text(
    client: &reqwest::Client,
    webhook: &str,
    secret: &str,
    text: &str,
) -> Result<NotifyResult> {
    send_text_with_retry(client, webhook, secret, text, 2).await
}

pub async fn send_text_with_retry(
    client: &reqwest::Client,
    webhook: &str,
    secret: &str,
    text: &str,
    max_retries: usize,
) -> Result<NotifyResult> {
    if webhook.trim().is_empty() {
        return Err(anyhow!("未配置飞书 Webhook 地址"));
    }

    let mut last_err = None;
    for attempt in 0..=max_retries {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(500 * (1 << (attempt - 1)))).await;
        }

        let mut payload = serde_json::Map::new();
        payload.insert("msg_type".into(), json!("text"));
        payload.insert("content".into(), json!({ "text": text }));

        if !secret.trim().is_empty() {
            let timestamp = now_ts();
            payload.insert("timestamp".into(), json!(timestamp.to_string()));
            payload.insert("sign".into(), json!(gen_sign(timestamp, secret.trim())));
        }

        let send_res = client
            .post(webhook.trim())
            .json(&serde_json::Value::Object(payload))
            .send()
            .await;

        let response = match send_res {
            Ok(resp) => resp,
            Err(err) => {
                last_err = Some(anyhow!("请求飞书失败：{err}"));
                continue;
            }
        };

        let status = response.status();
        let body = match response.text().await {
            Ok(body) => body,
            Err(err) => {
                last_err = Some(anyhow!("读取飞书响应失败：{err}"));
                continue;
            }
        };

        // 遇到 5xx 服务端错误且仍有重试机会时重试；4xx 客户端错误不重试
        if status.is_server_error() && attempt < max_retries {
            last_err = Some(anyhow!("HTTP {status}：{body}"));
            continue;
        }

        if !status.is_success() {
            return Ok(NotifyResult {
                ok: false,
                code: status.as_u16() as i64,
                msg: format!("HTTP {status}：{body}"),
            });
        }

        let parsed: serde_json::Value = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(_) => {
                return Ok(NotifyResult {
                    ok: false,
                    code: -1,
                    msg: format!("响应不是合法 JSON：{body}"),
                })
            }
        };

        let code = parsed
            .get("code")
            .and_then(|v| v.as_i64())
            .or_else(|| parsed.get("StatusCode").and_then(|v| v.as_i64()))
            .unwrap_or(0);
        let msg = parsed
            .get("msg")
            .and_then(|v| v.as_str())
            .or_else(|| parsed.get("StatusMessage").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();

        return Ok(NotifyResult {
            ok: code == 0,
            code,
            msg: if msg.is_empty() { body } else { msg },
        });
    }

    Err(last_err.unwrap_or_else(|| anyhow!("请求飞书失败（已达最大重试次数）")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_matches_reference_implementation() {
        // 参考实现（.NET HMACSHA256，独立于本 crate）：
        //   key = UTF8("1599360473\ndemo")，内容为空，结果 Base64 后如下
        let sign = gen_sign(1_599_360_473, "demo");
        assert_eq!(sign, "l1N0gAcBjdwBvGm1xMjOF0XSyaLRpR7tuO5dHfhAYc8=");
    }

    #[test]
    fn sign_is_stable() {
        assert_eq!(gen_sign(1_700_000_000, "s3cret"), gen_sign(1_700_000_000, "s3cret"));
        assert_ne!(gen_sign(1_700_000_000, "s3cret"), gen_sign(1_700_000_001, "s3cret"));
    }

    /// 用本地假 Webhook 校验请求体结构：签名、时间戳、文本内容、响应解析。
    #[tokio::test]
    async fn posts_signed_payload_and_parses_response() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0u8; 16 * 1024];
            let read = socket.read(&mut buffer).await.unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            let body = r#"{"code":0,"msg":"success"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
            request
        });

        let client = reqwest::Client::new();
        let webhook = format!("http://{addr}/open-apis/bot/v2/hook/test");
        let result = send_text(&client, &webhook, "demo-secret", "测试消息").await.unwrap();
        assert!(result.ok, "code={} msg={}", result.code, result.msg);
        assert_eq!(result.code, 0);

        let request = server.await.unwrap();
        let (headers, body) = request.split_once("\r\n\r\n").expect("请求应包含请求体");
        assert!(headers.starts_with("POST "), "应使用 POST：{headers}");
        let payload: serde_json::Value = serde_json::from_str(body.trim()).unwrap();
        assert_eq!(payload["msg_type"], "text");
        assert_eq!(payload["content"]["text"], "测试消息");
        let timestamp: i64 = payload["timestamp"]
            .as_str()
            .expect("应携带 timestamp")
            .parse()
            .unwrap();
        let sign = payload["sign"].as_str().expect("应携带 sign");
        assert_eq!(sign, gen_sign(timestamp, "demo-secret"));
    }

    /// 未配置密钥时不应携带签名字段。
    #[tokio::test]
    async fn omits_signature_without_secret() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0u8; 16 * 1024];
            let read = socket.read(&mut buffer).await.unwrap();
            let body = r#"{"StatusCode":0,"StatusMessage":"success"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&buffer[..read]).to_string()
        });

        let client = reqwest::Client::new();
        let webhook = format!("http://{addr}/open-apis/bot/v2/hook/test");
        let result = send_text(&client, &webhook, "", "无签名消息").await.unwrap();
        assert!(result.ok);

        let request = server.await.unwrap();
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        let payload: serde_json::Value = serde_json::from_str(body.trim()).unwrap();
        assert!(payload.get("timestamp").is_none());
        assert!(payload.get("sign").is_none());
        assert_eq!(payload["content"]["text"], "无签名消息");
    }

    /// 网络连通性检查（默认忽略，需要外网）：确认 rustls 配置可访问飞书 HTTPS 域名。
    #[tokio::test]
    #[ignore = "需要外网，手动运行：cargo test --lib -- --ignored tls_client"]
    async fn tls_client_reaches_feishu_https() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .user_agent("douyin-monitor/0.1 (+tauri)")
            .build()
            .expect("构建 HTTP 客户端失败");
        let response = client
            .get("https://open.feishu.cn/open-apis/bot/v2/hook/not-a-real-hook")
            .send()
            .await
            .expect("HTTPS 请求失败（TLS 配置或网络问题）");
        // 该地址必然返回业务错误，这里只验证 HTTPS 链路可用
        assert!(response.status().is_client_error() || response.status().is_success());
    }
}

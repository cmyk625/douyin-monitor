use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
    pub session_id: Option<String>,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

#[derive(Clone)]
pub struct CdpClient {
    tx: mpsc::UnboundedSender<String>,
    pending: Pending,
    next_id: Arc<AtomicU64>,
    events: broadcast::Sender<CdpEvent>,
}

impl CdpClient {
    /// 连接浏览器级 WebSocket 调试端点。
    pub async fn connect(ws_url: &str) -> Result<Self> {
        let (ws, _) = tokio::time::timeout(Duration::from_secs(10), tokio_tungstenite::connect_async(ws_url))
            .await
            .map_err(|_| anyhow!("连接浏览器调试端点超时（10 秒）：{ws_url}"))?
            .map_err(|err| anyhow!("连接浏览器调试端点失败：{err}"))?;

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let (events, _) = broadcast::channel::<CdpEvent>(512);

        let reader_pending = pending.clone();
        let reader_events = events.clone();
        let mut ws = ws;
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    outgoing = rx.recv() => {
                        let Some(text) = outgoing else { break };
                        if ws.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    incoming = ws.next() => {
                        match incoming {
                            Some(Ok(Message::Text(text))) => dispatch(&text, &reader_pending, &reader_events),
                            Some(Ok(Message::Binary(bytes))) => {
                                dispatch(&String::from_utf8_lossy(&bytes), &reader_pending, &reader_events)
                            }
                            Some(Ok(_)) => {}
                            Some(Err(_)) | None => break,
                        }
                    }
                }
            }
            if let Ok(mut map) = reader_pending.lock() {
                for (_, sender) in map.drain() {
                    let _ = sender.send(Err("浏览器连接已关闭".to_string()));
                }
            }
        });

        Ok(Self {
            tx,
            pending,
            next_id: Arc::new(AtomicU64::new(1)),
            events,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.events.subscribe()
    }

    pub async fn send(&self, method: &str, params: Value, session_id: Option<&str>) -> Result<Value> {
        self.send_timeout(method, params, session_id, Duration::from_secs(30)).await
    }

    pub async fn send_timeout(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
        timeout: Duration,
    ) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut message = serde_json::Map::new();
        message.insert("id".into(), json!(id));
        message.insert("method".into(), json!(method));
        message.insert("params".into(), params);
        if let Some(session) = session_id {
            message.insert("sessionId".into(), json!(session));
        }

        let (sender, receiver) = oneshot::channel();
        self.pending
            .lock()
            .map_err(|_| anyhow!("浏览器状态锁失效"))?
            .insert(id, sender);

        self.tx
            .send(Value::Object(message).to_string())
            .map_err(|_| anyhow!("浏览器连接不可用"))?;

        match tokio::time::timeout(timeout, receiver).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(message))) => Err(anyhow!("{method} 调用失败：{message}")),
            Ok(Err(_)) => Err(anyhow!("{method} 响应通道关闭")),
            Err(_) => {
                if let Ok(mut map) = self.pending.lock() {
                    map.remove(&id);
                }
                Err(anyhow!("{method} 超时（{} 秒）", timeout.as_secs()))
            }
        }
    }

    /// 在指定页面会话中执行 JS，返回结构化结果。
    pub async fn evaluate(&self, session_id: &str, expression: &str) -> Result<Value> {
        let response = self
            .send(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                    "userGesture": true,
                    "timeout": 30_000
                }),
                Some(session_id),
            )
            .await?;

        if let Some(details) = response.get("exceptionDetails") {
            let text = details
                .get("exception")
                .and_then(|value| value.get("description"))
                .or_else(|| details.get("text"))
                .and_then(|value| value.as_str())
                .unwrap_or("页面脚本执行异常");
            return Err(anyhow!("{text}"));
        }

        Ok(response
            .get("result")
            .and_then(|value| value.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }
}

fn dispatch(text: &str, pending: &Pending, events: &broadcast::Sender<CdpEvent>) {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return;
    };
    if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
        let result = if let Some(error) = value.get("error") {
            Err(error
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("未知错误")
                .to_string())
        } else {
            Ok(value.get("result").cloned().unwrap_or(Value::Null))
        };
        if let Ok(mut map) = pending.lock() {
            if let Some(sender) = map.remove(&id) {
                let _ = sender.send(result);
            }
        }
        return;
    }
    if let Some(method) = value.get("method").and_then(|v| v.as_str()) {
        let event = CdpEvent {
            method: method.to_string(),
            params: value.get("params").cloned().unwrap_or(Value::Null),
            session_id: value
                .get("sessionId")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string()),
        };
        let _ = events.send(event);
    }
}

//! 互传联盟协议 WebSocket 消息格式
//!
//! 消息格式: `type:id:name?payload`
//! - type: "action" 或 "ack"
//! - id: 消息 ID (数字)
//! - name: 动作名称
//! - payload: 可选的 JSON 载荷

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::LazyLock;

static MSG_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\w+):(\d+):(\w+)(\?(.*))?$").unwrap());

/// CatShare 兼容的 WebSocket 消息
#[derive(Debug, Clone)]
pub struct WsMessage {
    pub msg_type: String,
    pub id: u32,
    pub name: String,
    pub payload: Option<Value>,
}

impl std::fmt::Display for WsMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.msg_type, self.id, self.name)?;
        if let Some(payload) = &self.payload {
            write!(f, "?{}", payload)?;
        }
        Ok(())
    }
}

impl WsMessage {
    /// 解析 CatShare 格式的消息
    pub fn parse(text: &str) -> Option<Self> {
        let caps = MSG_PATTERN.captures(text)?;

        let msg_type = caps.get(1)?.as_str().to_string();
        let id: u32 = caps.get(2)?.as_str().parse().ok()?;
        let name = caps.get(3)?.as_str().to_string();

        let payload = caps
            .get(5)
            .and_then(|m| serde_json::from_str(m.as_str()).ok());

        Some(Self {
            msg_type,
            id,
            name,
            payload,
        })
    }

    /// 创建 action 消息
    pub fn action(id: u32, name: &str, payload: Option<Value>) -> Self {
        Self {
            msg_type: "action".to_string(),
            id,
            name: name.to_string(),
            payload,
        }
    }

    /// 创建 ack 响应消息
    pub fn ack(id: u32, name: &str, payload: Option<Value>) -> Self {
        Self {
            msg_type: "ack".to_string(),
            id,
            name: name.to_string(),
            payload,
        }
    }

    /// 创建版本协商消息
    pub fn version_negotiation(id: u32) -> Self {
        Self::action(
            id,
            "versionNegotiation",
            Some(serde_json::json!({
                "version": 1,
                "versions": [1]
            })),
        )
    }

    /// 创建状态消息
    pub fn status(id: u32, task_id: &str, status_type: i32, reason: &str) -> Self {
        Self::action(
            id,
            "status",
            Some(serde_json::json!({
                "taskId": task_id,
                "id": task_id,
                "type": status_type,
                "reason": reason
            })),
        )
    }
}

/// 发送请求载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendRequest {
    /// 任务 ID (某些版本可能使用 id 代替 taskId)
    #[serde(default)]
    pub task_id: Option<String>,
    /// 任务 ID 的别名
    #[serde(default)]
    pub id: Option<String>,
    /// 发送者 ID (可选)
    #[serde(default)]
    pub sender_id: Option<String>,
    pub sender_name: String,
    pub file_name: String,
    pub mime_type: String,
    pub file_count: u32,
    pub total_size: u64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cat_share_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub thumbnail: Option<String>,
}

impl SendRequest {
    /// 获取任务 ID，优先使用 task_id，否则使用 id
    pub fn get_task_id(&self) -> String {
        self.task_id
            .clone()
            .or_else(|| self.id.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// 获取发送者 ID
    pub fn get_sender_id(&self) -> String {
        self.sender_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_action() {
        let msg = WsMessage::parse("action:1:sendRequest?{\"taskId\":\"123\"}").unwrap();
        assert_eq!(msg.msg_type, "action");
        assert_eq!(msg.id, 1);
        assert_eq!(msg.name, "sendRequest");
        assert!(msg.payload.is_some());
    }

    #[test]
    fn test_parse_ack() {
        let msg = WsMessage::parse("ack:0:versionNegotiation?{\"version\":1}").unwrap();
        assert_eq!(msg.msg_type, "ack");
        assert_eq!(msg.id, 0);
        assert_eq!(msg.name, "versionNegotiation");
    }

    #[test]
    fn test_to_string() {
        let msg = WsMessage::version_negotiation(0);
        let text = msg.to_string();
        assert!(text.starts_with("action:0:versionNegotiation?"));
    }

    #[test]
    fn test_roundtrip() {
        let original = WsMessage::status(99, "task123", 1, "ok");
        let text = original.to_string();
        let parsed = WsMessage::parse(&text).unwrap();

        assert_eq!(parsed.msg_type, original.msg_type);
        assert_eq!(parsed.id, original.id);
        assert_eq!(parsed.name, original.name);
    }
}

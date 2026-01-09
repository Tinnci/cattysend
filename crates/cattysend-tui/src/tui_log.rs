//! TUI 日志层
//!
//! 自定义 tracing Layer，将日志发送到 TUI 的日志面板。

use crate::app::AppEvent;
use std::fmt;
use tokio::sync::mpsc;
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

/// 发送日志到 TUI 的 Layer
pub struct TuiLogLayer {
    tx: mpsc::Sender<AppEvent>,
}

impl TuiLogLayer {
    pub fn new(tx: mpsc::Sender<AppEvent>) -> Self {
        Self { tx }
    }
}

impl<S> Layer<S> for TuiLogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let level = event.metadata().level().to_string();

        // 提取日志消息
        let mut message = String::new();
        let mut visitor = MessageVisitor(&mut message);
        event.record(&mut visitor);

        // 如果消息为空，使用目标名称
        if message.is_empty() {
            message = event.metadata().target().to_string();
        }

        // 尝试发送到 TUI（非阻塞）
        let _ = self.tx.try_send(AppEvent::LogMessage { level, message });
    }
}

/// 访问者，用于提取事件中的消息字段
struct MessageVisitor<'a>(&'a mut String);

impl<'a> tracing::field::Visit for MessageVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            *self.0 = format!("{:?}", value);
        } else if self.0.is_empty() {
            // 如果还没有消息，使用第一个字段
            *self.0 = format!("{}={:?}", field.name(), value);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            *self.0 = value.to_string();
        } else if self.0.is_empty() {
            *self.0 = format!("{}={}", field.name(), value);
        }
    }
}

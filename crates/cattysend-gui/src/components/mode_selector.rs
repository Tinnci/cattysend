//! 模式选择器组件

use crate::state::AppMode;
use dioxus::prelude::*;

/// 发送/接收模式选择器
#[component]
pub fn ModeSelector(current_mode: AppMode, on_change: EventHandler<AppMode>) -> Element {
    let modes = vec![
        (AppMode::Home, "🏠", "主页", "发送文件"),
        (AppMode::Receiving, "📥", "接收", "等待连接"),
        (AppMode::Settings, "⚙️", "设置", "系统配置"),
    ];

    rsx! {
        for (mode, icon, title, desc) in modes {
            div {
                class: if current_mode == mode { "mode-card active" } else { "mode-card" },
                onclick: move |_| on_change.call(mode.clone()),
                div { class: "mode-card-icon", "{icon}" }
                div { class: "mode-card-title", "{title}" }
                div { class: "mode-card-desc", "{desc}" }
            }
        }
    }
}

//! 模式选择器组件

use crate::state::AppMode;
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct ModeSelectorProps {
    pub current_mode: AppMode,
    pub on_change: EventHandler<AppMode>,
}

/// 发送/接收模式选择器
#[component]
pub fn ModeSelector(props: ModeSelectorProps) -> Element {
    rsx! {
        div { class: "mode-selector",
            ModeButton {
                mode: AppMode::Sending,
                icon: "📤",
                title: "发送文件",
                description: "选择文件发送给附近设备",
                is_active: props.current_mode == AppMode::Sending,
                on_click: props.on_change,
            }

            ModeButton {
                mode: AppMode::Receiving,
                icon: "📥",
                title: "接收文件",
                description: "等待其他设备发送文件",
                is_active: props.current_mode == AppMode::Receiving,
                on_click: props.on_change,
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct ModeButtonProps {
    mode: AppMode,
    icon: &'static str,
    title: &'static str,
    description: &'static str,
    is_active: bool,
    on_click: EventHandler<AppMode>,
}

#[component]
fn ModeButton(props: ModeButtonProps) -> Element {
    let active_class = if props.is_active { "active" } else { "" };
    let mode = props.mode.clone();

    rsx! {
        div {
            class: "mode-btn {active_class}",
            onclick: move |_| props.on_click.call(mode.clone()),

            div { class: "mode-btn-icon", "{props.icon}" }
            div { class: "mode-btn-title", "{props.title}" }
            div { class: "mode-btn-desc", "{props.description}" }
        }
    }
}

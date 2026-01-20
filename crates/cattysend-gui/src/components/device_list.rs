//! 设备列表组件

use crate::state::DiscoveredDeviceInfo;
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct DeviceListProps {
    pub devices: Vec<DiscoveredDeviceInfo>,
    pub selected: Option<String>,
    pub on_select: EventHandler<String>,
    pub on_refresh: EventHandler<()>,
    pub is_scanning: bool,
}

/// 设备列表
#[component]
pub fn DeviceList(props: DeviceListProps) -> Element {
    rsx! {
        div { class: "card",
            div { class: "card-header",
                h2 { class: "card-title", "📱 附近设备" }
                button {
                    class: "btn btn-secondary btn-icon",
                    disabled: props.is_scanning,
                    onclick: move |_| props.on_refresh.call(()),
                    if props.is_scanning { "⏳" } else { "🔄" }
                }
            }

            if props.devices.is_empty() {
                div { class: "empty-state",
                    div { class: "empty-state-icon", "📡" }
                    p { class: "empty-state-text",
                        if props.is_scanning {
                            "正在扫描附近设备..."
                        } else {
                            "点击刷新按钮扫描附近设备"
                        }
                    }
                }
            } else {
                div { class: "device-list",
                    for device in props.devices.iter() {
                        DeviceItem {
                            key: "{device.address}",
                            device: device.clone(),
                            is_selected: props.selected.as_ref() == Some(&device.address),
                            on_click: props.on_select,
                        }
                    }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct DeviceItemProps {
    device: DiscoveredDeviceInfo,
    is_selected: bool,
    on_click: EventHandler<String>,
}

#[component]
fn DeviceItem(props: DeviceItemProps) -> Element {
    let selected_class = if props.is_selected { "selected" } else { "" };
    let address = props.device.address.clone();

    // 根据信号强度选择图标
    let signal_icon = match props.device.rssi {
        r if r > -50 => "📶",
        r if r > -70 => "📶",
        _ => "📶",
    };

    // 根据品牌选择设备图标
    let device_icon = match props.device.brand.as_deref() {
        Some("xiaomi") => "📱",
        Some("oppo") => "📱",
        Some("vivo") => "📱",
        Some("huawei") => "📱",
        _ => "💻",
    };

    rsx! {
        div {
            class: "device-item {selected_class}",
            onclick: move |_| props.on_click.call(address.clone()),

            div { class: "device-icon", "{device_icon}" }

            div { class: "device-info",
                div { class: "device-name", "{props.device.name}" }
                div { class: "device-address", "{props.device.address}" }
            }

            div { class: "device-rssi",
                span { "{signal_icon}" }
                span { "{props.device.rssi} dBm" }
            }
        }
    }
}

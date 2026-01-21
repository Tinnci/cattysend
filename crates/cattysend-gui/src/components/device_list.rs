//! 设备列表组件

use crate::state::DiscoveredDeviceInfo;
use dioxus::prelude::*;

/// 设备列表
#[component]
pub fn DeviceList(
    devices: Vec<DiscoveredDeviceInfo>,
    selected: Option<String>,
    on_select: EventHandler<String>,
    on_refresh: EventHandler<()>,
    is_scanning: bool,
) -> Element {
    rsx! {
        div {
            div { class: "card-header",
                h2 { "周边设备" }
                button {
                    class: "btn btn-accent",
                    disabled: is_scanning,
                    onclick: move |_| on_refresh.call(()),
                    if is_scanning { "扫描中..." } else { "刷新" }
                }
            }

            if devices.is_empty() {
                div { class: "empty-state",
                    div { class: "empty-state-icon", "🛰️" }
                    p { class: "empty-state-text", "正在监听无线电信号..." }
                }
            } else {
                div { class: "device-list",
                    for device in devices.iter() {
                        {
                            let addr = device.address.clone();
                            let is_selected = selected.as_deref() == Some(addr.as_str());
                            let class_name = if is_selected { "device-item selected" } else { "device-item" };
                            let icon = match device.brand.as_deref().unwrap_or("") {
                                "xiaomi" | "Xiaomi" => "📱",
                                "oppo" | "OPPO" => "📲",
                                _ => "💻"
                            };

                            rsx! {
                                div {
                                    key: "{addr}",
                                    class: "{class_name}",
                                    onclick: move |_| on_select.call(addr.clone()),

                                    div { class: "device-icon", "{icon}" }

                                    div { class: "device-info",
                                        div { class: "device-name", "{device.name}" }
                                        div { class: "device-address", "{device.address}" }
                                    }

                                    div { class: "device-rssi",
                                        "{device.rssi} dBm"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

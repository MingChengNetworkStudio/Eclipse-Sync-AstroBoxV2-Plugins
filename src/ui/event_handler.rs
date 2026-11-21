//! 事件处理：互联消息兑现协议等待槽、设备列表刷新、UI 事件路由。

use astrobox_ng_wit::astrobox::psys_host_v4::{device, dialog, ui};
use serde_json::Value;

use crate::ui::activation;
use crate::ui::build::rerender;
use crate::ui::chat;
use crate::ui::protocol;
use crate::ui::state::{self, MainTab};

// ========== 事件 ID 常量（build.rs 通过 glob 导入复用） ==========

// Tab 切换
pub const TAB_CHAT_EVENT: &str = "tab_chat";
pub const TAB_SETTINGS_EVENT: &str = "tab_settings";
pub const TAB_ABOUT_EVENT: &str = "tab_about";

// 激活流程
pub const ACT_START_EVENT: &str = "act_start";
pub const ACT_OPEN_VERIFY_EVENT: &str = "act_open_verify";
pub const ACT_VERIFY_PAID_EVENT: &str = "act_verify_paid";
pub const ACT_VERIFY_FREE_EVENT: &str = "act_verify_free";
pub const ACT_RETRY_EVENT: &str = "act_retry";

// 设备
pub const REFRESH_DEVICES_EVENT: &str = "action:devices.refresh";

// 聊天
pub const CHAT_REFRESH_EVENT: &str = "chat:refresh";
pub const CHAT_NEW_EVENT: &str = "chat:new";
pub const CHAT_OPEN_PREFIX: &str = "chat:open:";
pub const CHAT_DEL_PREFIX: &str = "chat:del:";
pub const CHAT_BACK_EVENT: &str = "chat:back";
pub const CHAT_LOAD_OLDER_EVENT: &str = "chat:older";
pub const CHAT_PROVIDER_EVENT: &str = "chat:provider";
pub const CHAT_MODEL_EVENT: &str = "chat:model";
pub const CHAT_INPUT_EVENT: &str = "chat:input";
pub const CHAT_SEND_EVENT: &str = "chat:send";
pub const CHAT_STOP_EVENT: &str = "chat:stop";

// 设置
pub const SET_REFRESH_AUTH_EVENT: &str = "set:refresh_auth";
pub const SET_TOGGLE_KEY_EVENT: &str = "set:toggle_key";
pub const SET_REVOKE_EVENT: &str = "set:revoke";

// 关于
pub const OPEN_HELP_DOC_EVENT: &str = "open_help_doc";
pub const OPEN_QQ_GROUP_EVENT: &str = "open_qq_group";

// ========== 互联消息 ==========

/// 宿主信封里拆出的来源信息
struct ParsedMessage {
    addr: String,
    pkg_name: String,
    data: String,
}

fn parse_message(payload: &str) -> ParsedMessage {
    let envelope: Option<Value> = serde_json::from_str(payload).ok();
    let data = envelope
        .as_ref()
        .and_then(|v| v.get("payloadText").and_then(|x| x.as_str()))
        .map(str::to_string)
        .unwrap_or_else(|| payload.to_string());

    let pkg_name = envelope
        .as_ref()
        .and_then(|v| v.get("pkgName").and_then(|x| x.as_str()))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_default();

    let addr = envelope
        .as_ref()
        .and_then(|v| v.get("addr").and_then(|x| x.as_str()))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_default();

    ParsedMessage {
        addr,
        pkg_name,
        data,
    }
}

/// 处理快应用消息：拆信封后交给协议层兑现等待槽
pub async fn handle_interconnect_message(payload: &str) {
    let parsed = parse_message(payload);
    if parsed.addr.is_empty() || parsed.pkg_name.is_empty() {
        tracing::warn!("丢弃无法定位来源(addr/pkg)的互联消息");
        return;
    }

    let Ok(msg) = serde_json::from_str::<Value>(&parsed.data) else {
        tracing::warn!("解析互联消息失败: {}", parsed.data);
        return;
    };

    let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let status = msg.get("status").and_then(|v| v.as_str()).unwrap_or("OK");
    let data = msg.get("data").cloned();

    if msg_type.is_empty() {
        return;
    }

    if !protocol::resolve(&parsed.addr, msg_type, status, data) {
        tracing::info!("收到无人等待的消息类型: {}", msg_type);
    }
}

// ========== 设备管理 ==========

/// 插件加载时的初始设备刷新
pub async fn initial_refresh() {
    refresh_connected_devices().await;
}

/// 手动刷新设备列表
pub async fn refresh_device_list() {
    state::set_devices_loading(true);
    rerender();
    refresh_connected_devices().await;
    state::set_devices_loading(false);
    rerender();
}

async fn refresh_connected_devices() {
    let devices = device::get_connected_device_list()
        .await
        .into_iter()
        .map(|info| (info.addr, info.name))
        .collect();
    state::set_devices(devices);
}

// ========== UI 事件 ==========

pub async fn ui_event_processor(
    _event_type: ui::Event,
    event_id: &str,
    event_payload: &str,
) {
    // 带列表下标的事件优先匹配
    if let Some(idx_str) = event_id.strip_prefix(CHAT_OPEN_PREFIX) {
        if let Ok(idx) = idx_str.parse::<usize>() {
            chat::open_session(idx).await;
        }
        return;
    }
    if let Some(idx_str) = event_id.strip_prefix(CHAT_DEL_PREFIX) {
        if let Ok(idx) = idx_str.parse::<usize>() {
            chat::delete_session(idx).await;
        }
        return;
    }

    match event_id {
        // Tab
        TAB_CHAT_EVENT => {
            switch_tab(MainTab::Chat);
            // 进入聊天 Tab 时按需拉取一次会话与供应商，避免重复拉起快应用
            if state::is_activated() {
                let need_sessions = state::with_state(|s| s.sessions.is_empty());
                // 官方占位对象的模型列表为空，说明还没拉过供应商
                let need_providers = state::with_state(|s| {
                    s.active_provider.is_official && s.active_provider.models.is_empty()
                });
                if need_sessions {
                    chat::refresh_sessions().await;
                }
                if need_providers {
                    chat::refresh_providers().await;
                }
            }
        }
        TAB_SETTINGS_EVENT => switch_tab(MainTab::Settings),
        TAB_ABOUT_EVENT => switch_tab(MainTab::About),

        // 激活
        ACT_START_EVENT | ACT_RETRY_EVENT => activation::start_activation().await,
        ACT_OPEN_VERIFY_EVENT => activation::open_verify_url().await,
        ACT_VERIFY_PAID_EVENT => activation::verify_payment(false).await,
        ACT_VERIFY_FREE_EVENT => activation::verify_payment(true).await,

        // 设备
        REFRESH_DEVICES_EVENT => refresh_device_list().await,

        // 聊天
        CHAT_REFRESH_EVENT => {
            chat::refresh_sessions().await;
            chat::refresh_providers().await;
        }
        CHAT_NEW_EVENT => chat::new_session().await,
        CHAT_BACK_EVENT => chat::back_to_list(),
        CHAT_LOAD_OLDER_EVENT => chat::load_older().await,
        CHAT_PROVIDER_EVENT => {
            let label = parse_event_value(event_payload);
            chat::select_provider_by_label(&label).await;
        }
        CHAT_MODEL_EVENT => {
            let model = parse_event_value(event_payload);
            state::set_selected_model(&model);
            rerender();
        }
        CHAT_INPUT_EVENT => {
            let text = parse_event_value(event_payload);
            state::set_draft(&text);
        }
        CHAT_SEND_EVENT => chat::send_message().await,
        CHAT_STOP_EVENT => chat::stop_streaming(),

        // 设置
        SET_REFRESH_AUTH_EVENT => activation::refresh_auth_info().await,
        SET_TOGGLE_KEY_EVENT => {
            state::with_state(|s| s.api_key_visible = !s.api_key_visible);
            rerender();
        }
        SET_REVOKE_EVENT => activation::delete_local_auth().await,

        // 关于
        OPEN_HELP_DOC_EVENT => dialog::open_url("https://docs.b4qaq.cn/docs/eclipse/"),
        OPEN_QQ_GROUP_EVENT => dialog::open_url(
            "http://qm.qq.com/cgi-bin/qm/qr?_wv=1027&k=1vc4XKmAyGeJJTmXumfkaaxRcQl1hMaK",
        ),
        _ => {}
    }
}

fn switch_tab(tab: MainTab) {
    state::with_state(|s| s.current_tab = tab);
    rerender();
}

/// 提取 UI 事件 payload 里的文本值（Input/Select 回执可能包了多层 JSON）
fn parse_event_value(payload: &str) -> String {
    if let Ok(json) = serde_json::from_str::<Value>(payload) {
        if let Some(value) = extract_event_value(&json) {
            return value.trim().to_string();
        }
    }
    payload.trim().to_string()
}

fn extract_event_value(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    for key in ["value", "text", "content", "label"] {
        if let Some(text) = value.get(key).and_then(|v| v.as_str()) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    for key in ["detail", "target", "currentTarget", "data"] {
        if let Some(nested) = value.get(key) {
            if let Some(text) = extract_event_value(nested) {
                return Some(text);
            }
        }
    }
    None
}

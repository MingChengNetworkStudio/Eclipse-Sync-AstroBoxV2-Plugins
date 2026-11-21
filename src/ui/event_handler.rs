//! 事件处理：互联消息兑现协议等待槽、设备列表刷新、UI 事件路由。

use astrobox_ng_wit::astrobox::psys_host_v4::{device, dialog, ui};
use serde_json::Value;

use crate::ui::activation;
use crate::ui::build::rerender;
use crate::ui::chat;
use crate::ui::files;
use crate::ui::prompts;
use crate::ui::protocol;
use crate::ui::state::{self, MainTab};

// ========== 事件 ID 常量（build.rs 通过 glob 导入复用） ==========

// Tab 切换
pub const TAB_CHAT_EVENT: &str = "tab_chat";
pub const TAB_FILES_EVENT: &str = "tab_files";
pub const TAB_AI_EVENT: &str = "tab_ai";
pub const TAB_SETTINGS_EVENT: &str = "tab_settings";

// 激活流程
pub const ACT_START_EVENT: &str = "act_start";
pub const ACT_OPEN_VERIFY_EVENT: &str = "act_open_verify";
pub const ACT_VERIFY_PAID_EVENT: &str = "act_verify_paid";
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
pub const CHAT_MODEL_EVENT: &str = "chat:model";
pub const CHAT_INPUT_EVENT: &str = "chat:input";
pub const CHAT_SEND_EVENT: &str = "chat:send";
pub const CHAT_STOP_EVENT: &str = "chat:stop";

// 文件管理
pub const FILES_REFRESH_EVENT: &str = "files:refresh";
pub const FILES_PREVIEW_PREFIX: &str = "files:preview:";
pub const FILES_DOWNLOAD_PREFIX: &str = "files:download:";
pub const FILES_BACK_EVENT: &str = "files:back";
pub const FILES_DOWNLOAD_CURRENT_EVENT: &str = "files:download_current";

// AI 设置
pub const AI_REFRESH_EVENT: &str = "ai:refresh";
pub const AI_PROVIDER_EVENT: &str = "ai:provider";
pub const AI_PROVIDER_NEW_EVENT: &str = "ai:provider_new";
pub const AI_PROVIDER_SAVE_EVENT: &str = "ai:provider_save";
pub const AI_PROVIDER_CANCEL_EVENT: &str = "ai:provider_cancel";
pub const AI_PROVIDER_EDIT_PREFIX: &str = "ai:provider_edit:";
pub const AI_PROVIDER_DEL_PREFIX: &str = "ai:provider_del:";
pub const AI_PROVIDER_NAME_INPUT: &str = "ai:provider_name_input";
pub const AI_PROVIDER_BASEURL_INPUT: &str = "ai:provider_baseurl_input";
pub const AI_PROVIDER_APIKEY_INPUT: &str = "ai:provider_apikey_input";
pub const AI_MODEL_INPUT_PREFIX: &str = "ai:model_input:";
pub const AI_MODEL_ADD_EVENT: &str = "ai:model_add";
pub const AI_MODEL_DEL_PREFIX: &str = "ai:model_del:";
pub const AI_PROMPT_SELECT_EVENT: &str = "ai:prompt_select";
pub const AI_PROMPT_NEW_EVENT: &str = "ai:prompt_new";
pub const AI_PROMPT_SAVE_EVENT: &str = "ai:prompt_save";
pub const AI_PROMPT_CANCEL_EVENT: &str = "ai:prompt_cancel";
pub const AI_PROMPT_EDIT_PREFIX: &str = "ai:prompt_edit:";
pub const AI_PROMPT_DEL_PREFIX: &str = "ai:prompt_del:";
pub const AI_PROMPT_NAME_INPUT: &str = "ai:prompt_name_input";
pub const AI_PROMPT_CONTENT_INPUT: &str = "ai:prompt_content_input";

// 设置
pub const SET_REFRESH_AUTH_EVENT: &str = "set:refresh_auth";
pub const SET_TOGGLE_KEY_EVENT: &str = "set:toggle_key";
pub const SET_REVOKE_EVENT: &str = "set:revoke";
pub const SET_RECHARGE_PREFIX: &str = "set:recharge:";

// 关于（并入设置页）
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

/// 处理快应用消息：拆信封后按 pending 登记驱动对应流程
pub async fn handle_interconnect_message(payload: &str) {
    let mut parsed = parse_message(payload);

    // 兜底：个别宿主回执可能不带 addr/pkgName，用第一台设备与固定包名补齐
    if parsed.addr.is_empty() {
        if let Some(addr) = state::first_device_addr() {
            parsed.addr = addr;
        }
    }
    if parsed.pkg_name.is_empty() {
        parsed.pkg_name = state::PKG_NAME.to_string();
    }
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

    // 取出该回执对应的待处理请求；无人等待则忽略
    let Some(kind) = protocol::take_pending(&parsed.addr, msg_type) else {
        tracing::info!("收到无人等待的消息类型: {}", msg_type);
        return;
    };

    let ok = status.is_empty() || status == "OK";
    match kind {
        protocol::PendingKind::ActApikey => {
            activation::on_apikey_reply(ok, status, data).await;
        }
        protocol::PendingKind::ActDeviceInfo => {
            activation::on_device_info_reply(ok, status, data).await;
        }
        protocol::PendingKind::PutSettings => {
            if !ok {
                tracing::warn!("PUT_SETTINGS 失败: {}", status);
            }
        }
        other => {
            chat::dispatch_reply(other, ok, status, data).await;
        }
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
    if let Some(idx_str) = event_id.strip_prefix(FILES_PREVIEW_PREFIX) {
        if let Ok(idx) = idx_str.parse::<usize>() {
            files::open_file(idx, protocol::FileAction::Preview).await;
        }
        return;
    }
    if let Some(idx_str) = event_id.strip_prefix(FILES_DOWNLOAD_PREFIX) {
        if let Ok(idx) = idx_str.parse::<usize>() {
            files::open_file(idx, protocol::FileAction::Download).await;
        }
        return;
    }
    if let Some(idx_str) = event_id.strip_prefix(AI_PROMPT_EDIT_PREFIX) {
        if let Ok(idx) = idx_str.parse::<usize>() {
            prompts::edit_prompt(idx);
            rerender();
        }
        return;
    }
    if let Some(idx_str) = event_id.strip_prefix(AI_PROMPT_DEL_PREFIX) {
        if let Ok(idx) = idx_str.parse::<usize>() {
            prompts::delete_prompt(idx).await;
        }
        return;
    }
    if let Some(idx_str) = event_id.strip_prefix(AI_PROVIDER_EDIT_PREFIX) {
        if let Ok(idx) = idx_str.parse::<usize>() {
            chat::edit_provider(idx);
            rerender();
        }
        return;
    }
    if let Some(idx_str) = event_id.strip_prefix(AI_PROVIDER_DEL_PREFIX) {
        if let Ok(idx) = idx_str.parse::<usize>() {
            chat::delete_provider(idx).await;
        }
        return;
    }
    if let Some(idx_str) = event_id.strip_prefix(AI_MODEL_INPUT_PREFIX) {
        if let Ok(idx) = idx_str.parse::<usize>() {
            state::update_provider_model(idx, &parse_event_value(event_payload));
        }
        return;
    }
    if let Some(idx_str) = event_id.strip_prefix(AI_MODEL_DEL_PREFIX) {
        if let Ok(idx) = idx_str.parse::<usize>() {
            state::remove_provider_model(idx);
            rerender();
        }
        return;
    }
    if let Some(order_str) = event_id.strip_prefix(SET_RECHARGE_PREFIX) {
        if let Ok(order) = order_str.parse::<u8>() {
            activation::open_recharge(order).await;
        }
        return;
    }

    match event_id {
        // Tab
        TAB_CHAT_EVENT => {
            switch_tab(MainTab::Chat);
            if state::is_activated() {
                if state::with_state(|s| s.sessions.is_empty()) {
                    chat::refresh_sessions().await;
                }
                // 模型选择器依赖供应商数据，进入聊天页时同样按需预拉
                let need_providers = state::with_state(|s| {
                    s.active_provider.is_official && s.active_provider.models.is_empty()
                });
                if need_providers {
                    chat::refresh_providers().await;
                }
            }
        }
        TAB_FILES_EVENT => {
            switch_tab(MainTab::Files);
            if state::is_activated() && state::with_state(|s| s.files_groups.is_empty()) {
                files::refresh_files().await;
            }
        }
        TAB_AI_EVENT => {
            switch_tab(MainTab::AiSettings);
            if state::is_activated() {
                let need_providers = state::with_state(|s| {
                    s.active_provider.is_official && s.active_provider.models.is_empty()
                });
                if need_providers {
                    chat::refresh_providers().await;
                }
                if state::with_state(|s| s.prompts.is_empty()) {
                    prompts::refresh_prompts().await;
                }
            }
        }
        TAB_SETTINGS_EVENT => {
            switch_tab(MainTab::Settings);
            if state::is_activated() {
                // 老版本未持久化设备信息，进入设置页时补拉
                crate::ui::activation::ensure_device_info().await;
            }
        }

        // 激活
        ACT_START_EVENT | ACT_RETRY_EVENT => activation::start_activation().await,
        ACT_OPEN_VERIFY_EVENT => activation::open_verify_url().await,
        ACT_VERIFY_PAID_EVENT => activation::verify_payment().await,

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

        // 文件管理
        FILES_REFRESH_EVENT => files::refresh_files().await,
        FILES_BACK_EVENT => files::back_to_groups(),
        FILES_DOWNLOAD_CURRENT_EVENT => files::download_preview().await,

        // AI 设置
        AI_REFRESH_EVENT => {
            chat::refresh_providers().await;
            prompts::refresh_prompts().await;
        }
        AI_PROVIDER_EVENT => {
            let label = parse_event_value(event_payload);
            chat::select_provider_by_label(&label).await;
        }
        AI_PROVIDER_NEW_EVENT => {
            chat::new_provider();
            rerender();
        }
        AI_PROVIDER_SAVE_EVENT => chat::save_provider().await,
        AI_PROVIDER_CANCEL_EVENT => {
            state::end_provider_edit();
            rerender();
        }
        AI_PROVIDER_NAME_INPUT => {
            state::set_provider_name_draft(&parse_event_value(event_payload));
        }
        AI_PROVIDER_BASEURL_INPUT => {
            state::set_provider_baseurl_draft(&parse_event_value(event_payload));
        }
        AI_PROVIDER_APIKEY_INPUT => {
            state::set_provider_apikey_draft(&parse_event_value(event_payload));
        }
        AI_MODEL_ADD_EVENT => {
            state::add_provider_model();
            rerender();
        }
        AI_PROMPT_SELECT_EVENT => {
            chat::select_prompt(&parse_event_value(event_payload)).await;
        }
        AI_PROMPT_NEW_EVENT => {
            prompts::new_prompt();
            rerender();
        }
        AI_PROMPT_SAVE_EVENT => prompts::save_prompt().await,
        AI_PROMPT_CANCEL_EVENT => {
            state::begin_prompt_edit(None, "", "");
            rerender();
        }
        AI_PROMPT_NAME_INPUT => {
            state::set_prompt_name_draft(&parse_event_value(event_payload));
        }
        AI_PROMPT_CONTENT_INPUT => {
            state::set_prompt_content_draft(&parse_event_value(event_payload));
        }

        // 设置
        SET_REFRESH_AUTH_EVENT => activation::refresh_auth_info().await,
        SET_TOGGLE_KEY_EVENT => {
            state::with_state(|s| s.api_key_visible = !s.api_key_visible);
            rerender();
        }
        SET_REVOKE_EVENT => activation::delete_local_auth().await,

        // 关于（并入设置页）
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

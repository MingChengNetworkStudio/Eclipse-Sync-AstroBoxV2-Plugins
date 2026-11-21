//! 插件全局状态：激活信息、设备列表、聊天会话/消息/供应商缓存。
//! 所有可变状态集中在 `PluginState`，通过 `OnceLock<Mutex<..>>` 访问，
//! 宿主事件回调与后台 spawn 任务都走同一把锁。

use std::sync::{Mutex, OnceLock};

use serde::Deserialize;

use crate::ui::persist;

/// Eclipse 快应用包名（通信目标固定，不支持多应用）
pub const PKG_NAME: &str = "moe.mcns.Eclipse";
/// 铭诚 API 网关地址
pub const API_BASE_URL: &str = "https://api.b4qaq.cn";
/// 官方 AI 中转 OpenAI 兼容基址
pub const RELAY_BASE_URL: &str = "https://api.b4qaq.cn/api/v2/ai/v1";
/// 官方中转供应商 id
pub const OFFICIAL_ID: &str = "official";

/// 主 Tab 枚举
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MainTab {
    Chat,
    Files,
    AiSettings,
    Settings,
}

/// 激活流程状态机
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum VerificationStatus {
    /// 未开始（欢迎页）
    #[default]
    NotStarted,
    /// 检查设备连接
    CheckingDevice,
    /// 正在向手表请求 APIKey / 设备信息
    GettingInfo,
    /// 等待用户付款或点击免费激活
    WaitingPayment,
    /// 正在向服务器校验
    Verifying,
    /// 已激活
    Verified,
    /// 激活失败（可重试）
    Failed,
}

/// 聊天页视图层级
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatView {
    /// 会话列表
    #[default]
    SessionList,
    /// 单个会话对话页
    Chat,
}

/// 供应商（字段与手表 providers.json / GET_PROVIDERS 回执对齐）
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Provider {
    pub id: String,
    pub name: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    pub models: Vec<String>,
    #[serde(rename = "activeModel")]
    pub active_model: String,
    /// 仅官方中转对象带此标记，不落盘、不参与协议序列化
    #[serde(rename = "isOfficialProvider", default)]
    pub is_official: bool,
}

impl Provider {
    /// 构造官方中转占位对象（模型列表以后续 GET_PROVIDERS 的 active 回执为准）
    pub fn official() -> Self {
        Provider {
            id: OFFICIAL_ID.to_string(),
            name: "官方中转".to_string(),
            base_url: RELAY_BASE_URL.to_string(),
            is_official: true,
            ..Default::default()
        }
    }
}

/// 插件全局状态
pub struct PluginState {
    /// 插件页面根元素 id
    pub root_element_id: Option<String>,
    pub current_tab: MainTab,

    // ----- 激活相关 -----
    pub verification_status: VerificationStatus,
    /// 激活 APIKey（空串表示未激活）
    pub api_key: String,
    /// APIKey 明文是否可见（默认脱敏）
    pub api_key_visible: bool,
    /// 主机侧设备 (name, addr)，取第一台已连接设备
    pub host_device: Option<(String, String)>,
    /// 手表 GET_DEVICEINFO 回执（激活验证用，含 product/deviceId/serial）
    pub device_info: Option<serde_json::Value>,
    /// getInfo 接口完整回执（余额、userType 等）
    pub server_info: Option<serde_json::Value>,
    /// AI 中转 /v1/usage 回执
    pub relay_usage: Option<serde_json::Value>,

    // ----- 设备列表 -----
    pub devices: Vec<(String, String)>,
    pub devices_loading: bool,

    // ----- 聊天：会话 -----
    pub chat_view: ChatView,
    /// 手表会话对象数组（原样保留，渲染时按需取字段）
    pub sessions: Vec<serde_json::Value>,
    pub sessions_loading: bool,
    /// 当前打开的会话 id
    pub current_session_id: Option<String>,

    // ----- 聊天：消息 -----
    /// 当前会话已加载的消息（按时间正序）
    pub messages: Vec<serde_json::Value>,
    /// 下一次「加载更早」使用的分块下标（0 为最新块，已加载）
    pub next_chunk: usize,
    /// 剩余未加载的更早分块数
    pub older_chunks: usize,
    pub messages_loading: bool,

    // ----- 聊天：供应商 -----
    /// 手表上的自定义供应商列表
    pub providers: Vec<Provider>,
    /// 当前生效供应商（可能是官方中转）
    pub active_provider: Provider,
    /// 本次对话选中的模型（默认取 active_provider.active_model）
    pub selected_model: String,

    // ----- 聊天：输入与流式 -----
    /// 输入框草稿
    pub draft: String,
    /// 是否正在请求模型（含流式输出中）
    pub sending: bool,
    /// 流式累积的 assistant 正文
    pub streaming_text: String,
    /// 上次流式重渲染时间戳（节流用，ms）
    pub last_stream_render_ms: u128,

    // ----- 文件管理 -----
    /// GET_FILES_GROUPED 回执：[{id, title, files:[相对路径]}]
    pub files_groups: Vec<serde_json::Value>,
    pub files_loading: bool,
    /// 文件预览：(文件名, 文本内容或错误提示)
    pub file_preview: Option<(String, String)>,

    // ----- AI 设定 -----
    /// GET_PROMPTS 回执：[{id, name, content, ...}]
    pub prompts: Vec<serde_json::Value>,
    pub prompts_loading: bool,
    /// 编辑中的设定 id（None 表示新建）
    pub prompt_edit_id: Option<String>,
    pub prompt_name_draft: String,
    pub prompt_content_draft: String,

    /// 渲染计数（调试/统计用）
    pub render_tick: u64,
}

static STATE: OnceLock<Mutex<PluginState>> = OnceLock::new();

pub fn with_state<R>(f: impl FnOnce(&mut PluginState) -> R) -> R {
    let mutex = STATE.get_or_init(|| {
        Mutex::new(PluginState {
            root_element_id: None,
            current_tab: MainTab::Chat,
            verification_status: VerificationStatus::NotStarted,
            api_key: String::new(),
            api_key_visible: false,
            host_device: None,
            device_info: None,
            server_info: None,
            relay_usage: None,
            devices: Vec::new(),
            devices_loading: false,
            chat_view: ChatView::SessionList,
            sessions: Vec::new(),
            sessions_loading: false,
            current_session_id: None,
            messages: Vec::new(),
            next_chunk: 1,
            older_chunks: 0,
            messages_loading: false,
            providers: Vec::new(),
            active_provider: Provider::official(),
            selected_model: String::new(),
            draft: String::new(),
            sending: false,
            streaming_text: String::new(),
            last_stream_render_ms: 0,
            files_groups: Vec::new(),
            files_loading: false,
            file_preview: None,
            prompts: Vec::new(),
            prompts_loading: false,
            prompt_edit_id: None,
            prompt_name_draft: String::new(),
            prompt_content_draft: String::new(),
            render_tick: 0,
        })
    });
    let mut guard = mutex.lock().unwrap_or_else(|p| p.into_inner());
    f(&mut guard)
}

// ========== 通用访问 ==========

pub fn set_root(element_id: &str) {
    with_state(|s| s.root_element_id = Some(element_id.to_string()));
}

pub fn root() -> Option<String> {
    with_state(|s| s.root_element_id.clone())
}

/// 是否已激活（本地存在 APIKey 即视为已激活，失效由 getInfo 的 201 兜底）
pub fn is_activated() -> bool {
    with_state(|s| !s.api_key.is_empty())
}

pub fn now_unix_ms() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

// ========== 激活状态 ==========

/// 写入激活 Key 并切换到已激活状态，同时落盘
pub fn set_api_key(key: &str) {
    with_state(|s| {
        s.api_key = key.to_string();
        if !key.is_empty() {
            s.verification_status = VerificationStatus::Verified;
        }
        s.render_tick = s.render_tick.wrapping_add(1);
    });
    persist_now();
}

pub fn set_verification_status(status: VerificationStatus) {
    with_state(|s| {
        s.verification_status = status;
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

pub fn set_device_info(info: Option<serde_json::Value>) {
    with_state(|s| s.device_info = info);
}

pub fn set_server_info(info: Option<serde_json::Value>) {
    with_state(|s| s.server_info = info);
}

pub fn set_relay_usage(usage: Option<serde_json::Value>) {
    with_state(|s| s.relay_usage = usage);
}

/// 清空全部激活与聊天缓存（撤销授权时使用）
pub fn clear_activation() {
    with_state(|s| {
        s.api_key.clear();
        s.api_key_visible = false;
        s.verification_status = VerificationStatus::NotStarted;
        s.device_info = None;
        s.server_info = None;
        s.relay_usage = None;
        s.sessions.clear();
        s.messages.clear();
        s.providers.clear();
        s.active_provider = Provider::official();
        s.selected_model.clear();
        s.current_session_id = None;
        s.chat_view = ChatView::SessionList;
        s.draft.clear();
        s.sending = false;
        s.streaming_text.clear();
        s.files_groups.clear();
        s.files_loading = false;
        s.file_preview = None;
        s.prompts.clear();
        s.prompts_loading = false;
        s.prompt_edit_id = None;
        s.prompt_name_draft.clear();
        s.prompt_content_draft.clear();
        s.render_tick = s.render_tick.wrapping_add(1);
    });
    persist::delete_config();
}

// ========== 设备列表 ==========

pub fn set_devices(devices: Vec<(String, String)>) {
    with_state(|s| {
        s.devices = devices;
        s.devices_loading = false;
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

pub fn devices() -> Vec<(String, String)> {
    with_state(|s| s.devices.clone())
}

/// 取第一台已连接设备地址（同 Eternal 策略，多设备不做切换器）
pub fn first_device_addr() -> Option<String> {
    with_state(|s| s.devices.first().map(|(addr, _)| addr.clone()))
}

pub fn set_devices_loading(loading: bool) {
    with_state(|s| s.devices_loading = loading);
}

pub fn is_devices_loading() -> bool {
    with_state(|s| s.devices_loading)
}

pub fn set_host_device(name: &str, addr: &str) {
    with_state(|s| s.host_device = Some((name.to_string(), addr.to_string())));
}

// ========== 聊天状态 ==========

pub fn set_chat_view(view: ChatView) {
    with_state(|s| {
        s.chat_view = view;
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

pub fn set_sessions(sessions: Vec<serde_json::Value>) {
    with_state(|s| {
        s.sessions = sessions;
        s.sessions_loading = false;
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

pub fn set_sessions_loading(loading: bool) {
    with_state(|s| s.sessions_loading = loading);
}

/// 打开会话：记录 id、写入首批消息与分块游标
pub fn open_session(
    session_id: &str,
    messages: Vec<serde_json::Value>,
    older_chunks: usize,
) {
    with_state(|s| {
        s.current_session_id = Some(session_id.to_string());
        s.messages = messages;
        s.next_chunk = 1;
        s.older_chunks = older_chunks;
        s.chat_view = ChatView::Chat;
        s.streaming_text.clear();
        s.draft.clear();
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

pub fn close_session() {
    with_state(|s| {
        s.current_session_id = None;
        s.messages.clear();
        s.older_chunks = 0;
        s.next_chunk = 1;
        s.streaming_text.clear();
        s.draft.clear();
        s.chat_view = ChatView::SessionList;
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

/// 在历史消息前面拼接更早的分块，并推进游标
pub fn prepend_older_messages(mut older: Vec<serde_json::Value>, remaining: usize) {
    with_state(|s| {
        older.append(&mut s.messages);
        s.messages = older;
        s.next_chunk += 1;
        s.older_chunks = remaining;
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

pub fn set_messages_loading(loading: bool) {
    with_state(|s| s.messages_loading = loading);
}

pub fn current_session_id() -> Option<String> {
    with_state(|s| s.current_session_id.clone())
}

/// 追加一条消息到本地缓存
pub fn push_message(msg: serde_json::Value) {
    with_state(|s| s.messages.push(msg));
}

/// 移除最后一条消息（发送失败回滚用）
pub fn pop_message() {
    with_state(|s| {
        s.messages.pop();
    });
}

// ========== 供应商 ==========

/// 写入 GET_PROVIDERS 回执：自定义列表 + 当前生效供应商
pub fn set_providers(customs: Vec<Provider>, active: Provider) {
    with_state(|s| {
        let model = if active.active_model.is_empty() {
            s.selected_model.clone()
        } else {
            active.active_model.clone()
        };
        s.providers = customs;
        s.active_provider = active;
        // 首次拉取或当前选择不在模型列表时，跟随供应商的 activeModel
        if s.selected_model.is_empty()
            || !s.active_provider.models.contains(&s.selected_model)
        {
            s.selected_model = model;
        }
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

pub fn set_active_provider(active: Provider) {
    with_state(|s| {
        // 切换供应商后模型跟随其 activeModel
        s.selected_model = active.active_model.clone();
        s.active_provider = active;
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

pub fn set_selected_model(model: &str) {
    with_state(|s| {
        s.selected_model = model.to_string();
    });
}

// ========== 输入与流式 ==========

pub fn set_draft(text: &str) {
    with_state(|s| s.draft = text.to_string());
}

pub fn take_draft() -> String {
    with_state(|s| s.draft.drain(..).collect())
}

pub fn set_sending(sending: bool) {
    with_state(|s| {
        s.sending = sending;
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

/// 流式过程中追加 delta 文本
pub fn push_stream_delta(delta: &str) {
    with_state(|s| s.streaming_text.push_str(delta));
}

pub fn take_streaming_text() -> String {
    with_state(|s| s.streaming_text.drain(..).collect())
}

/// 节流判断：距上次流式渲染超过 interval_ms 才允许重绘
pub fn stream_render_due(interval_ms: u128) -> bool {
    let now = now_unix_ms();
    with_state(|s| {
        if now.saturating_sub(s.last_stream_render_ms) >= interval_ms {
            s.last_stream_render_ms = now;
            true
        } else {
            false
        }
    })
}

// ========== 持久化 ==========

/// 启动时从磁盘恢复激活状态
pub fn restore_from_disk(config: persist::OnDiskConfig) {
    with_state(|s| {
        s.api_key = config.api_key;
        s.verification_status = if s.api_key.is_empty() {
            VerificationStatus::NotStarted
        } else {
            VerificationStatus::Verified
        };
    });
}

/// 当前状态快照落盘
pub fn persist_now() {
    let config = with_state(|s| persist::OnDiskConfig {
        version: persist::CURRENT_VERSION,
        api_key: s.api_key.clone(),
    });
    persist::save_config(&config);
}

// ========== 文件管理 ==========

pub fn set_files_groups(groups: Vec<serde_json::Value>) {
    with_state(|s| {
        s.files_groups = groups;
        s.files_loading = false;
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

pub fn set_files_loading(loading: bool) {
    with_state(|s| s.files_loading = loading);
}

pub fn set_file_preview(preview: Option<(String, String)>) {
    with_state(|s| {
        s.file_preview = preview;
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

// ========== AI 设定 ==========

pub fn set_prompts(prompts: Vec<serde_json::Value>) {
    with_state(|s| {
        s.prompts = prompts;
        s.prompts_loading = false;
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

pub fn set_prompts_loading(loading: bool) {
    with_state(|s| s.prompts_loading = loading);
}

/// 进入编辑态（id=None 为新建），并把草稿填充为该设定现有内容
pub fn begin_prompt_edit(id: Option<String>, name: &str, content: &str) {
    with_state(|s| {
        s.prompt_edit_id = id;
        s.prompt_name_draft = name.to_string();
        s.prompt_content_draft = content.to_string();
        s.render_tick = s.render_tick.wrapping_add(1);
    });
}

pub fn set_prompt_name_draft(text: &str) {
    with_state(|s| s.prompt_name_draft = text.to_string());
}

pub fn set_prompt_content_draft(text: &str) {
    with_state(|s| s.prompt_content_draft = text.to_string());
}

//! 聊天功能：会话与消息的拉取/同步、供应商切换、直连供应商的流式对话。
//!
//! 互联读写全部为事件驱动：UI 动作只负责发起请求，回执到达后由
//! `event_handler` 调用本模块的 on_*_reply 继续，避免阻塞宿主事件循环。
//! 对话本身走插件直连供应商的 SSE（WASI HTTP），不经过互联通道。

use std::sync::atomic::{AtomicBool, Ordering};

use astrobox_ng_wit::astrobox::psys_host_v4::dialog;
use serde_json::{json, Value};

use crate::ui::api_client;
use crate::ui::build::rerender;
use crate::ui::protocol::{self, PendingKind};
use crate::ui::state;

/// 流式取消标记（同一时刻只允许一个对话任务）
static CHAT_CANCEL: AtomicBool = AtomicBool::new(false);
/// 流式渲染最小间隔（ms），避免 delta 过密拖垮 UI
const STREAM_RENDER_INTERVAL_MS: u128 = 150;
/// 对话请求连接超时
const CHAT_TIMEOUT_MS: u32 = 30_000;

// ========== 消息/会话字段辅助 ==========

/// 消息角色，缺省按 assistant 处理（与手表 normalize 逻辑一致）
pub fn msg_role(msg: &Value) -> &str {
    msg.get("role").and_then(|v| v.as_str()).unwrap_or("assistant")
}

pub fn msg_content(msg: &Value) -> &str {
    msg.get("content").and_then(|v| v.as_str()).unwrap_or("")
}

pub fn session_id(session: &Value) -> String {
    session.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string()
}

pub fn session_title(session: &Value) -> String {
    session
        .get("title")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("未命名会话")
        .to_string()
}

/// 会话更新时间（ms），缺省 0
pub fn session_updated_at(session: &Value) -> i64 {
    session
        .get("updatedAt")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}

// ========== 会话列表（发起） ==========

/// 从手表拉取会话列表
pub async fn refresh_sessions() {
    state::set_sessions_loading(true);
    rerender();
    if let Err(err) = protocol::refresh_sessions().await {
        state::set_sessions_loading(false);
        rerender();
        show_alert("加载会话失败", &err).await;
    }
}

/// 新建会话（回执到达后自动打开）
pub async fn new_session() {
    if let Err(err) = protocol::create_session("").await {
        show_alert("新建会话失败", &err).await;
    }
}

/// 删除会话（二次确认）
pub async fn delete_session(idx: usize) {
    let target = state::with_state(|s| s.sessions.get(idx).cloned());
    let Some(session) = target else {
        return;
    };
    let id = session_id(&session);
    let title = session_title(&session);

    if !show_confirm("删除会话", &format!("确定删除「{}」？", title)).await {
        return;
    }
    if let Err(err) = protocol::delete_session(&id).await {
        show_alert("删除失败", &err).await;
    }
}

/// 打开会话：发起 GET_SESSION 读取最新分块（chunk=0）
pub async fn open_session(idx: usize) {
    let id = state::with_state(|s| s.sessions.get(idx).map(session_id));
    if let Some(id) = id {
        state::set_messages_loading(true);
        rerender();
        if let Err(err) = protocol::request_session(&id, 0).await {
            state::set_messages_loading(false);
            rerender();
            show_alert("打开会话失败", &err).await;
        }
    }
}

/// 加载更早的消息分块
pub async fn load_older() {
    let Some(id) = state::current_session_id() else {
        return;
    };
    if state::with_state(|s| s.older_chunks == 0 || s.messages_loading) {
        return;
    }
    let chunk = state::with_state(|s| s.next_chunk);
    state::set_messages_loading(true);
    rerender();
    if let Err(err) = protocol::request_session(&id, chunk).await {
        state::set_messages_loading(false);
        rerender();
        show_alert("加载更早消息失败", &err).await;
    }
}

/// 退回会话列表
pub fn back_to_list() {
    state::close_session();
    rerender();
}

// ========== 会话/消息回执处理 ==========

/// SESSION_LIST 回执
pub async fn on_session_list_reply(ok: bool, status: &str, data: Option<Value>) {
    state::set_sessions_loading(false);
    if !ok {
        rerender();
        show_alert("加载会话失败", status).await;
        return;
    }
    let list = data.and_then(|v| v.as_array().cloned()).unwrap_or_default();
    state::set_sessions(list);
    rerender();
}

/// PUT_SESSION_CREATE_DONE 回执：刷新列表并打开新会话
pub async fn on_session_created_reply(ok: bool, status: &str, data: Option<Value>) {
    if !ok {
        show_alert("新建会话失败", status).await;
        return;
    }
    let id = data.map(|d| session_id(&d)).unwrap_or_default();
    if id.is_empty() {
        show_alert("新建会话失败", "回执缺少会话 id").await;
        return;
    }
    refresh_sessions().await;
    state::set_messages_loading(true);
    rerender();
    if let Err(err) = protocol::request_session(&id, 0).await {
        state::set_messages_loading(false);
        rerender();
        show_alert("打开会话失败", &err).await;
    }
}

/// DEL_SESSION_DONE 回执
pub async fn on_session_deleted_reply(deleted_id: &str, ok: bool, status: &str) {
    if !ok {
        show_alert("删除失败", status).await;
        return;
    }
    let is_current = state::current_session_id().as_deref() == Some(deleted_id);
    if is_current {
        state::close_session();
    }
    refresh_sessions().await;
}

/// SESSION 回执：chunk=0 打开会话，chunk>0 前插更早消息
pub async fn on_session_reply(
    session_id: &str,
    chunk: usize,
    ok: bool,
    status: &str,
    data: Option<Value>,
) {
    state::set_messages_loading(false);
    if !ok {
        rerender();
        show_alert("读取会话失败", status).await;
        return;
    }

    let detail = data.unwrap_or(Value::Null);
    let messages = detail
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let older = detail
        .get("olderChunks")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    if chunk == 0 {
        // 打开会话时同步手表侧绑定的 AI 设定 id
        let prompt_id = detail
            .get("session")
            .and_then(|s| s.get("promptId"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        state::set_selected_prompt(&prompt_id);
        state::open_session(session_id, messages, older);
        // 聊天页的 AI 设定选择器依赖列表数据，未加载时补拉（事件驱动，不阻塞打开）
        if state::with_state(|s| s.prompts.is_empty()) {
            if let Err(err) = protocol::request_prompts().await {
                tracing::warn!("加载AI设定失败: {}", err);
            }
        }
    } else {
        state::prepend_older_messages(messages, older);
    }
    rerender();
}

/// PUT_MESSAGES_DONE 回执：同步失败时提示（本地消息仍保留）
pub async fn on_messages_put_reply(_session_id: &str, ok: bool, status: &str) {
    if !ok {
        show_alert("同步失败", &format!("写回手表失败: {}", status)).await;
    }
}

/// PUT_SESSION_UPDATE_DONE 回执：自动改名后刷新列表
pub async fn on_session_updated_reply(_session_id: &str, ok: bool, _status: &str) {
    if ok {
        let _ = protocol::refresh_sessions().await;
    }
}

// ========== 供应商 ==========

/// 拉取手表供应商列表与当前生效供应商
pub async fn refresh_providers() {
    if let Err(err) = protocol::request_providers().await {
        tracing::warn!("加载供应商失败: {}", err);
    }
}

/// 切换激活供应商（同步到手表）。`label` 为 Select 选中的显示名
pub async fn select_provider_by_label(label: &str) {
    let target_id = state::with_state(|s| {
        if label == "官方中转" {
            return state::OFFICIAL_ID.to_string();
        }
        s.providers
            .iter()
            .find(|p| p.name == label)
            .map(|p| p.id.clone())
            .unwrap_or_else(|| s.active_provider.id.clone())
    });

    if target_id == state::with_state(|s| s.active_provider.id.clone()) {
        return;
    }
    if let Err(err) = protocol::set_active_provider(&target_id).await {
        show_alert("切换供应商失败", &err).await;
    }
}

/// PROVIDERS 回执
pub fn on_providers_reply(ok: bool, data: Option<Value>) {
    if !ok {
        return;
    }
    if let Some(value) = data {
        let (customs, active) = protocol::parse_providers(&value);
        state::set_providers(customs, active);
        rerender();
    }
}

/// PUT_PROVIDER_ACTIVE_DONE 回执
pub fn on_active_provider_reply(ok: bool, data: Option<Value>) {
    if !ok {
        return;
    }
    if let Some(value) = data {
        if let Ok(active) = serde_json::from_value::<state::Provider>(value) {
            state::set_active_provider(active);
            rerender();
        }
    }
}

// ========== 供应商新增/编辑/删除 ==========

/// 进入新建供应商态
pub fn new_provider() {
    state::begin_provider_edit(None, "", "", "", &[]);
}

/// 进入编辑态：把所选自定义供应商填入草稿
pub fn edit_provider(idx: usize) {
    let target = state::with_state(|s| s.providers.get(idx).cloned());
    if let Some(p) = target {
        state::begin_provider_edit(
            Some(p.id),
            &p.name,
            &p.base_url,
            &p.api_key,
            &p.models,
        );
    }
}

/// 保存供应商草稿（校验 name/baseUrl 必填）
pub async fn save_provider() {
    let (id, name, base_url, api_key) = state::with_state(|s| {
        (
            s.provider_edit_id.clone(),
            s.provider_name_draft.trim().to_string(),
            s.provider_baseurl_draft.trim().to_string(),
            s.provider_apikey_draft.trim().to_string(),
        )
    });

    if name.is_empty() || base_url.is_empty() {
        show_alert("无法保存", "供应商名称和接口地址必填").await;
        return;
    }
    let models = state::provider_models_draft();
    let active_model = models.first().cloned().unwrap_or_default();

    let mut payload = json!({
        "name": name,
        "baseUrl": base_url,
        "apiKey": api_key,
        "models": models,
        "activeModel": active_model,
    });
    if let Some(id) = id.filter(|s| !s.is_empty()) {
        payload["id"] = json!(id);
    }

    if let Err(err) = protocol::put_provider(payload).await {
        show_alert("保存失败", &err).await;
    }
}

/// 删除自定义供应商（二次确认）
pub async fn delete_provider(idx: usize) {
    let target = state::with_state(|s| s.providers.get(idx).cloned());
    let Some(p) = target else {
        return;
    };
    if !show_confirm("删除供应商", &format!("确定删除「{}」？", p.name)).await {
        return;
    }
    if let Err(err) = protocol::delete_provider(&p.id).await {
        show_alert("删除失败", &err).await;
    }
}

/// PUT_PROVIDER_DONE 回执：退出编辑态并刷新列表
async fn on_provider_saved(ok: bool, status: &str) {
    if !ok {
        show_alert("保存失败", status).await;
        return;
    }
    state::end_provider_edit();
    refresh_providers().await;
}

/// DEL_PROVIDER_DONE 回执：刷新列表（手表可能已自动切换激活供应商）
async fn on_provider_deleted(ok: bool, status: &str) {
    if !ok {
        show_alert("删除失败", status).await;
        return;
    }
    refresh_providers().await;
}

/// 切换当前会话使用的 AI 设定。`label` 为 Select 选中的显示名，
/// 选中「不使用AI设定」时解绑。切换结果通过 PUT_SESSION_UPDATE 同步手表
pub async fn select_prompt(label: &str) {
    let (prompt_id, session_id) = state::with_state(|s| {
        let id = if label == "不使用AI设定" {
            String::new()
        } else {
            s.prompts
                .iter()
                .find(|p| {
                    p.get("name")
                        .and_then(|v| v.as_str())
                        .map(|n| n == label)
                        .unwrap_or(false)
                })
                .and_then(|p| p.get("id").and_then(|v| v.as_str()).map(str::to_string))
                .unwrap_or_default()
        };
        (id, s.current_session_id.clone())
    });

    state::set_selected_prompt(&prompt_id);

    if let Some(sid) = session_id {
        if let Err(err) = protocol::update_session(&sid, json!({ "promptId": prompt_id })).await {
            show_alert("绑定AI设定失败", &err).await;
        }
    }
    rerender();
}

// ========== 流式对话 ==========

/// 发送输入框内容并发起流式请求
pub async fn send_message() {
    // 同步收集上下文，避免持锁跨 await
    let (session_id, provider, model, text) = state::with_state(|s| {
        let text = s.draft.trim().to_string();
        (
            s.current_session_id.clone(),
            s.active_provider.clone(),
            s.selected_model.clone(),
            text,
        )
    });

    let Some(session_id) = session_id else {
        return;
    };
    if text.is_empty() || state::with_state(|s| s.sending) {
        return;
    }
    if model.is_empty() {
        show_alert("无法发送", "当前供应商没有可用模型，请先在手表端配置").await;
        return;
    }

    // 本地立即上屏用户消息
    let now = state::now_unix_ms() as u64;
    let user_msg = json!({
        "id": format!("msg_{}_{}", now, rand_suffix()),
        "role": "user",
        "content": text,
        "model": model,
        "timestamp": now,
    });
    state::push_message(user_msg.clone());
    state::set_draft("");
    state::with_state(|s| {
        s.sending = true;
        s.streaming_text.clear();
        s.last_stream_render_ms = 0;
    });
    rerender();

    // 组装鉴权头：官方中转用复合 Key，自定义供应商用其自身 Key（可为空）
    let mut headers = std::collections::HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json;charset=UTF-8".to_string());
    let bearer = if provider.is_official {
        let api_key = state::with_state(|s| s.api_key.clone());
        format!("Eclipse-{}", api_key)
    } else {
        provider.api_key.clone()
    };
    if !bearer.is_empty() {
        headers.insert("Authorization".to_string(), format!("Bearer {}", bearer));
    }

    // AI 设定内容必须在锁外取：prompt_content_by_id 内部也会 with_state，
    // 嵌套加锁会触发 std::Mutex 重入死锁（表现为宿主杀进程/闪退）
    let prompt_id = state::with_state(|s| s.selected_prompt_id.clone());
    let prompt_content = state::prompt_content_by_id(&prompt_id);

    // 历史消息只保留 role/content，与手表 chat.ux 的请求形态一致；
    // 若会话选择了 AI 设定，在最前面注入 system 消息
    let history: Vec<Value> = state::with_state(|s| {
        let mut out: Vec<Value> = Vec::new();
        if let Some(content) = prompt_content.as_deref() {
            if !content.trim().is_empty() {
                out.push(json!({ "role": "system", "content": content }));
            }
        }
        out.extend(s.messages.iter().filter(|m| {
            let role = msg_role(m);
            (role == "user" || role == "assistant") && !msg_content(m).is_empty()
        }).map(|m| json!({ "role": msg_role(m), "content": msg_content(m) })));
        out
    });

    let url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
    let body = json!({
        "model": model,
        "messages": history,
        "stream": true,
    });
    let body_bytes = match serde_json::to_vec(&body) {
        Ok(bytes) => bytes,
        Err(err) => {
            rollback_user_message(&user_msg, &text);
            show_alert("请求构造失败", &err.to_string()).await;
            return;
        }
    };

    CHAT_CANCEL.store(false, Ordering::SeqCst);

    // SSE 为阻塞式 WASI 读取，放后台任务执行，不阻塞其他事件
    astrobox_ng_wit::spawn(async move {
        let result = api_client::execute_sse(
            "POST",
            &url,
            &headers,
            Some(&body_bytes),
            CHAT_TIMEOUT_MS,
            &mut |_event, data| {
                if CHAT_CANCEL.load(Ordering::SeqCst) {
                    return false;
                }
                let line = data.trim();
                if line == "[DONE]" {
                    return false;
                }
                if let Ok(chunk) = serde_json::from_str::<Value>(line) {
                    if let Some(delta) = chunk
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|v| v.as_str())
                    {
                        if !delta.is_empty() {
                            state::push_stream_delta(delta);
                            if state::stream_render_due(STREAM_RENDER_INTERVAL_MS) {
                                rerender();
                            }
                        }
                    }
                }
                true
            },
            &|| CHAT_CANCEL.load(Ordering::SeqCst),
        );

        finalize_stream(result, &session_id, &user_msg, &text, &model).await;
    });
}

/// 流式结束后的统一收尾
async fn finalize_stream(
    result: Result<(), String>,
    session_id: &str,
    user_msg: &Value,
    text: &str,
    model: &str,
) {
    let cancelled = CHAT_CANCEL.load(Ordering::SeqCst);
    let full = state::take_streaming_text();

    // 失败/停止且没有任何产出：回滚本地用户消息并恢复草稿
    let hard_failed = result.is_err() || (cancelled && full.is_empty()) || full.is_empty();
    if hard_failed {
        let reason = match result {
            Err(err) => err,
            Ok(()) if cancelled => "已停止生成".to_string(),
            Ok(()) => "模型未返回有效内容".to_string(),
        };
        rollback_user_message(user_msg, text);
        state::set_sending(false);
        rerender();
        show_alert("对话失败", &reason).await;
        return;
    }

    // 组装 assistant 消息并落本地
    let now = state::now_unix_ms() as u64;
    let assistant_msg = json!({
        "id": format!("msg_{}_{}", now, rand_suffix()),
        "role": "assistant",
        "content": full,
        "model": model,
        "timestamp": now,
    });
    state::push_message(assistant_msg.clone());
    state::set_sending(false);
    rerender();

    // 同步两条消息到手表（发起式，回执在 on_messages_put_reply 处理）
    if let Err(err) = protocol::put_messages(session_id, vec![user_msg.clone(), assistant_msg]).await
    {
        tracing::warn!("消息同步到手表失败: {}", err);
        show_alert("同步失败", &format!("本地已保存，但写回手表失败: {}", err)).await;
    }

    // 刷新列表以更新 updatedAt 排序（不修改会话标题，命名由手表端负责）
    refresh_sessions().await;
}

/// 停止当前流式生成
pub fn stop_streaming() {
    CHAT_CANCEL.store(true, Ordering::SeqCst);
}

/// 失败回滚：移除本地用户消息，把草稿恢复为原文
fn rollback_user_message(user_msg: &Value, text: &str) {
    let last_is_user = state::with_state(|s| {
        s.messages
            .last()
            .map(|m| m.get("id") == user_msg.get("id"))
            .unwrap_or(false)
    });
    if last_is_user {
        state::pop_message();
    }
    state::set_draft(text);
}

/// 生成消息 id 的随机后缀（避免同毫秒冲突）
fn rand_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{:x}", nanos % 0x10000)
}

// ========== 回执分发入口（event_handler 调用） ==========

/// 按 pending 类型分发互联回执到对应处理函数
pub async fn dispatch_reply(kind: PendingKind, ok: bool, status: &str, data: Option<Value>) {
    match kind {
        PendingKind::SessionList => on_session_list_reply(ok, status, data).await,
        PendingKind::CreateSession => on_session_created_reply(ok, status, data).await,
        PendingKind::DeleteSession(id) => on_session_deleted_reply(&id, ok, status).await,
        PendingKind::OpenSession(id, chunk) => {
            on_session_reply(&id, chunk, ok, status, data).await
        }
        PendingKind::PutMessages(id) => on_messages_put_reply(&id, ok, status).await,
        PendingKind::UpdateSession(id) => on_session_updated_reply(&id, ok, status).await,
        PendingKind::Providers => on_providers_reply(ok, data),
        PendingKind::SetActiveProvider => on_active_provider_reply(ok, data),
        PendingKind::FilesGrouped => {
            crate::ui::files::on_files_reply(ok, status, data).await
        }
        PendingKind::OpenFile(sid, name, action) => {
            crate::ui::files::on_file_reply(&sid, &name, action, ok, status, data).await
        }
        PendingKind::Prompts => crate::ui::prompts::on_prompts_reply(ok, data),
        PendingKind::PutPrompt => {
            crate::ui::prompts::on_prompt_saved(ok, status).await
        }
        PendingKind::DelPrompt(id) => {
            crate::ui::prompts::on_prompt_deleted(&id, ok, status).await
        }
        PendingKind::PutProvider => on_provider_saved(ok, status).await,
        PendingKind::DelProvider => on_provider_deleted(ok, status).await,
        // 激活相关与纯记录类回执不在此处理
        PendingKind::ActApikey
        | PendingKind::ActDeviceInfo
        | PendingKind::PutSettings => {}
    }
}

// ========== 弹窗 ==========

async fn show_alert(title: &str, message: &str) {
    let _ = dialog::show_dialog(
        dialog::DialogType::Alert,
        dialog::DialogStyle::Website,
        dialog::DialogInfo {
            title: title.to_string(),
            content: message.to_string(),
            buttons: vec![dialog::DialogButton {
                id: "ok".to_string(),
                primary: true,
                content: "确定".to_string(),
            }],
        },
    )
    .await;
}

async fn show_confirm(title: &str, message: &str) -> bool {
    let result = dialog::show_dialog(
        dialog::DialogType::Alert,
        dialog::DialogStyle::Website,
        dialog::DialogInfo {
            title: title.to_string(),
            content: message.to_string(),
            buttons: vec![
                dialog::DialogButton {
                    id: "cancel".to_string(),
                    primary: false,
                    content: "取消".to_string(),
                },
                dialog::DialogButton {
                    id: "ok".to_string(),
                    primary: true,
                    content: "确定".to_string(),
                },
            ],
        },
    )
    .await;
    result.clicked_btn_id == "ok"
}

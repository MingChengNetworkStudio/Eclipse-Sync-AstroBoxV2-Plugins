//! 聊天功能：会话与消息的拉取/同步、供应商切换、直连供应商的流式对话。
//!
//! 对话路径：插件直连供应商 OpenAI 兼容 `/chat/completions`（stream=true），
//! delta 实时写入 state 并节流渲染；完成后用 PUT_MESSAGES 把两条消息写回手表。

use std::sync::atomic::{AtomicBool, Ordering};

use astrobox_ng_wit::astrobox::psys_host_v4::dialog;
use serde_json::{json, Value};

use crate::ui::api_client;
use crate::ui::build::rerender;
use crate::ui::protocol;
use crate::ui::state;

/// 流式取消标记（同一时刻只允许一个对话任务）
static CHAT_CANCEL: AtomicBool = AtomicBool::new(false);
/// 流式渲染最小间隔（ms），避免 delta 过密拖垮手表 UI
const STREAM_RENDER_INTERVAL_MS: u128 = 150;
/// 对话请求连接超时
const CHAT_TIMEOUT_MS: u32 = 30_000;
/// 自动生成会话标题的最大长度
const AUTO_TITLE_LEN: usize = 12;

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

// ========== 会话列表 ==========

/// 从手表拉取会话列表
pub async fn refresh_sessions() {
    state::set_sessions_loading(true);
    rerender();
    match protocol::get_session_list().await {
        Ok(list) => state::set_sessions(list),
        Err(err) => {
            state::set_sessions_loading(false);
            rerender();
            show_alert("加载会话失败", &err).await;
        }
    }
    rerender();
}

/// 新建会话并直接打开
pub async fn new_session() {
    match protocol::create_session("").await {
        Ok(session) => {
            let id = session_id(&session);
            refresh_sessions().await;
            open_session_by_id(&id).await;
        }
        Err(err) => show_alert("新建会话失败", &err).await,
    }
}

/// 删除会话（二次确认）。若删除的是当前打开的会话，自动退回列表
pub async fn delete_session(idx: usize) {
    let target = state::with_state(|s| s.sessions.get(idx).cloned());
    let Some(session) = target else {
        return;
    };
    let id = session_id(&session);
    let title = session_title(&session);

    let confirmed = show_confirm("删除会话", &format!("确定删除「{}」？", title)).await;
    if !confirmed {
        return;
    }

    match protocol::delete_session(&id).await {
        Ok(()) => {
            let is_current = state::current_session_id().as_deref() == Some(id.as_str());
            if is_current {
                state::close_session();
            }
            refresh_sessions().await;
        }
        Err(err) => show_alert("删除失败", &err).await,
    }
}

/// 打开会话：读取最新分块（chunk=0，约 20 条）
pub async fn open_session(idx: usize) {
    let id = state::with_state(|s| s.sessions.get(idx).map(session_id));
    if let Some(id) = id {
        open_session_by_id(&id).await;
    }
}

async fn open_session_by_id(id: &str) {
    state::set_messages_loading(true);
    rerender();

    match protocol::get_session(id, 0).await {
        Ok(detail) => {
            let messages = detail
                .get("messages")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let older = detail
                .get("olderChunks")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            state::set_messages_loading(false);
            state::open_session(id, messages, older);
            rerender();
        }
        Err(err) => {
            state::set_messages_loading(false);
            rerender();
            show_alert("打开会话失败", &err).await;
        }
    }
}

/// 加载更早的消息分块（chunk 下标在 state 中维护）
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

    match protocol::get_session(&id, chunk).await {
        Ok(detail) => {
            let older_messages = detail
                .get("messages")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let remaining = detail
                .get("olderChunks")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            state::set_messages_loading(false);
            state::prepend_older_messages(older_messages, remaining);
            rerender();
        }
        Err(err) => {
            state::set_messages_loading(false);
            rerender();
            show_alert("加载更早消息失败", &err).await;
        }
    }
}

/// 退回会话列表
pub fn back_to_list() {
    state::close_session();
    rerender();
}

// ========== 供应商 ==========

/// 拉取手表供应商列表与当前生效供应商
pub async fn refresh_providers() {
    match protocol::get_providers().await {
        Ok((customs, active)) => state::set_providers(customs, active),
        Err(err) => tracing::warn!("加载供应商失败: {}", err),
    }
    rerender();
}

/// 切换激活供应商（同步到手表）。`label` 为 Select 选中的显示名
pub async fn select_provider_by_label(label: &str) {
    let target_id = state::with_state(|s| {
        if label == "官方中转" {
            return "official".to_string();
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

    match protocol::set_active_provider(&target_id).await {
        Ok(active) => state::set_active_provider(active),
        Err(err) => show_alert("切换供应商失败", &err).await,
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
        "id": format!("msg_{}_{}", now, &rand_suffix()),
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

    // 历史消息只保留 role/content，与手表 chat.ux 的请求形态一致
    let history: Vec<Value> = state::with_state(|s| {
        s.messages
            .iter()
            .filter(|m| {
                let role = msg_role(m);
                (role == "user" || role == "assistant") && !msg_content(m).is_empty()
            })
            .map(|m| json!({ "role": msg_role(m), "content": msg_content(m) }))
            .collect()
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
        "id": format!("msg_{}_{}", now, &rand_suffix()),
        "role": "assistant",
        "content": full,
        "model": model,
        "timestamp": now,
    });
    state::push_message(assistant_msg.clone());
    state::set_sending(false);
    rerender();

    // 同步两条消息到手表（手表会补齐缺失字段并按时间排序落盘）
    if let Err(err) = protocol::put_messages(session_id, vec![user_msg.clone(), assistant_msg]).await
    {
        tracing::warn!("消息同步到手表失败: {}", err);
        show_alert("同步失败", &format!("本地已保存，但写回手表失败: {}", err)).await;
    }

    // 首轮对话自动用首条用户消息给会话改名（手表默认标题为「新会话」）
    maybe_auto_rename(session_id, text).await;

    // 刷新列表以更新 updatedAt 排序
    protocol::get_session_list()
        .await
        .map(state::set_sessions)
        .ok();
    rerender();
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

/// 会话标题仍是默认值时，用首条用户消息前 12 个字自动改名
async fn maybe_auto_rename(sid: &str, first_text: &str) {
    let need_rename = state::with_state(|s| {
        s.sessions
            .iter()
            .find(|sess| session_id(sess) == sid)
            .map(|sess| {
                let title = session_title(sess);
                title.is_empty() || title == "新会话" || title == "未命名会话"
            })
            .unwrap_or(false)
    });
    if !need_rename {
        return;
    }
    let title: String = first_text.chars().take(AUTO_TITLE_LEN).collect();
    if title.is_empty() {
        return;
    }
    if let Err(err) = protocol::update_session(sid, json!({ "title": title })).await {
        tracing::warn!("自动改名失败: {}", err);
    }
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

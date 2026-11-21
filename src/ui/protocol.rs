//! Eclipse 互联协议客户端：发起 `{type,data}` 请求，回执由事件循环驱动。
//!
//! 设计与永昼天气同步器一致：**发送方绝不阻塞等待回执**。
//! 发送前在 PENDING 表登记请求类型与上下文，回执到达后由
//! `event_handler` 取出并分发给 activation/chat 的处理函数，
//! 避免阻塞宿主事件派发导致回执永远无法进入插件。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use astrobox_ng_wit::astrobox::psys_host_v4::{interconnect, register, thirdpartyapp};
use serde_json::{json, Value};

use crate::ui::state::{self, Provider, PKG_NAME};

/// 拉起快应用后，该时间窗内不再重复 launch/sleep（毫秒）
const LAUNCH_CACHE_MS: u128 = 10_000;
/// 拉起快应用后等待其就绪的时间
const LAUNCH_READY_WAIT_MS: u64 = 2_000;

/// 待处理请求的类型与上下文，回执到达后据此继续流程
pub enum PendingKind {
    /// 激活：GET_APIKEY 的回执
    ActApikey,
    /// 激活：GET_DEVICEINFO 的回执
    ActDeviceInfo,
    /// PUT_SETTINGS 回执（仅记录）
    PutSettings,
    /// GET_SESSION_LIST 回执
    SessionList,
    /// PUT_SESSION_CREATE 回执
    CreateSession,
    /// DEL_SESSION 回执，携带被删会话 id
    DeleteSession(String),
    /// GET_SESSION 回执，携带会话 id 与分块下标（0=打开会话，>0=加载更早）
    OpenSession(String, usize),
    /// PUT_MESSAGES 回执，携带会话 id
    PutMessages(String),
    /// GET_PROVIDERS 回执
    Providers,
    /// PUT_PROVIDER_ACTIVE 回执
    SetActiveProvider,
    /// PUT_SESSION_UPDATE 回执，携带会话 id
    UpdateSession(String),
}

struct PendingEntry {
    kind: PendingKind,
    /// 登记时间（ms），用于诊断长时间未兑现的请求
    _created_ms: u128,
}

/// 键：`设备地址|回执类型`
static PENDING: OnceLock<Mutex<HashMap<String, PendingEntry>>> = OnceLock::new();
/// 各设备最近一次成功拉起快应用的时间（ms），避免连续请求时重复 sleep
static LAST_LAUNCH: OnceLock<Mutex<HashMap<String, u128>>> = OnceLock::new();

fn pending() -> &'static Mutex<HashMap<String, PendingEntry>> {
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn last_launch() -> &'static Mutex<HashMap<String, u128>> {
    LAST_LAUNCH.get_or_init(|| Mutex::new(HashMap::new()))
}

fn slot_key(addr: &str, reply_type: &str) -> String {
    format!("{}|{}", addr, reply_type)
}

/// 登记一条待处理请求（必须在实际发送前调用，防止回执极快到达时丢失）
fn put_pending(addr: &str, reply_type: &str, kind: PendingKind) {
    pending()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(
            slot_key(addr, reply_type),
            PendingEntry {
                kind,
                _created_ms: state::now_unix_ms(),
            },
        );
}

/// 回执到达时取出待处理请求；无人等待返回 None
pub fn take_pending(addr: &str, reply_type: &str) -> Option<PendingKind> {
    pending()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&slot_key(addr, reply_type))
        .map(|entry| entry.kind)
}

/// 拉起 Eclipse 快应用；短时间内已拉过则跳过（launch 本身幂等）
async fn ensure_launched(addr: &str) -> Result<(), String> {
    let now = state::now_unix_ms();
    let recently_launched = last_launch()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(addr)
        .map(|t| now.saturating_sub(*t) < LAUNCH_CACHE_MS)
        .unwrap_or(false);
    if recently_launched {
        return Ok(());
    }

    let app_list = thirdpartyapp::get_thirdparty_app_list(addr.to_string())
        .await
        .map_err(|reason| format!("获取设备应用列表失败: {}", reason))?;
    let app = app_list
        .iter()
        .find(|a| a.package_name == PKG_NAME)
        .ok_or_else(|| "设备上未安装 Eclipse 快应用".to_string())?;

    thirdpartyapp::launch_qa(addr.to_string(), app.clone(), "/index".to_string())
        .await
        .map_err(|reason| format!("启动 Eclipse 失败: {}", reason))?;
    std::thread::sleep(Duration::from_millis(LAUNCH_READY_WAIT_MS));

    last_launch()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(addr.to_string(), state::now_unix_ms());
    Ok(())
}

/// 发起一条协议请求：拉起应用 → 登记 pending → 注册接收器 → 发送。
/// 本函数只等待「发送动作」完成，不等待业务回执，回执走事件循环。
async fn send(
    addr: &str,
    req_type: &str,
    reply_type: &str,
    data: Option<Value>,
    kind: PendingKind,
) -> Result<(), String> {
    ensure_launched(addr).await?;

    // 先登记再注册/发送，杜绝回执早于登记到达的竞态
    put_pending(addr, reply_type, kind);

    if let Err(reason) =
        register::register_interconnect_recv(addr.to_string(), PKG_NAME.to_string()).await
    {
        tracing::warn!("注册互联接收器失败: addr={} reason={}", addr, reason);
    }

    let payload = match data {
        Some(d) => json!({ "type": req_type, "data": d }),
        None => json!({ "type": req_type }),
    };
    if let Err(reason) = interconnect::send_qaic_message(
        addr.to_string(),
        PKG_NAME.to_string(),
        payload.to_string(),
    )
    .await
    {
        // 发送失败要撤掉登记，避免残留 pending 吞掉后续同类型回执
        take_pending(addr, reply_type);
        return Err(format!("发送消息失败: {}", reason));
    }
    Ok(())
}

/// 取第一台已连接设备地址，没有设备时返回友好错误
async fn require_addr() -> Result<String, String> {
    state::first_device_addr().ok_or_else(|| {
        "没有已连接的设备，请先在 AstroBox 中连接手表".to_string()
    })
}

// ========== 通用指令 ==========

/// 发起 GET_APIKEY，回执 APIKEY 由激活模块处理
pub async fn request_apikey() -> Result<(), String> {
    let addr = require_addr().await?;
    send(&addr, "GET_APIKEY", "APIKEY", None, PendingKind::ActApikey).await
}

/// 发起 GET_DEVICEINFO，回执 DEVICEINFO 由激活模块处理
pub async fn request_device_info() -> Result<(), String> {
    let addr = require_addr().await?;
    send(
        &addr,
        "GET_DEVICEINFO",
        "DEVICEINFO",
        None,
        PendingKind::ActDeviceInfo,
    )
    .await
}

/// 发起 PUT_SETTINGS（如下发 APIKey），回执仅记录
pub async fn put_settings(settings: Value) -> Result<(), String> {
    let addr = require_addr().await?;
    send(
        &addr,
        "PUT_SETTINGS",
        "PUT_SETTINGS_DONE",
        Some(settings),
        PendingKind::PutSettings,
    )
    .await
}

// ========== 会话管理 ==========

/// 发起 GET_SESSION_LIST
pub async fn refresh_sessions() -> Result<(), String> {
    let addr = require_addr().await?;
    send(
        &addr,
        "GET_SESSION_LIST",
        "SESSION_LIST",
        None,
        PendingKind::SessionList,
    )
    .await
}

/// 发起 PUT_SESSION_CREATE
pub async fn create_session(title: &str) -> Result<(), String> {
    let addr = require_addr().await?;
    let data = if title.is_empty() {
        json!({})
    } else {
        json!({ "title": title })
    };
    send(
        &addr,
        "PUT_SESSION_CREATE",
        "PUT_SESSION_CREATE_DONE",
        Some(data),
        PendingKind::CreateSession,
    )
    .await
}

/// 发起 DEL_SESSION
pub async fn delete_session(session_id: &str) -> Result<(), String> {
    let addr = require_addr().await?;
    send(
        &addr,
        "DEL_SESSION",
        "DEL_SESSION_DONE",
        Some(json!({ "id": session_id })),
        PendingKind::DeleteSession(session_id.to_string()),
    )
    .await
}

/// 发起 PUT_SESSION_UPDATE
pub async fn update_session(session_id: &str, patch: Value) -> Result<(), String> {
    let addr = require_addr().await?;
    send(
        &addr,
        "PUT_SESSION_UPDATE",
        "PUT_SESSION_UPDATE_DONE",
        Some(json!({ "id": session_id, "patch": patch })),
        PendingKind::UpdateSession(session_id.to_string()),
    )
    .await
}

/// 发起 GET_SESSION。chunk=0 表示打开会话，>0 表示加载更早分块
pub async fn request_session(session_id: &str, chunk: usize) -> Result<(), String> {
    let addr = require_addr().await?;
    send(
        &addr,
        "GET_SESSION",
        "SESSION",
        Some(json!({ "id": session_id, "chunk": chunk })),
        PendingKind::OpenSession(session_id.to_string(), chunk),
    )
    .await
}

// ========== 消息管理 ==========

/// 发起 PUT_MESSAGES，批量追加消息到手表
pub async fn put_messages(session_id: &str, messages: Vec<Value>) -> Result<(), String> {
    let addr = require_addr().await?;
    send(
        &addr,
        "PUT_MESSAGES",
        "PUT_MESSAGES_DONE",
        Some(json!({ "id": session_id, "messages": messages })),
        PendingKind::PutMessages(session_id.to_string()),
    )
    .await
}

// ========== 供应商管理 ==========

/// 发起 GET_PROVIDERS
pub async fn request_providers() -> Result<(), String> {
    let addr = require_addr().await?;
    send(
        &addr,
        "GET_PROVIDERS",
        "PROVIDERS",
        None,
        PendingKind::Providers,
    )
    .await
}

/// 发起 PUT_PROVIDER_ACTIVE
pub async fn set_active_provider(provider_id: &str) -> Result<(), String> {
    let addr = require_addr().await?;
    send(
        &addr,
        "PUT_PROVIDER_ACTIVE",
        "PUT_PROVIDER_ACTIVE_DONE",
        Some(json!({ "id": provider_id })),
        PendingKind::SetActiveProvider,
    )
    .await
}

/// 解析 GET_PROVIDERS 回执为 (自定义列表, 当前生效供应商)
pub fn parse_providers(value: &Value) -> (Vec<Provider>, Provider) {
    let customs = value
        .get("list")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value::<Provider>(item.clone()).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // active 可能是官方中转（id=official）或自定义供应商；解析失败回落官方占位
    let active = value
        .get("active")
        .and_then(|v| serde_json::from_value::<Provider>(v.clone()).ok())
        .unwrap_or_else(Provider::official);

    (customs, active)
}

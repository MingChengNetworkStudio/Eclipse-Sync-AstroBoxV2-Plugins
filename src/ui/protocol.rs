//! Eclipse 互联协议客户端：负责拉起快应用、发送 `{type,data}` 请求并等待对应回执。
//!
//! 回执关联方式：以 `(设备地址, 回执类型)` 为键登记等待槽，
//! `event_handler` 收到消息后调用 [`resolve`] 兑现；调用方用 Condvar 等待结果。

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

use astrobox_ng_wit::astrobox::psys_host_v4::{interconnect, register, thirdpartyapp};
use serde_json::{json, Value};

use crate::ui::state::{self, Provider, PKG_NAME};

/// 单次请求总超时
const REQUEST_TIMEOUT_MS: u64 = 15_000;
/// 拉起快应用后等待其就绪的时间
const LAUNCH_READY_WAIT_MS: u64 = 2_000;

/// 等待槽：None 表示尚未兑现，Some 表示已收到回执（成功或错误）
type Slot = Arc<(Mutex<Option<Result<Value, String>>>, Condvar)>;

static PENDING: OnceLock<Mutex<HashMap<String, Slot>>> = OnceLock::new();

fn pending() -> &'static Mutex<HashMap<String, Slot>> {
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn slot_key(addr: &str, reply_type: &str) -> String {
    format!("{}|{}", addr, reply_type)
}

/// 兑现一条回执。返回 true 表示有等待者消费了该消息，false 表示无人等待（忽略）。
pub fn resolve(addr: &str, reply_type: &str, status: &str, data: Option<Value>) -> bool {
    let slot = pending()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&slot_key(addr, reply_type));
    let Some(slot) = slot else {
        return false;
    };
    let (lock, cvar) = &*slot;
    let result = if status.is_empty() || status == "OK" {
        Ok(data.unwrap_or(Value::Null))
    } else {
        Err(status.to_string())
    };
    *lock.lock().unwrap_or_else(|p| p.into_inner()) = Some(result);
    cvar.notify_all();
    true
}

/// 拉起 Eclipse 快应用并确认已安装（已在前台时 launch 仍会成功）
async fn ensure_launched(addr: &str) -> Result<(), String> {
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
    Ok(())
}

/// 发送一条协议请求并等待回执。
/// - `req_type`：请求 type（如 GET_SESSION_LIST）
/// - `reply_type`：回执 type（如 SESSION_LIST）
/// - `data`：可选请求 data
async fn request(
    addr: &str,
    req_type: &str,
    reply_type: &str,
    data: Option<Value>,
) -> Result<Value, String> {
    ensure_launched(addr).await?;

    // 登记等待槽
    let slot: Slot = Arc::new((Mutex::new(None), Condvar::new()));
    {
        let mut map = pending().lock().unwrap_or_else(|p| p.into_inner());
        // 同键旧请求直接替换（正常流程为串行，不会出现并发同键）
        map.insert(slot_key(addr, reply_type), slot.clone());
    }

    // 注册接收器，失败仅告警（可能此前已注册）
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
        pending()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&slot_key(addr, reply_type));
        return Err(format!("发送消息失败: {}", reason));
    }

    // Condvar 等待回执，整体超时
    let deadline = Duration::from_millis(REQUEST_TIMEOUT_MS);
    let (lock, cvar) = &*slot;
    let mut guard = lock.lock().unwrap_or_else(|p| p.into_inner());
    let mut waited = Duration::ZERO;
    while guard.is_none() {
        let (g, _timeout) = cvar
            .wait_timeout(guard, Duration::from_millis(200))
            .unwrap_or_else(|p| p.into_inner());
        guard = g;
        waited += Duration::from_millis(200);
        if waited >= deadline {
            break;
        }
    }

    match guard.take() {
        Some(result) => result,
        None => {
            pending()
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&slot_key(addr, reply_type));
            Err(format!("等待 {} 回执超时", reply_type))
        }
    }
}

/// 取第一台已连接设备地址，没有设备时返回友好错误
async fn require_addr() -> Result<String, String> {
    state::first_device_addr().ok_or_else(|| "没有已连接的设备，请先在 AstroBox 中连接手表".to_string())
}

// ========== 通用指令 ==========

/// GET_APIKEY → 设备上保存的 APIKey 字符串（可能为空串）
pub async fn get_apikey() -> Result<String, String> {
    let addr = require_addr().await?;
    let value = request(&addr, "GET_APIKEY", "APIKEY", None).await?;
    Ok(value.as_str().unwrap_or("").to_string())
}

/// GET_DEVICEINFO → 设备信息对象（product/deviceId/serial 等）
pub async fn get_device_info() -> Result<Value, String> {
    let addr = require_addr().await?;
    request(&addr, "GET_DEVICEINFO", "DEVICEINFO", None).await
}

/// PUT_SETTINGS：浅合并写入手表设置（如下发 APIKey）
pub async fn put_settings(settings: Value) -> Result<(), String> {
    let addr = require_addr().await?;
    request(&addr, "PUT_SETTINGS", "PUT_SETTINGS_DONE", Some(settings)).await?;
    Ok(())
}

// ========== 会话管理 ==========

/// GET_SESSION_LIST → 会话对象数组
pub async fn get_session_list() -> Result<Vec<Value>, String> {
    let addr = require_addr().await?;
    let value = request(&addr, "GET_SESSION_LIST", "SESSION_LIST", None).await?;
    Ok(value.as_array().cloned().unwrap_or_default())
}

/// PUT_SESSION_CREATE → 新建的会话对象
pub async fn create_session(title: &str) -> Result<Value, String> {
    let addr = require_addr().await?;
    let data = if title.is_empty() {
        json!({})
    } else {
        json!({ "title": title })
    };
    request(&addr, "PUT_SESSION_CREATE", "PUT_SESSION_CREATE_DONE", Some(data)).await
}

/// DEL_SESSION：删除会话及其消息分块、工作区
pub async fn delete_session(session_id: &str) -> Result<(), String> {
    let addr = require_addr().await?;
    request(
        &addr,
        "DEL_SESSION",
        "DEL_SESSION_DONE",
        Some(json!({ "id": session_id })),
    )
    .await?;
    Ok(())
}

/// PUT_SESSION_UPDATE：更新会话字段（title/webEnabled/searchMode/memory/promptId）
pub async fn update_session(session_id: &str, patch: Value) -> Result<(), String> {
    let addr = require_addr().await?;
    request(
        &addr,
        "PUT_SESSION_UPDATE",
        "PUT_SESSION_UPDATE_DONE",
        Some(json!({ "id": session_id, "patch": patch })),
    )
    .await?;
    Ok(())
}

/// GET_SESSION：取某个分块的会话详情与消息
/// 返回 `{session, messages, chunk, totalChunks, olderChunks}`
pub async fn get_session(session_id: &str, chunk: usize) -> Result<Value, String> {
    let addr = require_addr().await?;
    request(
        &addr,
        "GET_SESSION",
        "SESSION",
        Some(json!({ "id": session_id, "chunk": chunk })),
    )
    .await
}

// ========== 消息管理 ==========

/// PUT_MESSAGES：批量追加消息（手表自动补 id/role/timestamp，同 id 覆盖）
pub async fn put_messages(session_id: &str, messages: Vec<Value>) -> Result<(), String> {
    let addr = require_addr().await?;
    request(
        &addr,
        "PUT_MESSAGES",
        "PUT_MESSAGES_DONE",
        Some(json!({ "id": session_id, "messages": messages })),
    )
    .await?;
    Ok(())
}

// ========== 供应商管理 ==========

/// GET_PROVIDERS → (自定义供应商列表, 当前生效供应商)
pub async fn get_providers() -> Result<(Vec<Provider>, Provider), String> {
    let addr = require_addr().await?;
    let value = request(&addr, "GET_PROVIDERS", "PROVIDERS", None).await?;

    let customs = value
        .get("list")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value::<Provider>(item.clone()).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // active 可能是官方中转（id=official）或自定义供应商；解析失败时回落官方占位
    let active = value
        .get("active")
        .and_then(|v| serde_json::from_value::<Provider>(v.clone()).ok())
        .unwrap_or_else(Provider::official);

    Ok((customs, active))
}

/// PUT_PROVIDER_ACTIVE：切换激活供应商，返回切换后的生效对象
pub async fn set_active_provider(provider_id: &str) -> Result<Provider, String> {
    let addr = require_addr().await?;
    let value = request(
        &addr,
        "PUT_PROVIDER_ACTIVE",
        "PUT_PROVIDER_ACTIVE_DONE",
        Some(json!({ "id": provider_id })),
    )
    .await?;
    Ok(serde_json::from_value::<Provider>(value).unwrap_or_else(|_| Provider::official()))
}

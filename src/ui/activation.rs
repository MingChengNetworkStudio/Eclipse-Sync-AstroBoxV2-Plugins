//! 激活流程：从手表读取 Key/设备信息 → 铭诚 API verifyCheck 校验 → 下发 Key 回手表。
//!
//! 与永昼天气同步器同款事件驱动模型：发起互联请求后立即返回，
//! 回执由 `event_handler` 分发到本模块的 on_*_reply 继续状态机，
//! 绝不在事件任务里阻塞等待互联回执。

use std::collections::HashMap;

use astrobox_ng_wit::astrobox::psys_host_v4::dialog;
use serde_json::json;
use urlencoding::encode;

use crate::ui::api_client;
use crate::ui::build::rerender;
use crate::ui::protocol;
use crate::ui::state::{self, VerificationStatus, API_BASE_URL, RELAY_BASE_URL};

/// 激活入口：记录设备后发起 GET_APIKEY，回执到达时继续
pub async fn start_activation() {
    state::set_verification_status(VerificationStatus::CheckingDevice);
    rerender();

    // 记录第一台设备供激活页展示
    let Some((name, addr)) = state::devices().into_iter().next() else {
        state::set_verification_status(VerificationStatus::Failed);
        rerender();
        show_alert("未连接设备", "请先在 AstroBox 中连接手表后重试").await;
        return;
    };
    state::set_host_device(&name, &addr);

    state::set_verification_status(VerificationStatus::GettingInfo);
    rerender();

    if let Err(err) = protocol::request_apikey().await {
        state::set_verification_status(VerificationStatus::Failed);
        rerender();
        show_alert("激活失败", &err).await;
    }
}

/// GET_APIKEY 回执处理
pub async fn on_apikey_reply(ok: bool, status: &str, data: Option<serde_json::Value>) {
    if !ok {
        tracing::warn!("GET_APIKEY 回执失败: {}", status);
        // 手表报错时仍尝试走设备信息激活流程
        request_device_info().await;
        return;
    }

    let key = data
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();

    if key.trim().is_empty() {
        // 手表没有 Key，进入设备信息 + 付款/免费激活流程
        request_device_info().await;
        return;
    }

    // 手表上已有 Key（如之前在快应用内激活过），询问是否直接采用
    let masked = mask_api_key(key.trim());
    let use_key = show_confirm(
        "检测到设备Key",
        &format!("设备上已存在授权 Key：\n{}\n\n是否使用该 Key？", masked),
        "重新验证",
        "使用",
    )
    .await;
    if use_key {
        activate_with_key(key.trim()).await;
    } else {
        request_device_info().await;
    }
}

/// 发起 GET_DEVICEINFO
async fn request_device_info() {
    state::set_verification_status(VerificationStatus::GettingInfo);
    rerender();

    if let Err(err) = protocol::request_device_info().await {
        state::set_verification_status(VerificationStatus::Failed);
        rerender();
        show_alert("获取设备信息失败", &err).await;
    }
}

/// GET_DEVICEINFO 回执处理
pub async fn on_device_info_reply(ok: bool, status: &str, data: Option<serde_json::Value>) {
    if !ok {
        state::set_verification_status(VerificationStatus::Failed);
        rerender();
        show_alert("获取设备信息失败", status).await;
        return;
    }

    state::set_device_info(data);
    state::set_verification_status(VerificationStatus::WaitingPayment);
    rerender();
}

/// 打开浏览器进入支付/激活页面
pub async fn open_verify_url() {
    let Some(url) = build_verify_url("/verify/Eclipse") else {
        show_alert("错误", "设备信息缺失，请重新进入激活流程").await;
        return;
    };
    dialog::open_url(&url);
}

/// 校验激活结果。`free=true` 走免费版通道，否则走付费校验
pub async fn verify_payment(free: bool) {
    state::set_verification_status(VerificationStatus::Verifying);
    rerender();

    let check_path = if free {
        "/verifyCheck/Eclipse?type=free"
    } else {
        "/verifyCheck/Eclipse?type=normal"
    };
    let Some(url) = build_verify_url(check_path) else {
        state::set_verification_status(VerificationStatus::Failed);
        rerender();
        show_alert("错误", "设备信息缺失，请重新进入激活流程").await;
        return;
    };

    // verifyCheck 为插件自身发起的无鉴权 HTTP，可直接 await（WASI 调用会驱动事件循环）
    let headers = HashMap::new();
    match api_client::get_json(&url, &headers) {
        Ok(body) => {
            if body.get("status").and_then(|v| v.as_i64()) == Some(200) {
                let api_key = body
                    .get("result")
                    .and_then(|r| r.get("APIKey"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if api_key.is_empty() {
                    verification_failed("返回数据缺少 APIKey").await;
                    return;
                }
                // 签名校验在手表端进行，插件侧与 Eternal 同步器一样直接采信
                activate_with_key(api_key).await;
            } else {
                let msg = body
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("未知错误");
                verification_failed(&format!("验证失败: {}", msg)).await;
            }
        }
        Err(err) => verification_failed(&format!("请求失败: {}", err)).await,
    }
}

/// 激活成功后的统一收尾：本地存盘 → 下发手表 → 拉取授权信息
async fn activate_with_key(api_key: &str) {
    state::set_api_key(api_key);

    // 把 Key 写回手表设置（发起后不等回执，PUT_SETTINGS_DONE 仅记录）
    if let Err(err) = protocol::put_settings(json!({ "APIKey": api_key })).await {
        tracing::warn!("下发 APIKey 到手表失败: {}", err);
    }

    state::set_verification_status(VerificationStatus::Verified);
    rerender();
    show_alert("成功", "验证成功！").await;

    // 后台拉取余额与 AI 用量，网络超时不阻塞交互
    astrobox_ng_wit::spawn(async move {
        fetch_get_info().await;
        fetch_relay_usage().await;
    });
}

/// 激活失败：回到等待付款状态，允许重试
async fn verification_failed(message: &str) {
    state::set_verification_status(VerificationStatus::Failed);
    rerender();
    show_alert("激活失败", message).await;
}

/// 拉取授权/余额信息（POST getInfo）。HTTP 201 表示授权失效，清空本地状态
pub async fn fetch_get_info() {
    let api_key = state::with_state(|s| s.api_key.clone());
    if api_key.is_empty() {
        return;
    }

    let url = format!("{}/api/v2/getInfo/Eclipse", API_BASE_URL);
    let body = json!({ "Key": api_key });
    let headers = HashMap::new();

    match api_client::post_json_with_status(&url, &body, &headers) {
        Ok((201, _)) => handle_reactivation().await,
        Ok((_, json)) => {
            state::set_server_info(Some(json));
            rerender();
        }
        Err(err) => {
            tracing::warn!("获取授权信息失败: {}", err);
            show_alert("刷新失败", &err).await;
        }
    }
}

/// 拉取官方 AI 中转的累计用量（GET /ai/v1/usage，复合 Key 鉴权）
pub async fn fetch_relay_usage() {
    let api_key = state::with_state(|s| s.api_key.clone());
    if api_key.is_empty() {
        return;
    }

    let url = format!("{}/usage", RELAY_BASE_URL);
    let mut headers = HashMap::new();
    headers.insert(
        "Authorization".to_string(),
        format!("Bearer Eclipse-{}", api_key),
    );

    match api_client::get_json(&url, &headers) {
        Ok(usage) => state::set_relay_usage(Some(usage)),
        Err(err) => {
            // 自定义供应商/未开通 ai 服务时 403 属正常情况，静默处理
            tracing::info!("AI 用量获取失败（可能未开通中转服务）: {}", err);
            state::set_relay_usage(None);
        }
    }
    rerender();
}

/// 授权失效（201）：清空 Key 与缓存，回到激活页
async fn handle_reactivation() {
    state::clear_activation();
    rerender();
    show_alert("授权失效", "授权信息无法获取，可能已过期，请重新激活").await;
}

/// 撤销本地授权：二次确认后清空全部状态与配置文件
pub async fn delete_local_auth() {
    let confirmed = show_confirm(
        "撤销授权",
        "将删除本地 APIKey 与所有缓存数据，确定继续？",
        "取消",
        "确定",
    )
    .await;
    if !confirmed {
        return;
    }
    state::clear_activation();
    rerender();
    show_alert("成功", "本地授权信息已删除").await;
}

/// 刷新授权信息与 AI 用量（设置页按钮）
pub async fn refresh_auth_info() {
    fetch_get_info().await;
    fetch_relay_usage().await;
}

// ========== 辅助函数 ==========

/// 拼接 verify/verifyCheck 的 data 参数：`product.deviceId.serial.unix秒` 并 URL 编码
fn build_verify_url(path_and_query: &str) -> Option<String> {
    let info = state::with_state(|s| s.device_info.clone())?;
    let product = info.get("product")?.as_str()?;
    let device_id = info.get("deviceId")?.as_str()?;
    let serial = info.get("serial")?.as_str()?;
    let timestamp = state::now_unix_ms() / 1000;
    let raw = format!("{}.{}.{}.{}", product, device_id, serial, timestamp);
    Some(format!(
        "{}/api/v2{}?data={}",
        API_BASE_URL,
        path_and_query,
        encode(&raw)
    ))
}

/// APIKey 脱敏：前 4 位 + **** + 后 4 位
fn mask_api_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 12 {
        return key.to_string();
    }
    let prefix: String = chars[..4].iter().collect();
    let suffix: String = chars[chars.len() - 4..].iter().collect();
    format!("{}****{}", prefix, suffix)
}

/// 单按钮提示框
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

/// 双按钮确认框，返回是否点击主按钮
async fn show_confirm(
    title: &str,
    message: &str,
    cancel_text: &str,
    ok_text: &str,
) -> bool {
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
                    content: cancel_text.to_string(),
                },
                dialog::DialogButton {
                    id: "ok".to_string(),
                    primary: true,
                    content: ok_text.to_string(),
                },
            ],
        },
    )
    .await;
    result.clicked_btn_id == "ok"
}

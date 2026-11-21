//! 文件管理：读取手表 GET_FILES_GROUPED 分组清单，支持文本预览与下载到手机。
//! 下载走宿主 save-file 分块保存链路；二进制内容由手表 base64 回传后本地解码。

use astrobox_ng_wit::astrobox::psys_host_v4::dialog;
use base64::Engine;
use serde_json::Value;

use crate::ui::build::rerender;
use crate::ui::protocol::{self, FileAction};
use crate::ui::state;

/// 拉取文件分组
pub async fn refresh_files() {
    state::set_files_loading(true);
    rerender();
    if let Err(err) = protocol::request_files_grouped().await {
        state::set_files_loading(false);
        rerender();
        show_alert("加载文件失败", &err).await;
    }
}

/// 把分组清单拍平为 (会话 id, 扁平文件名) 列表，渲染与点击索引用同一套顺序
pub fn flat_files() -> Vec<(String, String)> {
    let groups = state::with_state(|s| s.files_groups.clone());
    let mut out = Vec::new();
    for group in groups {
        let sid = group
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let files = group
            .get("files")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for path in files {
            let path = path.as_str().unwrap_or("");
            // UI 相对路径形如 ws/<会话id>/a.txt，取最后一段作为协议要求的扁平文件名
            let name = path.rsplit('/').next().unwrap_or(path).to_string();
            out.push((sid.clone(), name));
        }
    }
    out
}

/// 点击文件：按动作发起读取。会话 id 为空（其他文件分组）时协议不支持
pub async fn open_file(global_idx: usize, action: FileAction) {
    let flat = flat_files();
    let Some((sid, name)) = flat.get(global_idx).cloned() else {
        return;
    };
    if sid.is_empty() {
        show_alert("无法操作", "该文件不在会话工作区内，协议不支持手机端读取").await;
        return;
    }
    if let Err(err) = protocol::request_session_file(&sid, &name, action).await {
        show_alert("读取文件失败", &err).await;
    }
}

/// FILES_GROUPED 回执
pub async fn on_files_reply(ok: bool, status: &str, data: Option<Value>) {
    state::set_files_loading(false);
    if !ok {
        rerender();
        show_alert("加载文件失败", status).await;
        return;
    }
    let groups = data.and_then(|v| v.as_array().cloned()).unwrap_or_default();
    state::set_files_groups(groups);
    rerender();
}

/// SESSION_FILE 回执：按 pending 动作分流到预览或下载
pub async fn on_file_reply(
    sid: &str,
    name: &str,
    action: FileAction,
    ok: bool,
    status: &str,
    data: Option<Value>,
) {
    if !ok {
        show_alert("读取文件失败", status).await;
        return;
    }
    let detail = data.unwrap_or(Value::Null);
    let is_binary = detail
        .get("base64")
        .and_then(|v| v.as_u64())
        .map(|b| b == 1)
        .unwrap_or(false);
    let raw = detail.get("data").and_then(|v| v.as_str()).unwrap_or("");

    match action {
        FileAction::Preview => {
            if is_binary {
                // 二进制没有文本原文，预览仅给提示，不允许在预览态下载
                state::set_file_preview(Some((
                    sid.to_string(),
                    name.to_string(),
                    "二进制文件（图片等），不支持页面预览，请返回列表点击「下载」保存到手机".to_string(),
                    false,
                )));
            } else {
                let text = if raw.is_empty() {
                    "（空文件）".to_string()
                } else {
                    raw.to_string()
                };
                state::set_file_preview(Some((sid.to_string(), name.to_string(), text, true)));
            }
            // 状态变更后必须主动重绘，否则界面停留在列表（预览“没反应”的根因）
            rerender();
        }
        FileAction::Download => save_to_phone(name, raw, is_binary).await,
    }
}

/// 预览页内点击下载：仅文本预览可直接存盘
pub async fn download_preview() {
    let preview = state::with_state(|s| s.file_preview.clone());
    if let Some((_sid, name, text, downloadable)) = preview {
        if !downloadable {
            show_alert("无法下载", "二进制文件请在文件列表中点击「下载」").await;
            return;
        }
        save_bytes(&name, text.as_bytes()).await;
    }
}

/// 退出预览回到分组清单
pub fn back_to_groups() {
    state::set_file_preview(None);
    rerender();
}

/// 处理下载动作的字节准备：base64 解码或文本转字节
async fn save_to_phone(name: &str, raw: &str, is_binary: bool) {
    let bytes = if is_binary {
        match base64::engine::general_purpose::STANDARD.decode(raw) {
            Ok(bytes) => bytes,
            Err(err) => {
                show_alert("下载失败", &format!("base64 解码失败: {}", err)).await;
                return;
            }
        }
    } else {
        raw.as_bytes().to_vec()
    };
    save_bytes(name, &bytes).await;
}

/// 宿主保存对话框：start → write_chunk → finish，用户取消时静默收尾
async fn save_bytes(name: &str, bytes: &[u8]) {
    let filter = dialog::FilterConfig {
        multiple: false,
        extensions: vec![],
        default_directory: String::new(),
        default_file_name: name.to_string(),
    };

    let session = match dialog::save_file_start(filter).await {
        Ok(session) => session,
        Err(_) => {
            // 用户取消也会走到这里，不算错误
            tracing::info!("用户取消保存文件: {}", name);
            return;
        }
    };

    if let Err(reason) = dialog::save_file_write_chunk(session.session_id, bytes.to_vec()).await {
        tracing::error!("写入保存分块失败: {}", reason);
        let _ = dialog::save_file_abort(session.session_id).await;
        show_alert("下载失败", &format!("写入失败: {}", reason)).await;
        return;
    }

    if let Err(reason) = dialog::save_file_finish(session.session_id).await {
        tracing::error!("完成保存失败: {}", reason);
        let _ = dialog::save_file_abort(session.session_id).await;
        show_alert("下载失败", &format!("保存失败: {}", reason)).await;
        return;
    }

    show_alert("下载完成", &format!("已保存: {}", session.name)).await;
}

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

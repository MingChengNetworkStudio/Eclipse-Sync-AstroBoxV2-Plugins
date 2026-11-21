//! 文件管理：读取手表 GET_FILES_GROUPED 分组清单，预览会话工作区文本文件。
//! 协议仅支持会话工作区文件读取，「其他文件」分组不提供预览。

use astrobox_ng_wit::astrobox::psys_host_v4::dialog;
use serde_json::Value;

use crate::ui::build::rerender;
use crate::ui::protocol;
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
fn flat_files() -> Vec<(String, String)> {
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

/// 点击文件：会话工作区文件发起读取，其他分组提示不支持
pub async fn open_file(global_idx: usize) {
    let flat = flat_files();
    let Some((sid, name)) = flat.get(global_idx).cloned() else {
        return;
    };
    if sid.is_empty() {
        show_alert("无法预览", "该文件不在会话工作区内，暂不支持手机端读取").await;
        return;
    }
    if let Err(err) = protocol::request_session_file(&sid, &name).await {
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

/// SESSION_FILE 回执
pub async fn on_file_reply(
    _sid: &str,
    name: &str,
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

    let text = if is_binary {
        "二进制文件（图片等），不支持在手机端预览".to_string()
    } else {
        detail
            .get("data")
            .and_then(|v| v.as_str())
            .unwrap_or("（空文件）")
            .to_string()
    };
    state::set_file_preview(Some((name.to_string(), text)));
}

/// 退出预览回到分组清单
pub fn back_to_groups() {
    state::set_file_preview(None);
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

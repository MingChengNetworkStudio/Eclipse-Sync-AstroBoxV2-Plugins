//! AI 设定（系统提示词）管理：列出/新建/编辑/删除，经互联协议同步到手表。

use astrobox_ng_wit::astrobox::psys_host_v4::dialog;
use serde_json::Value;

use crate::ui::build::rerender;
use crate::ui::protocol;
use crate::ui::state;

/// 拉取 AI 设定列表
pub async fn refresh_prompts() {
    state::set_prompts_loading(true);
    rerender();
    if let Err(err) = protocol::request_prompts().await {
        state::set_prompts_loading(false);
        rerender();
        show_alert("加载AI设定失败", &err).await;
    }
}

/// 进入新建态：清空草稿
pub fn new_prompt() {
    state::begin_prompt_edit(None, "", "");
}

/// 进入编辑态：把所选设定填入草稿
pub fn edit_prompt(idx: usize) {
    let target = state::with_state(|s| s.prompts.get(idx).cloned());
    if let Some(item) = target {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
        state::begin_prompt_edit(Some(id), &name, &content);
    }
}

/// 保存草稿（新建或覆盖），content 必填
pub async fn save_prompt() {
    let (id, name, content) = state::with_state(|s| {
        (
            s.prompt_edit_id.clone(),
            s.prompt_name_draft.trim().to_string(),
            s.prompt_content_draft.trim().to_string(),
        )
    });
    if content.is_empty() {
        show_alert("无法保存", "设定内容不能为空").await;
        return;
    }
    if let Err(err) = protocol::put_prompt(id.as_deref(), &name, &content).await {
        show_alert("保存失败", &err).await;
    }
}

/// 删除设定（二次确认）
pub async fn delete_prompt(idx: usize) {
    let target = state::with_state(|s| s.prompts.get(idx).cloned());
    let Some(item) = target else {
        return;
    };
    let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("未命名设定");
    if !show_confirm("删除AI设定", &format!("确定删除「{}」？", name)).await {
        return;
    }
    if let Err(err) = protocol::delete_prompt(&id).await {
        show_alert("删除失败", &err).await;
    }
}

/// PROMPTS 回执
pub fn on_prompts_reply(ok: bool, data: Option<Value>) {
    if !ok {
        state::set_prompts_loading(false);
        rerender();
        return;
    }
    let list = data.and_then(|v| v.as_array().cloned()).unwrap_or_default();
    state::set_prompts(list);
}

/// PUT_PROMPT_DONE 回执：退出编辑态并刷新列表
pub async fn on_prompt_saved(ok: bool, status: &str) {
    if !ok {
        show_alert("保存失败", status).await;
        return;
    }
    state::begin_prompt_edit(None, "", "");
    refresh_prompts().await;
}

/// DEL_PROMPT_DONE 回执
pub async fn on_prompt_deleted(_id: &str, ok: bool, status: &str) {
    if !ok {
        show_alert("删除失败", status).await;
        return;
    }
    refresh_prompts().await;
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

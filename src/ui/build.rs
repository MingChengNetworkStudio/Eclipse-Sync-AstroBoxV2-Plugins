// V4 的宿主 UI 接口：元素构建为同步调用，数据全部从 state 快照读取
use astrobox_ng_wit::astrobox::psys_host_v4::ui;

use crate::ui::chat;
use crate::ui::event_handler::*;
use crate::ui::icons;
use crate::ui::state::{self, ChatView, MainTab, VerificationStatus};

// ========== 设计令牌（全局统一配色/尺寸） ==========

const COLOR_TEXT_PRIMARY: &str = "#f4f4f5";
const COLOR_TEXT_SECONDARY: &str = "#a1a1aa";
const COLOR_TEXT_MUTED: &str = "#71717a";
const COLOR_TEXT_DANGER: &str = "#f87171";
const COLOR_ACCENT: &str = "#2563eb";
const COLOR_CARD_BG: &str = "#1E1E1F";
const COLOR_CARD_BG_2: &str = "#2A2A2A";
const COLOR_DANGER_BG: &str = "#3f1d1d";
const COLOR_BUBBLE_USER: &str = "#2563eb";
const COLOR_BUBBLE_ASSISTANT: &str = "#2A2A2A";
const COLOR_SUCCESS: &str = "#4ade80";
const SCROLL_HEIGHT: u32 = 420;

// ========== 渲染入口 ==========

pub async fn render_main_ui(element_id: &str) {
    state::set_root(element_id);
    rerender();
}

/// 同步重绘：读取最新 state 重建整棵 UI 树
pub fn rerender() {
    let Some(root) = state::root() else {
        return;
    };
    ui::render(&root, build_main_ui());
}

fn build_main_ui() -> ui::Element {
    let container = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .height_full()
        .padding(20)
        .gap(16);

    // 未激活时不显示 Tab，整页只渲染激活流程
    if !state::is_activated() {
        return container.child(build_verification_ui());
    }

    let current_tab = state::with_state(|s| s.current_tab);
    let tabs = build_tabs(current_tab);
    let content = match current_tab {
        MainTab::Chat => build_chat_tab(),
        MainTab::Files => build_files_tab(),
        MainTab::AiSettings => build_ai_settings_tab(),
        MainTab::Settings => build_settings_tab(),
    };
    container.child(tabs).child(content)
}

// ========== Tab 栏 ==========

fn build_tabs(current_tab: MainTab) -> ui::Element {
    let tabs_root = ui::Element::new(ui::ElementType::TabsRoot, None)
        .flex()
        .justify_center()
        .margin_bottom(8);

    let tabs_list = ui::Element::new(ui::ElementType::TabsList, None)
        .flex()
        .bg(COLOR_CARD_BG)
        .radius(999)
        .padding(4)
        .gap(4);

    let triggers = [
        (
            "聊天",
            icons::chat_tab_svg(),
            MainTab::Chat,
            TAB_CHAT_EVENT,
        ),
        (
            "文件",
            icons::files_tab_svg(),
            MainTab::Files,
            TAB_FILES_EVENT,
        ),
        (
            "AI设置",
            icons::ai_settings_tab_svg(),
            MainTab::AiSettings,
            TAB_AI_EVENT,
        ),
        (
            "设置",
            icons::settings_tab_svg(),
            MainTab::Settings,
            TAB_SETTINGS_EVENT,
        ),
    ];

    let mut list = tabs_list;
    for (label, svg, tab, event) in triggers {
        list = list.child(build_tab_trigger(
            label,
            svg,
            current_tab == tab,
            event,
        ));
    }

    tabs_root.child(list)
}

fn build_tab_trigger(
    label: &str,
    icon_svg: String,
    is_active: bool,
    event_id: &str,
) -> ui::Element {
    ui::Element::new(ui::ElementType::TabsTrigger, None)
        .without_default_styles()
        .on(ui::Event::Click, event_id)
        .radius(999)
        .padding_top(10)
        .padding_bottom(10)
        .padding_left(12)
        .padding_right(12)
        .bg(if is_active { COLOR_CARD_BG_2 } else { COLOR_CARD_BG })
        .text_color(if is_active { "#FFFFFF" } else { "#BBBBBB" })
        .flex()
        .align_center()
        .gap(5)
        .child(
            ui::Element::new(ui::ElementType::Svg, Some(&icon_svg))
                .width(20)
                .height(20),
        )
        .child(ui::Element::new(ui::ElementType::Span, Some(label)).size(13))
}

// ========== 激活流程页 ==========

fn build_verification_ui() -> ui::Element {
    let status = state::with_state(|s| s.verification_status);
    let root = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .align_center()
        .gap(16);

    match status {
        VerificationStatus::NotStarted => root
            .child(title_text("欢迎使用 Eclipse同步器", 24))
            .child(hint_text("接下来开始进行激活验证"))
            .child(primary_button_full("开始验证", ACT_START_EVENT)),

        VerificationStatus::CheckingDevice => root
            .child(title_text("检测设备链接", 24))
            .child(hint_text("请选择您的设备"))
            .child(build_host_device_card()),

        VerificationStatus::GettingInfo => root
            .child(title_text("正在获取信息", 24))
            .child(hint_text("正在与手表上的 Eclipse 通信，请稍后")),

        VerificationStatus::WaitingPayment => root
            .child(title_text("设备激活", 24))
            .child(hint_text("您选择的设备:"))
            .child(build_host_device_card())
            .child(section_spacer())
            .child(section_text("付费激活"))
            .child(primary_button_full("前往支付页面", ACT_OPEN_VERIFY_EVENT))
            .child(hint_text("支付完成后点击下方按钮验证"))
            .child(primary_button_full("验证支付", ACT_VERIFY_PAID_EVENT)),

        VerificationStatus::Verifying => root
            .child(title_text("验证中，请稍后", 24))
            .child(hint_text("正在验证订单并下发 Key 至设备")),

        VerificationStatus::Verified => root
            .child(title_text("一切就绪！", 24))
            .child(hint_text("开始使用吧")),

        VerificationStatus::Failed => root
            .child(title_text("验证失败", 24))
            .child(hint_text("请检查设备连接与网络后重试"))
            .child(primary_button_full("重试", ACT_RETRY_EVENT)),
    }
}

/// 激活页使用的主机设备卡片
fn build_host_device_card() -> ui::Element {
    let (name, addr) = state::with_state(|s| {
        s.host_device
            .clone()
            .unwrap_or_else(|| ("正在搜索设备...".to_string(), String::new()))
    });
    let display_name = if name.is_empty() { "未知设备".to_string() } else { name };

    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(16)
        .gap(12)
        .child(
            ui::Element::new(ui::ElementType::Svg, Some(&icons::device_svg()))
                .width(28)
                .height(28)
                .text_color(COLOR_TEXT_SECONDARY),
        )
        .child(
            ui::Element::new(ui::ElementType::Div, None)
                .flex()
                .flex_direction(ui::FlexDirection::Column)
                .gap(2)
                .flex_grow(1.0)
                .child(
                    ui::Element::new(ui::ElementType::P, Some(&display_name))
                        .size(15)
                        .text_color(COLOR_TEXT_PRIMARY),
                )
                .child(
                    ui::Element::new(ui::ElementType::P, Some(&addr))
                        .size(12)
                        .text_color(COLOR_TEXT_MUTED),
                ),
        )
}

// ========== 聊天 Tab ==========

fn build_chat_tab() -> ui::Element {
    let view = state::with_state(|s| s.chat_view);
    match view {
        ChatView::SessionList => build_session_list(),
        ChatView::Chat => build_chat_view(),
    }
}

/// 会话列表视图（标准 AI 应用首页形态）
fn build_session_list() -> ui::Element {
    let (sessions, loading) = state::with_state(|s| (s.sessions.clone(), s.sessions_loading));

    let header = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .gap(8)
        .child(
            ui::Element::new(ui::ElementType::Svg, Some(&icons::history_svg()))
                .width(20)
                .height(20)
                .text_color(COLOR_TEXT_SECONDARY),
        )
        .child(section_title("会话"))
        .child(ui::Element::new(ui::ElementType::Div, None).flex_grow(1.0))
        .child(ghost_button("同步").on(ui::Event::Click, CHAT_REFRESH_EVENT))
        .child(primary_button("新建对话", CHAT_NEW_EVENT));

    let mut col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(10)
        .child(header);

    if sessions.is_empty() {
        // 空列表：标准 AI 软件式欢迎页（问候语随机抽取并缓存）
        let (greeting, hint) = state::ensure_greeting();
        let welcome = ui::Element::new(ui::ElementType::Div, None)
            .flex()
            .flex_direction(ui::FlexDirection::Column)
            .width_full()
            .align_center()
            .gap(12)
            .padding_top(80)
            .child(
                ui::Element::new(ui::ElementType::P, Some(&greeting))
                    .size(24)
                    .text_color(COLOR_TEXT_PRIMARY),
            )
            .child(
                ui::Element::new(ui::ElementType::P, Some(&hint))
                    .size(14)
                    .text_color(COLOR_TEXT_SECONDARY),
            )
            .child(
                ui::Element::new(ui::ElementType::P, Some("对话记录会自动同步回手表"))
                    .size(12)
                    .text_color(COLOR_TEXT_MUTED),
            )
            .child(section_spacer())
            .child(primary_button("＋ 新建对话", CHAT_NEW_EVENT));
        let hint = if loading { "正在从手表加载会话…" } else { "" };
        col = col.child(welcome);
        if !hint.is_empty() {
            col = col.child(hint_text(hint));
        }
        return col;
    }

    for (idx, session) in sessions.iter().enumerate() {
        col = col.child(build_session_row(session, idx));
    }
    col
}

/// 单条会话行
fn build_session_row(session: &serde_json::Value, idx: usize) -> ui::Element {
    let title = chat::session_title(session);
    let time = format_timestamp(chat::session_updated_at(session));

    let open_btn = ui::Element::new(ui::ElementType::Button, Some(&title))
        .without_default_styles()
        .flex()
        .flex_grow(1.0)
        .padding_top(12)
        .padding_bottom(12)
        .padding_left(14)
        .padding_right(14)
        .radius(8)
        .bg(COLOR_CARD_BG_2)
        .text_color(COLOR_TEXT_PRIMARY)
        .size(14)
        .on(ui::Event::Click, &format!("{}{}", CHAT_OPEN_PREFIX, idx));

    let info_col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .gap(4)
        .flex_grow(1.0)
        .child(open_btn)
        .child(
            ui::Element::new(ui::ElementType::P, Some(&time))
                .size(11)
                .text_color(COLOR_TEXT_MUTED),
        );

    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(12)
        .gap(10)
        .child(info_col)
        .child(
            danger_button("删除").on(
                ui::Event::Click,
                &format!("{}{}", CHAT_DEL_PREFIX, idx),
            ),
        )
}

/// 对话视图：顶部栏 + 自适应消息区（flex_grow 撑满）+ 贴底聊天栏
fn build_chat_view() -> ui::Element {
    let (title, has_older, messages_loading, sending) = state::with_state(|s| {
        let title = s
            .sessions
            .iter()
            .find(|sess| {
                chat::session_id(sess) == s.current_session_id.clone().unwrap_or_default()
            })
            .map(chat::session_title)
            .unwrap_or_else(|| "对话".to_string());
        (title, s.older_chunks > 0, s.messages_loading, s.sending)
    });

    // 顶部栏卡片：返回 + 标题 + 模型选择
    let top_bar = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(8)
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(10)
        .child(
            ui::Element::new(ui::ElementType::Div, None)
                .flex()
                .flex_direction(ui::FlexDirection::Row)
                .align_center()
                .width_full()
                .gap(8)
                .child(ghost_button("返回").on(ui::Event::Click, CHAT_BACK_EVENT))
                .child(
                    ui::Element::new(ui::ElementType::P, Some(&title))
                        .size(16)
                        .text_color(COLOR_TEXT_PRIMARY)
                        .flex_grow(1.0),
                )
                .child(build_model_select()),
        )
        .child(build_prompt_select());

    // 消息区 flex_grow 吃掉中间高度，把输入栏顶到屏幕最下方
    let mut col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .height_full()
        .gap(10)
        .child(top_bar);

    if has_older {
        let older_text = if messages_loading { "加载中…" } else { "加载更早消息" };
        col = col.child(ghost_button(older_text).on(ui::Event::Click, CHAT_LOAD_OLDER_EVENT));
    }

    col.child(build_message_scroll()).child(build_input_area(sending))
}

/// AI 设定下拉：不使用 + 全部已保存设定，当前选中跟随会话 promptId
fn build_prompt_select() -> ui::Element {
    let (prompts, selected_id) = state::with_state(|s| {
        (
            s.prompts
                .iter()
                .filter_map(|p| {
                    p.get("name").and_then(|v| v.as_str()).map(str::to_string)
                })
                .collect::<Vec<_>>(),
            s.selected_prompt_id.clone(),
        )
    });

    // 由 id 反查显示名
    let current = state::with_state(|s| {
        if selected_id.is_empty() {
            "不使用AI设定".to_string()
        } else {
            s.prompts
                .iter()
                .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(selected_id.as_str()))
                .and_then(|p| p.get("name").and_then(|v| v.as_str()).map(str::to_string))
                .unwrap_or_else(|| "不使用AI设定".to_string())
        }
    });

    let mut select = ui::Element::new(ui::ElementType::Select, Some(&current))
        .on(ui::Event::Change, AI_PROMPT_SELECT_EVENT)
        .radius(8)
        .padding(8)
        .bg(COLOR_CARD_BG_2)
        .size(13)
        .width_full();

    select = select.child(
        ui::Element::new(ui::ElementType::Option, Some("不使用AI设定"))
            .prop("value", "不使用AI设定"),
    );
    for name in &prompts {
        select = select.child(
            ui::Element::new(ui::ElementType::Option, Some(name)).prop("value", name),
        );
    }
    select
}

/// 模型下拉
fn build_model_select() -> ui::Element {
    let (models, selected) = state::with_state(|s| {
        let selected = if s.selected_model.is_empty() {
            s.active_provider.active_model.clone()
        } else {
            s.selected_model.clone()
        };
        (s.active_provider.models.clone(), selected)
    });

    let current = if selected.is_empty() {
        "暂无模型".to_string()
    } else {
        selected
    };
    let mut select = ui::Element::new(ui::ElementType::Select, Some(&current))
        .on(ui::Event::Change, CHAT_MODEL_EVENT)
        .radius(8)
        .padding(8)
        .bg(COLOR_CARD_BG_2)
        .size(13);

    if models.is_empty() {
        select = select.child(
            ui::Element::new(ui::ElementType::Option, Some("暂无可用模型"))
                .prop("value", "暂无可用模型"),
        );
    } else {
        for model in &models {
            select = select.child(
                ui::Element::new(ui::ElementType::Option, Some(model)).prop("value", model),
            );
        }
    }
    select
}

/// 消息滚动区：flex_grow 撑满顶部栏与输入栏之间的空间
fn build_message_scroll() -> ui::Element {
    let (messages, sending, streaming) = state::with_state(|s| {
        (s.messages.clone(), s.sending, s.streaming_text.clone())
    });

    let mut scroll = ui::Element::new(ui::ElementType::ScrollArea, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .gap(12)
        .width_full()
        .flex_grow(1.0)
        .min_height(240)
        .padding(8);

    if messages.is_empty() && !sending {
        return scroll.child(build_chat_welcome());
    }

    for msg in &messages {
        scroll = scroll.child(build_bubble(chat::msg_role(msg), chat::msg_content(msg)));
    }

    // 流式输出中的 assistant 占位气泡
    if sending {
        let text = if streaming.is_empty() {
            "思考中…"
        } else {
            streaming.as_str()
        };
        scroll = scroll.child(build_bubble("assistant", text));
    }
    scroll
}

/// 对话内空消息欢迎卡
fn build_chat_welcome() -> ui::Element {
    let (greeting, hint) = state::ensure_greeting();
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .align_center()
        .gap(10)
        .padding_top(60)
        .child(
            ui::Element::new(ui::ElementType::P, Some(&greeting))
                .size(20)
                .text_color(COLOR_TEXT_PRIMARY),
        )
        .child(
            ui::Element::new(ui::ElementType::P, Some(&hint))
                .size(13)
                .text_color(COLOR_TEXT_MUTED),
        )
}

/// 单条消息气泡：user 右蓝、assistant 左灰，用位置与颜色区分角色
fn build_bubble(role: &str, content: &str) -> ui::Element {
    let is_user = role == "user";
    let bg = if is_user { COLOR_BUBBLE_USER } else { COLOR_BUBBLE_ASSISTANT };
    let text_color = if is_user { "#FFFFFF" } else { COLOR_TEXT_PRIMARY };

    let bubble = ui::Element::new(ui::ElementType::Div, None)
        .radius(14)
        .padding(14)
        .max_width(280)
        .bg(bg)
        .child(
            ui::Element::new(ui::ElementType::P, Some(content))
                .size(14)
                .text_color(text_color),
        );

    let mut row = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .width_full();
    row = if is_user {
        row.justify_end()
    } else {
        row.justify_start()
    };
    row.child(bubble)
}

/// 底部聊天栏：卡片容器 + 加高输入框 + 发送/停止，固定在视图最下方
fn build_input_area(sending: bool) -> ui::Element {
    let draft = state::with_state(|s| s.draft.clone());
    let input = ui::Element::new(ui::ElementType::Input, Some(&draft))
        .prop("type", "text")
        .prop("placeholder", "输入消息…")
        .flex_grow(1.0)
        .min_height(48)
        .padding(12)
        .radius(10)
        .bg(COLOR_CARD_BG_2)
        .text_color(COLOR_TEXT_PRIMARY)
        .size(14)
        .on(ui::Event::Input, CHAT_INPUT_EVENT);

    let action = if sending {
        danger_button("停止").on(ui::Event::Click, CHAT_STOP_EVENT)
    } else {
        primary_button("发送", CHAT_SEND_EVENT)
    };

    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .gap(8)
        .bg(COLOR_CARD_BG)
        .radius(14)
        .padding(10)
        .child(input)
        .child(action)
}

// ========== 文件管理 Tab ==========

fn build_files_tab() -> ui::Element {
    // 预览视图优先
    if let Some((_, name, text, downloadable)) = state::with_state(|s| s.file_preview.clone()) {
        return build_file_preview(&name, &text, downloadable);
    }

    let (groups, loading) = state::with_state(|s| (s.files_groups.clone(), s.files_loading));

    let header = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .gap(8)
        .child(
            ui::Element::new(ui::ElementType::Svg, Some(&icons::files_tab_svg()))
                .width(20)
                .height(20)
                .text_color(COLOR_TEXT_SECONDARY),
        )
        .child(section_title("文件管理"))
        .child(ui::Element::new(ui::ElementType::Div, None).flex_grow(1.0))
        .child(ghost_button("同步").on(ui::Event::Click, FILES_REFRESH_EVENT));

    let mut col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(10)
        .child(header);

    if groups.is_empty() {
        let hint = if loading { "正在加载文件…" } else { "暂无文件" };
        return col.child(hint_text(hint));
    }

    // 拍平索引与 files::flat_files/open_file 的顺序保持一致
    let mut global_idx = 0usize;
    for group in &groups {
        let group_id = group.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let group_title = group
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("已删除的会话");
        let files = group
            .get("files")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        if files.is_empty() {
            continue;
        }

        col = col.child(section_title(group_title));
        for path in &files {
            let path = path.as_str().unwrap_or("");
            let name = path.rsplit('/').next().unwrap_or(path);

            if group_id.is_empty() {
                // 「其他文件」分组不在会话工作区，协议无法读取
                col = col.child(
                    ui::Element::new(ui::ElementType::Div, None)
                        .flex()
                        .flex_direction(ui::FlexDirection::Row)
                        .align_center()
                        .width_full()
                        .bg(COLOR_CARD_BG)
                        .radius(10)
                        .padding(12)
                        .gap(8)
                        .child(
                            ui::Element::new(ui::ElementType::P, Some(name))
                                .size(13)
                                .text_color(COLOR_TEXT_MUTED)
                                .flex_grow(1.0),
                        )
                        .child(detail_text("不可操作")),
                );
            } else {
                col = col.child(
                    ui::Element::new(ui::ElementType::Div, None)
                        .flex()
                        .flex_direction(ui::FlexDirection::Row)
                        .align_center()
                        .width_full()
                        .bg(COLOR_CARD_BG)
                        .radius(10)
                        .padding(10)
                        .gap(8)
                        .child(
                            ui::Element::new(ui::ElementType::P, Some(name))
                                .size(13)
                                .text_color(COLOR_TEXT_PRIMARY)
                                .flex_grow(1.0),
                        )
                        .child(
                            primary_button("预览", &format!("{}{}", FILES_PREVIEW_PREFIX, global_idx)),
                        )
                        .child(
                            primary_button("下载", &format!("{}{}", FILES_DOWNLOAD_PREFIX, global_idx)),
                        ),
                );
            }
            global_idx += 1;
        }
    }
    col
}

/// 文件预览视图：标题栏（返回+文件名+下载）+ 内容滚动区
fn build_file_preview(name: &str, text: &str, downloadable: bool) -> ui::Element {
    let mut top_bar = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .gap(8)
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(10)
        .child(ghost_button("返回").on(ui::Event::Click, FILES_BACK_EVENT))
        .child(
            ui::Element::new(ui::ElementType::P, Some(name))
                .size(14)
                .text_color(COLOR_TEXT_PRIMARY)
                .flex_grow(1.0),
        );
    if downloadable {
        top_bar = top_bar.child(primary_button("下载", FILES_DOWNLOAD_CURRENT_EVENT));
    }

    let scroll = ui::Element::new(ui::ElementType::ScrollArea, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .gap(8)
        .width_full()
        .height(SCROLL_HEIGHT)
        .padding(12)
        .bg(COLOR_CARD_BG)
        .radius(12)
        .child(
            ui::Element::new(ui::ElementType::P, Some(text))
                .size(13)
                .text_color(COLOR_TEXT_PRIMARY),
        );

    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(10)
        .child(top_bar)
        .child(scroll)
}

// ========== AI 设置 Tab ==========

fn build_ai_settings_tab() -> ui::Element {
    let mut col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(10);

    col = col
        .child(build_section_header("供应商", AI_REFRESH_EVENT))
        .child(build_provider_select())
        .child(build_active_provider_card())
        .child(build_provider_editor())
        .child(build_custom_provider_list())
        .child(section_spacer())
        .child(section_title("AI 设定"))
        .child(build_prompt_editor())
        .child(build_prompt_list());
    col
}

/// 供应商下拉
fn build_provider_select() -> ui::Element {
    let (options, current) = state::with_state(|s| {
        let mut options = vec!["官方中转".to_string()];
        options.extend(s.providers.iter().map(|p| p.name.clone()));
        let current = if s.active_provider.is_official {
            "官方中转".to_string()
        } else {
            s.active_provider.name.clone()
        };
        (options, current)
    });

    let mut select = ui::Element::new(ui::ElementType::Select, Some(&current))
        .on(ui::Event::Change, AI_PROVIDER_EVENT)
        .radius(8)
        .padding(10)
        .bg(COLOR_CARD_BG_2)
        .size(14)
        .width_full();
    for label in &options {
        select = select.child(
            ui::Element::new(ui::ElementType::Option, Some(label)).prop("value", label),
        );
    }
    select
}

/// 当前生效供应商信息卡（含模型清单）
fn build_active_provider_card() -> ui::Element {
    let (name, base_url, model, models) = state::with_state(|s| {
        (
            s.active_provider.name.clone(),
            s.active_provider.base_url.clone(),
            s.active_provider.active_model.clone(),
            s.active_provider.models.clone(),
        )
    });

    let mut card = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(6)
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(14)
        .child(
            ui::Element::new(ui::ElementType::P, Some(&name))
                .size(15)
                .text_color(COLOR_TEXT_PRIMARY),
        )
        .child(detail_text(&format!("接口: {}", base_url)))
        .child(detail_text(&format!(
            "当前模型: {}",
            if model.is_empty() { "未选择" } else { &model }
        )));

    // 模型清单逐行展示
    if !models.is_empty() {
        card = card.child(detail_text(&format!("可用模型（{} 个）:", models.len())));
        for m in &models {
            card = card.child(
                ui::Element::new(ui::ElementType::P, Some(m))
                    .size(12)
                    .text_color(COLOR_TEXT_SECONDARY)
                    .margin_left(8),
            );
        }
    }
    card
}

/// 供应商编辑器：名称/地址/Key 输入 + 模型行的增删改
fn build_provider_editor() -> ui::Element {
    let (edit_id, name, base_url, api_key, models) = state::with_state(|s| {
        (
            s.provider_edit_id.clone(),
            s.provider_name_draft.clone(),
            s.provider_baseurl_draft.clone(),
            s.provider_apikey_draft.clone(),
            s.provider_models_draft.clone(),
        )
    });
    let title = if edit_id.is_some() { "编辑供应商" } else { "新建供应商" };

    let mut editor = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(8)
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(14)
        .child(section_text(title))
        .child(build_form_input("显示名称", &name, AI_PROVIDER_NAME_INPUT))
        .child(build_form_input("接口地址（OpenAI 兼容）", &base_url, AI_PROVIDER_BASEURL_INPUT))
        .child(build_form_input("API Key（可留空）", &api_key, AI_PROVIDER_APIKEY_INPUT))
        .child(section_text("模型列表"));

    // 每个模型一行：输入框 + 删除
    for (idx, model) in models.iter().enumerate() {
        let row = ui::Element::new(ui::ElementType::Div, None)
            .flex()
            .flex_direction(ui::FlexDirection::Row)
            .align_center()
            .width_full()
            .gap(8)
            .child(
                ui::Element::new(ui::ElementType::Input, Some(model))
                    .prop("type", "text")
                    .prop("placeholder", "模型名")
                    .flex_grow(1.0)
                    .min_height(44)
                    .padding(10)
                    .radius(8)
                    .bg(COLOR_CARD_BG_2)
                    .text_color(COLOR_TEXT_PRIMARY)
                    .size(13)
                    .on(
                        ui::Event::Input,
                        &format!("{}{}", AI_MODEL_INPUT_PREFIX, idx),
                    ),
            )
            .child(
                danger_button("删除").on(
                    ui::Event::Click,
                    &format!("{}{}", AI_MODEL_DEL_PREFIX, idx),
                ),
            );
        editor = editor.child(row);
    }

    editor
        .child(ghost_button("＋ 添加模型").on(ui::Event::Click, AI_MODEL_ADD_EVENT))
        .child(
            ui::Element::new(ui::ElementType::Div, None)
                .flex()
                .flex_direction(ui::FlexDirection::Row)
                .gap(8)
                .child(primary_button("保存", AI_PROVIDER_SAVE_EVENT))
                .child(ghost_button("清空").on(ui::Event::Click, AI_PROVIDER_CANCEL_EVENT))
                .child(ghost_button("新建").on(ui::Event::Click, AI_PROVIDER_NEW_EVENT)),
        )
}

/// 表单输入框统一样式
fn build_form_input(placeholder: &str, value: &str, event_id: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Input, Some(value))
        .prop("type", "text")
        .prop("placeholder", placeholder)
        .width_full()
        .min_height(44)
        .padding(10)
        .radius(8)
        .bg(COLOR_CARD_BG_2)
        .text_color(COLOR_TEXT_PRIMARY)
        .size(13)
        .on(ui::Event::Input, event_id)
}

/// 自定义供应商列表：每行带编辑/删除
fn build_custom_provider_list() -> ui::Element {
    let providers = state::with_state(|s| s.providers.clone());
    if providers.is_empty() {
        return hint_text("暂无自定义供应商，可在上方表单新建");
    }

    let mut col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(8);
    for (idx, provider) in providers.iter().enumerate() {
        let info = ui::Element::new(ui::ElementType::Div, None)
            .flex()
            .flex_direction(ui::FlexDirection::Column)
            .gap(4)
            .flex_grow(1.0)
            .child(
                ui::Element::new(ui::ElementType::P, Some(&provider.name))
                    .size(14)
                    .text_color(COLOR_TEXT_PRIMARY),
            )
            .child(detail_text(&provider.base_url))
            .child(detail_text(&format!("模型: {} 个", provider.models.len())));

        let row = ui::Element::new(ui::ElementType::Div, None)
            .flex()
            .flex_direction(ui::FlexDirection::Row)
            .align_center()
            .width_full()
            .gap(8)
            .bg(COLOR_CARD_BG)
            .radius(12)
            .padding(12)
            .child(info)
            .child(
                ghost_button("编辑").on(
                    ui::Event::Click,
                    &format!("{}{}", AI_PROVIDER_EDIT_PREFIX, idx),
                ),
            )
            .child(
                danger_button("删除").on(
                    ui::Event::Click,
                    &format!("{}{}", AI_PROVIDER_DEL_PREFIX, idx),
                ),
            );
        col = col.child(row);
    }
    col
}

/// AI 设定编辑器（新建/编辑共用）
fn build_prompt_editor() -> ui::Element {
    let (edit_id, name_draft, content_draft) = state::with_state(|s| {
        (
            s.prompt_edit_id.clone(),
            s.prompt_name_draft.clone(),
            s.prompt_content_draft.clone(),
        )
    });
    let editor_title = if edit_id.is_some() { "编辑AI设定" } else { "新建AI设定" };

    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(8)
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(14)
        .child(section_text(editor_title))
        .child(
            ui::Element::new(ui::ElementType::Input, Some(&name_draft))
                .prop("type", "text")
                .prop("placeholder", "设定名称（最长12字）")
                .width_full()
                .padding(10)
                .radius(8)
                .bg(COLOR_CARD_BG_2)
                .text_color(COLOR_TEXT_PRIMARY)
                .size(13)
                .on(ui::Event::Input, AI_PROMPT_NAME_INPUT),
        )
        .child(
            ui::Element::new(ui::ElementType::Textarea, Some(&content_draft))
                .prop("placeholder", "系统提示词内容")
                .width_full()
                .height(180)
                .padding(10)
                .radius(8)
                .bg(COLOR_CARD_BG_2)
                .text_color(COLOR_TEXT_PRIMARY)
                .size(13)
                .on(ui::Event::Input, AI_PROMPT_CONTENT_INPUT),
        )
        .child(
            ui::Element::new(ui::ElementType::Div, None)
                .flex()
                .flex_direction(ui::FlexDirection::Row)
                .gap(8)
                .child(primary_button("保存", AI_PROMPT_SAVE_EVENT))
                .child(ghost_button("清空").on(ui::Event::Click, AI_PROMPT_CANCEL_EVENT))
                .child(ghost_button("新建").on(ui::Event::Click, AI_PROMPT_NEW_EVENT)),
        )
}

/// AI 设定列表
fn build_prompt_list() -> ui::Element {
    let prompts = state::with_state(|s| s.prompts.clone());
    if prompts.is_empty() {
        return hint_text("暂无 AI 设定");
    }

    let mut col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(8);

    for (idx, item) in prompts.iter().enumerate() {
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("未命名设定");
        let content = item
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let preview: String = content.chars().take(40).collect();

        let row = ui::Element::new(ui::ElementType::Div, None)
            .flex()
            .flex_direction(ui::FlexDirection::Row)
            .align_center()
            .width_full()
            .gap(8)
            .bg(COLOR_CARD_BG)
            .radius(12)
            .padding(12)
            .child(
                ui::Element::new(ui::ElementType::Div, None)
                    .flex()
                    .flex_direction(ui::FlexDirection::Column)
                    .gap(4)
                    .flex_grow(1.0)
                    .child(
                        ui::Element::new(ui::ElementType::P, Some(name))
                            .size(14)
                            .text_color(COLOR_TEXT_PRIMARY),
                    )
                    .child(
                        ui::Element::new(ui::ElementType::P, Some(&preview))
                            .size(11)
                            .text_color(COLOR_TEXT_MUTED),
                    ),
            )
            .child(
                ghost_button("编辑").on(
                    ui::Event::Click,
                    &format!("{}{}", AI_PROMPT_EDIT_PREFIX, idx),
                ),
            )
            .child(
                danger_button("删除").on(
                    ui::Event::Click,
                    &format!("{}{}", AI_PROMPT_DEL_PREFIX, idx),
                ),
            );
        col = col.child(row);
    }
    col
}

// ========== 设置 Tab（含原关于页内容） ==========

fn build_settings_tab() -> ui::Element {
    let mut col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(10);

    col = col
        .child(section_title("授权状态"))
        .child(build_apikey_card())
        .child(build_balance_card())
        .child(build_recharge_card())
        .child(build_relay_usage_card())
        .child(build_device_info_card())
        .child(build_refresh_auth_button())
        .child(build_revoke_card())
        .child(section_title("更多内容"))
        .child(build_link_card(
            icons::help_svg(),
            "帮助文档",
            "操作步骤与常见问题解答",
            OPEN_HELP_DOC_EVENT,
        ))
        .child(build_link_card(
            icons::qq_group_svg(),
            "QQ交流群",
            "1076096725",
            OPEN_QQ_GROUP_EVENT,
        ))
        .child(section_title("构建信息"))
        .child(build_info_row(icons::time_svg(), "构建时间", &build_time_text()))
        .child(build_info_row(
            icons::user_svg(),
            "构建用户",
            option_env!("AB_BUILD_USER").unwrap_or("unknown"),
        ))
        .child(build_info_row(
            icons::branch_svg(),
            "当前分支",
            option_env!("AB_BUILD_GIT_BRANCH").unwrap_or("unknown"),
        ))
        .child(build_info_row(
            icons::hash_svg(),
            "当前hash",
            &short_git_hash(option_env!("AB_BUILD_GIT_HASH").unwrap_or("unknown")),
        ));
    col
}

/// APIKey 状态卡：点击切换显隐
fn build_apikey_card() -> ui::Element {
    let (key, visible, verified) = state::with_state(|s| {
        (s.api_key.clone(), s.api_key_visible, !s.api_key.is_empty())
    });
    let display = if !verified {
        "未激活".to_string()
    } else if visible {
        key
    } else {
        "********************************".to_string()
    };
    let hint = if visible {
        "点击隐藏 APIKey，请勿传播/分享"
    } else {
        "点击显示 APIKey，请勿传播/分享"
    };

    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(14)
        .gap(8)
        .on(ui::Event::Click, SET_TOGGLE_KEY_EVENT)
        .child(
            ui::Element::new(ui::ElementType::Div, None)
                .flex()
                .flex_direction(ui::FlexDirection::Row)
                .align_center()
                .width_full()
                .child(
                    ui::Element::new(ui::ElementType::P, Some("APIKey 状态"))
                        .size(15)
                        .text_color(COLOR_TEXT_PRIMARY),
                )
                .child(ui::Element::new(ui::ElementType::Div, None).flex_grow(1.0))
                .child(
                    ui::Element::new(ui::ElementType::P, Some(if verified { "已验证" } else { "未验证" }))
                        .size(14)
                        .text_color(if verified { COLOR_SUCCESS } else { COLOR_TEXT_DANGER }),
                ),
        )
        .child(
            ui::Element::new(ui::ElementType::P, Some(&display))
                .size(14)
                .text_color(COLOR_TEXT_SECONDARY),
        )
        .child(
            ui::Element::new(ui::ElementType::P, Some(hint))
                .size(11)
                .text_color(COLOR_TEXT_MUTED),
        )
}

/// 余额卡：大字号金额 + 进度条 + 计费字段
fn build_balance_card() -> ui::Element {
    let info = state::with_state(|s| s.server_info.clone());
    let mut card = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(14)
        .gap(10)
        .child(
            ui::Element::new(ui::ElementType::Div, None)
                .flex()
                .flex_direction(ui::FlexDirection::Row)
                .align_center()
                .gap(8)
                .child(
                    ui::Element::new(ui::ElementType::Svg, Some(&icons::wallet_svg()))
                        .width(18)
                        .height(18)
                        .text_color(COLOR_TEXT_SECONDARY),
                )
                .child(
                    ui::Element::new(ui::ElementType::P, Some("账户余额"))
                        .size(15)
                        .text_color(COLOR_TEXT_PRIMARY)
                        .flex_grow(1.0),
                ),
        );

    let Some(result) = info.as_ref().and_then(|v| v.get("result")) else {
        return card.child(hint_text("暂无余额信息，请点击「刷新授权信息」"));
    };

    let used = amount_str(result, "usedAmount");
    let total = amount_str(result, "totalAmount");
    if !used.is_empty() && !total.is_empty() {
        card = card.child(
            ui::Element::new(ui::ElementType::P, Some(&format!("{} / {} 元", used, total)))
                .size(22)
                .text_color(COLOR_TEXT_PRIMARY),
        );
        if let (Ok(used_num), Ok(total_num)) = (used.parse::<f64>(), total.parse::<f64>()) {
            if total_num > 0.0 {
                let percent = ((used_num / total_num) * 100.0).min(100.0) as u32;
                card = card.child(
                    ui::Element::new(ui::ElementType::Progress, None)
                        .width_full()
                        .height(12)
                        .prop("value", &percent.to_string()),
                );
            }
        }
    }

    // 计费附加字段做成 chips 行
    for (field, label) in [
        ("userType", "用户类型"),
        ("billingMode", "计费模式"),
        ("expiredAt", "到期时间"),
        ("remainingAmount", "剩余金额(元)"),
    ] {
        if let Some(value) = result.get(field).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            card = card.child(detail_text(&format!("{}: {}", label, value)));
        }
    }
    card
}

/// 充值入口卡：2 列金额 tab，点击打开对应 planOrder 支付页
fn build_recharge_card() -> ui::Element {
    let mut grid = ui::Element::new(ui::ElementType::Grid, None)
        .width_full()
        .grid_template_columns("repeat(2, minmax(0, 1fr))")
        .gap(8);

    for (order, amount) in crate::ui::activation::RECHARGE_PLANS {
        let label = format!("{} 元", amount);
        grid = grid.child(
            ui::Element::new(ui::ElementType::Button, Some(&label))
                .without_default_styles()
                .padding_top(14)
                .padding_bottom(14)
                .radius(10)
                .bg(COLOR_CARD_BG_2)
                .text_color(COLOR_TEXT_PRIMARY)
                .size(15)
                .on(
                    ui::Event::Click,
                    &format!("{}{}", SET_RECHARGE_PREFIX, order),
                ),
        );
    }

    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(8)
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(14)
        .child(section_text("充值余额"))
        .child(detail_text("点击金额在浏览器完成支付，到账后返回刷新授权"))
        .child(grid)
}

/// AI 中转用量卡：汇总网格 + 分模型小卡
fn build_relay_usage_card() -> ui::Element {
    let Some(usage) = state::with_state(|s| s.relay_usage.clone()) else {
        return ui::Element::new(ui::ElementType::Div, None)
            .flex()
            .flex_direction(ui::FlexDirection::Column)
            .width_full()
            .bg(COLOR_CARD_BG)
            .radius(12)
            .padding(14)
            .gap(8)
            .child(
                ui::Element::new(ui::ElementType::Div, None)
                    .flex()
                    .flex_direction(ui::FlexDirection::Row)
                    .align_center()
                    .gap(8)
                    .child(
                        ui::Element::new(ui::ElementType::Svg, Some(&icons::usage_svg()))
                            .width(18)
                            .height(18)
                            .text_color(COLOR_TEXT_SECONDARY),
                    )
                    .child(
                        ui::Element::new(ui::ElementType::P, Some("AI 中转用量"))
                            .size(15)
                            .text_color(COLOR_TEXT_PRIMARY),
                    ),
            )
            .child(hint_text("暂无数据（自定义供应商或未开通中转服务时不可用）"));
    };

    let calls = usage.get("calls").and_then(|v| v.as_u64()).unwrap_or(0);
    let cost = usage.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let input = usage.get("inputTokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let output = usage.get("outputTokens").and_then(|v| v.as_u64()).unwrap_or(0);

    // 汇总网格：2 列指标
    let grid = ui::Element::new(ui::ElementType::Grid, None)
        .width_full()
        .grid_template_columns("repeat(2, minmax(0, 1fr))")
        .gap(8)
        .child(build_metric_cell("总调用", &format!("{} 次", calls)))
        .child(build_metric_cell("总费用", &format!("{:.2} 元", cost)))
        .child(build_metric_cell("输入 Tokens", &format_number(input)))
        .child(build_metric_cell("输出 Tokens", &format_number(output)));

    let mut card = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(14)
        .gap(10)
        .child(
            ui::Element::new(ui::ElementType::Div, None)
                .flex()
                .flex_direction(ui::FlexDirection::Row)
                .align_center()
                .gap(8)
                .child(
                    ui::Element::new(ui::ElementType::Svg, Some(&icons::usage_svg()))
                        .width(18)
                        .height(18)
                        .text_color(COLOR_TEXT_SECONDARY),
                )
                .child(
                    ui::Element::new(ui::ElementType::P, Some("AI 中转用量"))
                        .size(15)
                        .text_color(COLOR_TEXT_PRIMARY)
                        .flex_grow(1.0),
                ),
        )
        .child(grid);

    // 分模型明细，每个模型一张小卡
    if let Some(models) = usage.get("byModel").and_then(|v| v.as_array()) {
        for item in models {
            let model = item.get("model").and_then(|v| v.as_str()).unwrap_or("未知模型");
            let m_calls = item.get("calls").and_then(|v| v.as_u64()).unwrap_or(0);
            let m_cost = item.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let m_in = item.get("inputTokens").and_then(|v| v.as_u64()).unwrap_or(0);
            let m_out = item.get("outputTokens").and_then(|v| v.as_u64()).unwrap_or(0);
            card = card.child(
                ui::Element::new(ui::ElementType::Div, None)
                    .flex()
                    .flex_direction(ui::FlexDirection::Column)
                    .width_full()
                    .gap(4)
                    .bg(COLOR_CARD_BG_2)
                    .radius(10)
                    .padding(10)
                    .child(
                        ui::Element::new(ui::ElementType::P, Some(model))
                            .size(13)
                            .text_color(COLOR_TEXT_PRIMARY),
                    )
                    .child(detail_text(&format!("{} 次 · {:.2} 元", m_calls, m_cost)))
                    .child(detail_text(&format!(
                        "入 {} / 出 {} Tokens",
                        format_number(m_in),
                        format_number(m_out)
                    ))),
            );
        }
    }
    card
}

/// 用量网格里的指标单元
fn build_metric_cell(label: &str, value: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .gap(4)
        .bg(COLOR_CARD_BG_2)
        .radius(10)
        .padding(10)
        .child(
            ui::Element::new(ui::ElementType::P, Some(value))
                .size(16)
                .text_color(COLOR_TEXT_PRIMARY),
        )
        .child(
            ui::Element::new(ui::ElementType::P, Some(label))
                .size(11)
                .text_color(COLOR_TEXT_MUTED),
        )
}

/// 设备信息卡
fn build_device_info_card() -> ui::Element {
    let Some(info) = state::with_state(|s| s.device_info.clone()) else {
        return ui::Element::new(ui::ElementType::Div, None)
            .flex()
            .flex_direction(ui::FlexDirection::Column)
            .width_full()
            .bg(COLOR_CARD_BG)
            .radius(12)
            .padding(14)
            .child(hint_text("设备信息缺失，重新激活后可获取"));
    };

    let fields = [
        ("设备代号", "product"),
        ("设备类型", "deviceType"),
        ("屏幕类型", "screenShape"),
        ("设备ID", "deviceId"),
        ("序列号", "serial"),
    ];

    let mut card = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(14)
        .gap(6)
        .child(section_text("设备信息"));

    for (label, key) in fields {
        let value = info.get(key).and_then(|v| v.as_str()).unwrap_or("-");
        card = card.child(detail_text(&format!("{}: {}", label, value)));
    }
    card
}

/// 刷新授权信息按钮（Eternal 同款 icon+text 全宽按钮，支持 loading 态）
fn build_refresh_auth_button() -> ui::Element {
    let refreshing = state::with_state(|s| s.auth_refreshing);
    let label = if refreshing {
        "刷新中…"
    } else {
        "刷新授权信息"
    };
    let mut button = build_icon_text_button_full(label, icons::refresh_svg(), SET_REFRESH_AUTH_EVENT);
    if refreshing {
        button = button.disabled();
    }
    button
}

/// 撤销授权红色卡（Eternal build_settings_card_colored 同款：淡红底+红字+图标）
fn build_revoke_card() -> ui::Element {
    build_colored_card(
        icons::refresh_svg(),
        "撤销授权",
        Some("删除本地 Key 与全部缓存数据"),
        SET_REVOKE_EVENT,
        "#F4433626",
        "#F44336",
    )
}

// ========== 通用组件 ==========

/// 图标+文字全宽按钮（复刻 Eternal build_icon_text_button_full）
fn build_icon_text_button_full(
    label: &str,
    icon_svg: String,
    event_id: &str,
) -> ui::Element {
    let icon = ui::Element::new(ui::ElementType::Svg, Some(&icon_svg))
        .width(22)
        .height(22);
    let text = ui::Element::new(ui::ElementType::Span, Some(label)).size(14);

    ui::Element::new(ui::ElementType::Button, None)
        .without_default_styles()
        .on(ui::Event::Click, event_id)
        .radius(12)
        .padding(14)
        .bg(COLOR_CARD_BG_2)
        .text_color(COLOR_TEXT_PRIMARY)
        .width_full()
        .flex()
        .align_center()
        .gap(8)
        .child(icon)
        .child(text)
}

/// 彩色设置卡（复刻 Eternal build_settings_card_colored，用于危险操作）
fn build_colored_card(
    icon_svg: String,
    title: &str,
    desc: Option<&str>,
    click_event: &str,
    bg_color: &str,
    text_color: &str,
) -> ui::Element {
    let icon = ui::Element::new(ui::ElementType::Svg, Some(&icon_svg))
        .width(22)
        .height(22)
        .text_color(text_color);

    let mut text_col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .child(
            ui::Element::new(ui::ElementType::P, Some(title))
                .size(15)
                .text_color(text_color),
        );
    if let Some(desc) = desc {
        text_col = text_col.child(
            ui::Element::new(ui::ElementType::P, Some(desc))
                .size(13)
                .text_color(text_color),
        );
    }

    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .bg(bg_color)
        .radius(12)
        .padding_left(12)
        .padding_right(12)
        .padding_top(12)
        .padding_bottom(12)
        .gap(10)
        .on(ui::Event::Click, click_event)
        .child(icon)
        .child(text_col)
}

fn build_section_header(title: &str, refresh_event: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .gap(8)
        .child(section_title(title))
        .child(ui::Element::new(ui::ElementType::Div, None).flex_grow(1.0))
        .child(ghost_button("同步").on(ui::Event::Click, refresh_event))
}

fn build_link_card(icon_svg: String, title: &str, desc: &str, event_id: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(12)
        .gap(10)
        .on(ui::Event::Click, event_id)
        .child(
            ui::Element::new(ui::ElementType::Svg, Some(&icon_svg))
                .width(22)
                .height(22),
        )
        .child(
            ui::Element::new(ui::ElementType::Div, None)
                .flex()
                .flex_direction(ui::FlexDirection::Column)
                .flex_grow(1.0)
                .child(
                    ui::Element::new(ui::ElementType::P, Some(title))
                        .size(15)
                        .text_color(COLOR_TEXT_PRIMARY),
                )
                .child(
                    ui::Element::new(ui::ElementType::P, Some(desc))
                        .size(13)
                        .text_color(COLOR_TEXT_SECONDARY),
                ),
        )
        .child(
            ui::Element::new(ui::ElementType::Svg, Some(&icons::more_link_svg()))
                .width(18)
                .height(18)
                .text_color("#0088FF"),
        )
}

fn build_info_row(icon_svg: String, label: &str, value: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(12)
        .gap(10)
        .child(
            ui::Element::new(ui::ElementType::Svg, Some(&icon_svg))
                .width(22)
                .height(22),
        )
        .child(
            ui::Element::new(ui::ElementType::P, Some(label))
                .size(14)
                .text_color(COLOR_TEXT_PRIMARY)
                .flex_grow(1.0),
        )
        .child(
            ui::Element::new(ui::ElementType::P, Some(value))
                .size(13)
                .text_color("#BBBBBB"),
        )
}

fn primary_button(text: &str, event_id: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Button, Some(text))
        .without_default_styles()
        .padding_left(16)
        .padding_right(16)
        .padding_top(9)
        .padding_bottom(9)
        .radius(10)
        .bg(COLOR_ACCENT)
        .text_color("#FFFFFF")
        .size(13)
        .on(ui::Event::Click, event_id)
}

/// 通栏主按钮（激活页用）
fn primary_button_full(text: &str, event_id: &str) -> ui::Element {
    let mut btn = primary_button(text, event_id);
    btn = btn.width_full();
    btn
}

fn ghost_button(text: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Button, Some(text))
        .without_default_styles()
        .padding_left(12)
        .padding_right(12)
        .padding_top(8)
        .padding_bottom(8)
        .radius(10)
        .bg(COLOR_CARD_BG_2)
        .text_color(COLOR_TEXT_SECONDARY)
        .size(12)
}

fn danger_button(text: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Button, Some(text))
        .without_default_styles()
        .padding_left(14)
        .padding_right(14)
        .padding_top(8)
        .padding_bottom(8)
        .radius(10)
        .bg(COLOR_DANGER_BG)
        .text_color(COLOR_TEXT_DANGER)
        .size(12)
}

fn title_text(text: &str, size: u32) -> ui::Element {
    ui::Element::new(ui::ElementType::P, Some(text))
        .size(size)
        .text_color("#FFFFFF")
}

fn section_text(text: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::P, Some(text))
        .size(15)
        .text_color(COLOR_TEXT_PRIMARY)
}

fn section_title(text: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::P, Some(text))
        .size(13)
        .text_color("#888888")
        .margin_left(4)
        .margin_top(8)
}

fn hint_text(text: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::P, Some(text))
        .size(12)
        .text_color(COLOR_TEXT_MUTED)
        .margin_top(8)
}

fn detail_text(text: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::P, Some(text))
        .size(12)
        .text_color(COLOR_TEXT_SECONDARY)
}

fn section_spacer() -> ui::Element {
    ui::Element::new(ui::ElementType::Div, None).height(8)
}

// ========== 数值/时间工具 ==========

/// 金额字段可能是字符串或数字，统一转字符串
fn amount_str(result: &serde_json::Value, key: &str) -> String {
    match result.get(key) {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// 大数字缩写（K/M/B），与手表端 formatNumber 风格一致
fn format_number(num: u64) -> String {
    if num >= 1_000_000_000 {
        format!("{:.1}B", num as f64 / 1_000_000_000.0)
    } else if num >= 1_000_000 {
        format!("{:.1}M", num as f64 / 1_000_000.0)
    } else if num >= 1_000 {
        format!("{:.1}K", num as f64 / 1_000.0)
    } else {
        num.to_string()
    }
}

/// 毫秒时间戳 → 北京时间 `YYYY-MM-DD HH:MM`
fn format_timestamp(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let secs = ms / 1000 + 8 * 3600;
    let day = secs.div_euclid(86400);
    let tod = secs.rem_euclid(86400);
    let (year, month, date) = civil_from_days(day);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        year,
        month,
        date,
        tod / 3600,
        (tod % 3600) / 60
    )
}

/// Howard Hinnant 的天数转公历日期算法
fn civil_from_days(z0: i64) -> (i64, u32, u32) {
    let z = z0 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let date = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    if month <= 2 {
        year += 1;
    }
    (year, month, date)
}

/// 构建环境变量里的 UTC ISO 时间转北京时间字符串
fn build_time_text() -> String {
    format_beijing_time(option_env!("AB_BUILD_TIME").unwrap_or("unknown"))
}

fn format_beijing_time(raw: &str) -> String {
    if let Some((y, m, d, hh, mm, ss)) = parse_iso_utc(raw) {
        let (y2, m2, d2, hh2) = add_hours(y, m, d, hh, 8);
        return format!(
            "{:04}‑{:02}‑{:02}_{:02}:{:02}:{:02}",
            y2, m2, d2, hh2, mm, ss
        );
    }
    raw.to_string()
}

fn parse_iso_utc(raw: &str) -> Option<(i32, i32, i32, i32, i32, i32)> {
    if raw.len() < 19 {
        return None;
    }
    let base = &raw[..19];
    let mut parts = base.split('T');
    let date = parts.next()?;
    let time = parts.next()?;
    let mut dparts = date.split('-');
    let y: i32 = dparts.next()?.parse().ok()?;
    let m: i32 = dparts.next()?.parse().ok()?;
    let d: i32 = dparts.next()?.parse().ok()?;
    let mut tparts = time.split(':');
    let hh: i32 = tparts.next()?.parse().ok()?;
    let mm: i32 = tparts.next()?.parse().ok()?;
    let ss: i32 = tparts.next()?.parse().ok()?;
    Some((y, m, d, hh, mm, ss))
}

fn add_hours(mut y: i32, mut m: i32, mut d: i32, mut hh: i32, add: i32) -> (i32, i32, i32, i32) {
    hh += add;
    while hh >= 24 {
        hh -= 24;
        d += 1;
        let dim = days_in_month(y, m);
        if d > dim {
            d = 1;
            m += 1;
            if m > 12 {
                m = 1;
                y += 1;
            }
        }
    }
    (y, m, d, hh)
}

fn days_in_month(y: i32, m: i32) -> i32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(y) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn is_leap_year(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

fn short_git_hash(hash: &str) -> String {
    let trimmed = hash.trim();
    if trimmed.is_empty() || trimmed == "unknown" {
        return "unknown".to_string();
    }
    trimmed.chars().take(7).collect()
}

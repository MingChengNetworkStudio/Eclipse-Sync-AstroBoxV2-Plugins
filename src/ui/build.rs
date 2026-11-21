// V4 的宿主 UI 接口：元素构建为同步调用，数据全部从 state 快照读取
use astrobox_ng_wit::astrobox::psys_host_v4::ui;

use crate::ui::chat;
use crate::ui::event_handler::*;
use crate::ui::icons;
use crate::ui::state::{self, ChatView, MainTab, VerificationStatus};

// ========== 颜色常量 ==========

const COLOR_TEXT_PRIMARY: &str = "#f4f4f5";
const COLOR_TEXT_SECONDARY: &str = "#a1a1aa";
const COLOR_TEXT_MUTED: &str = "#71717a";
const COLOR_TEXT_DANGER: &str = "#f87171";
const COLOR_BTN_PRIMARY_BG: &str = "#2563eb";
const COLOR_BTN_GHOST_BG: &str = "#27272a";
const COLOR_BTN_DANGER_BG: &str = "#3f1d1d";
const COLOR_CARD_BG: &str = "#1E1E1F";
const COLOR_BUBBLE_USER: &str = "#2563eb";
const COLOR_BUBBLE_ASSISTANT: &str = "#2A2A2A";

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
        MainTab::Settings => build_settings_tab(),
        MainTab::About => build_about_tab(),
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

    // 聊天/设置图标待用户提供 SVG，暂用纯文字触发器；关于页沿用现成图标
    let chat_trigger = build_tab_trigger(
        "聊天",
        None,
        current_tab == MainTab::Chat,
        TAB_CHAT_EVENT,
    );
    let settings_trigger = build_tab_trigger(
        "设置",
        None,
        current_tab == MainTab::Settings,
        TAB_SETTINGS_EVENT,
    );
    let about_trigger = build_tab_trigger(
        "关于",
        Some(icons::about_tab_svg()),
        current_tab == MainTab::About,
        TAB_ABOUT_EVENT,
    );

    tabs_root.child(
        tabs_list
            .child(chat_trigger)
            .child(settings_trigger)
            .child(about_trigger),
    )
}

fn build_tab_trigger(
    label: &str,
    icon_svg: Option<String>,
    is_active: bool,
    event_id: &str,
) -> ui::Element {
    let mut trigger = ui::Element::new(ui::ElementType::TabsTrigger, None)
        .without_default_styles()
        .on(ui::Event::Click, event_id)
        .radius(999)
        .padding_top(10)
        .padding_bottom(10)
        .padding_left(14)
        .padding_right(14)
        .bg(if is_active { "#2A2A2A" } else { COLOR_CARD_BG })
        .text_color(if is_active { "#FFFFFF" } else { "#BBBBBB" })
        .flex()
        .align_center()
        .gap(5);

    if let Some(svg) = icon_svg {
        let icon = ui::Element::new(ui::ElementType::Svg, Some(&svg))
            .width(22)
            .height(22);
        trigger = trigger.child(icon);
    }
    trigger.child(ui::Element::new(ui::ElementType::Span, Some(label)).size(14))
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
            .child(primary_button("开始验证", ACT_START_EVENT)),

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
            .child(section_text("付费版激活"))
            .child(primary_button("前往支付页面", ACT_OPEN_VERIFY_EVENT))
            .child(hint_text("支付完成后点击下方按钮验证"))
            .child(primary_button("验证支付", ACT_VERIFY_PAID_EVENT))
            .child(section_text("免费版激活"))
            .child(hint_text("免费版有请求次数限制"))
            .child(primary_button("免费激活", ACT_VERIFY_FREE_EVENT)),

        VerificationStatus::Verifying => root
            .child(title_text("验证中，请稍后", 24))
            .child(hint_text("正在验证订单并下发 Key 至设备")),

        VerificationStatus::Verified => root
            .child(title_text("一切就绪！", 24))
            .child(hint_text("开始使用吧")),

        VerificationStatus::Failed => root
            .child(title_text("验证失败", 24))
            .child(hint_text("请检查设备连接与网络后重试"))
            .child(primary_button("重试", ACT_RETRY_EVENT)),
    }
}

/// 激活页使用的主机设备卡片（name + 蓝牙地址）
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
                .child(ui::Element::new(ui::ElementType::P, Some(&display_name)).size(15).text_color(COLOR_TEXT_PRIMARY))
                .child(ui::Element::new(ui::ElementType::P, Some(&addr)).size(12).text_color(COLOR_TEXT_MUTED)),
        )
}

// ========== 聊天 Tab ==========

fn build_chat_tab() -> ui::Element {
    let view = state::with_state(|s| s.chat_view);
    let mut col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(16);

    col = col.child(build_chat_device_card());
    match view {
        ChatView::SessionList => col = col.child(build_session_list()),
        ChatView::Chat => col = col.child(build_chat_view()),
    }
    col
}

/// 聊天页顶部设备卡片：设备名/地址 + 刷新按钮
fn build_chat_device_card() -> ui::Element {
    let (name, addr, loading) = state::with_state(|s| {
        let (name, addr) = s
            .devices
            .first()
            .cloned()
            .map(|(addr, name)| (name, addr))
            .unwrap_or_default();
        (name, addr, s.devices_loading)
    });

    let title = if name.is_empty() { "未连接设备".to_string() } else { name };
    let sub = if addr.is_empty() {
        "请在 AstroBox 中连接手表/手环".to_string()
    } else {
        addr
    };
    let refresh_text = if loading { "刷新中…" } else { "刷新设备" };

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
                .child(ui::Element::new(ui::ElementType::P, Some(&title)).size(15).text_color(COLOR_TEXT_PRIMARY))
                .child(ui::Element::new(ui::ElementType::P, Some(&sub)).size(12).text_color(COLOR_TEXT_MUTED)),
        )
        .child(ghost_button(refresh_text).on(ui::Event::Click, REFRESH_DEVICES_EVENT))
}

/// 会话列表视图
fn build_session_list() -> ui::Element {
    let (sessions, loading) = state::with_state(|s| (s.sessions.clone(), s.sessions_loading));

    let header = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .gap(8)
        .child(section_title("会话列表"))
        .child(ui::Element::new(ui::ElementType::Div, None).flex_grow(1.0))
        .child(ghost_button("同步").on(ui::Event::Click, CHAT_REFRESH_EVENT))
        .child(primary_button("新建", CHAT_NEW_EVENT));

    let mut col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(8)
        .child(header);

    if sessions.is_empty() {
        let hint = if loading { "正在从手表加载会话…" } else { "暂无会话，点击「新建」开始第一次对话" };
        col = col.child(hint_text(hint));
        return col;
    }

    for (idx, session) in sessions.iter().enumerate() {
        col = col.child(build_session_row(session, idx));
    }
    col
}

/// 单条会话行：打开按钮（占满宽度）+ 删除按钮
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
        .bg(COLOR_BTN_GHOST_BG)
        .text_color(COLOR_TEXT_PRIMARY)
        .size(14)
        .on(
            ui::Event::Click,
            &format!("{}{}", CHAT_OPEN_PREFIX, idx),
        );

    let time_label = ui::Element::new(ui::ElementType::P, Some(&time))
        .size(11)
        .text_color(COLOR_TEXT_MUTED);

    let info_col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .gap(4)
        .flex_grow(1.0)
        .child(open_btn)
        .child(time_label);

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

/// 对话视图：返回/供应商/模型 + 消息滚动区 + 输入区
fn build_chat_view() -> ui::Element {
    let (title, has_older, messages_loading, sending) = state::with_state(|s| {
        let title = s
            .sessions
            .iter()
            .find(|sess| chat::session_id(sess) == s.current_session_id.clone().unwrap_or_default())
            .map(chat::session_title)
            .unwrap_or_else(|| "对话".to_string());
        (title, s.older_chunks > 0, s.messages_loading, s.sending)
    });

    let back_row = ui::Element::new(ui::ElementType::Div, None)
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
        );

    let mut col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(10)
        .child(back_row)
        .child(build_provider_select())
        .child(build_model_select());

    // 加载更早分块入口
    if has_older {
        let older_text = if messages_loading { "加载中…" } else { "加载更早消息" };
        col = col.child(ghost_button(older_text).on(ui::Event::Click, CHAT_LOAD_OLDER_EVENT));
    }

    col = col.child(build_message_scroll());
    col = col.child(build_input_area(sending));
    col
}

/// 供应商下拉：官方中转 + 自定义供应商
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
        .on(ui::Event::Change, CHAT_PROVIDER_EVENT)
        .radius(8)
        .padding(10)
        .bg("#2A2A2A")
        .size(14)
        .width_full();
    for label in &options {
        select = select.child(
            ui::Element::new(ui::ElementType::Option, Some(label)).prop("value", label),
        );
    }
    select
}

/// 模型下拉：当前供应商的模型列表
fn build_model_select() -> ui::Element {
    let (models, selected) = state::with_state(|s| {
        let selected = if s.selected_model.is_empty() {
            s.active_provider.active_model.clone()
        } else {
            s.selected_model.clone()
        };
        (s.active_provider.models.clone(), selected)
    });

    let current = if selected.is_empty() { "暂无可用模型".to_string() } else { selected };
    let mut select = ui::Element::new(ui::ElementType::Select, Some(&current))
        .on(ui::Event::Change, CHAT_MODEL_EVENT)
        .radius(8)
        .padding(10)
        .bg("#2A2A2A")
        .size(14)
        .width_full();

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

/// 消息滚动区：空消息显示欢迎页，否则渲染气泡 + 流式占位
fn build_message_scroll() -> ui::Element {
    let (messages, sending, streaming) = state::with_state(|s| {
        (s.messages.clone(), s.sending, s.streaming_text.clone())
    });

    let mut scroll = ui::Element::new(ui::ElementType::ScrollArea, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .gap(10)
        .width_full()
        .height(420)
        .padding(8);

    if messages.is_empty() && !sending {
        return scroll.child(build_welcome());
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

/// 无消息时的欢迎卡片
fn build_welcome() -> ui::Element {
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .align_center()
        .gap(10)
        .padding_top(60)
        .child(ui::Element::new(ui::ElementType::P, Some("你好，我是 Eclipse")).size(22).text_color(COLOR_TEXT_PRIMARY))
        .child(ui::Element::new(ui::ElementType::P, Some("在下方输入消息即可开始对话")).size(13).text_color(COLOR_TEXT_SECONDARY))
        .child(ui::Element::new(ui::ElementType::P, Some("对话消息会自动同步回手表")).size(13).text_color(COLOR_TEXT_MUTED))
        .child(ui::Element::new(ui::ElementType::P, Some("可在上方切换供应商与模型")).size(13).text_color(COLOR_TEXT_MUTED))
}

/// 单条消息气泡：user 右蓝、assistant 左灰
fn build_bubble(role: &str, content: &str) -> ui::Element {
    let is_user = role == "user";
    let bg = if is_user { COLOR_BUBBLE_USER } else { COLOR_BUBBLE_ASSISTANT };
    let text_color = if is_user { "#FFFFFF" } else { COLOR_TEXT_PRIMARY };

    let bubble = ui::Element::new(ui::ElementType::Div, None)
        .radius(12)
        .padding(12)
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

/// 底部输入区：输入框 + 发送/停止按钮
fn build_input_area(sending: bool) -> ui::Element {
    let draft = state::with_state(|s| s.draft.clone());
    let input = ui::Element::new(ui::ElementType::Input, Some(&draft))
        .prop("type", "text")
        .prop("placeholder", "输入消息…")
        .flex_grow(1.0)
        .padding(10)
        .radius(8)
        .bg("#2A2A2A")
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
        .child(input)
        .child(action)
}

// ========== 设置 Tab ==========

fn build_settings_tab() -> ui::Element {
    let mut col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(8);

    col = col
        .child(section_title("授权状态"))
        .child(build_apikey_card())
        .child(build_balance_card())
        .child(build_relay_usage_card())
        .child(build_device_info_card())
        .child(primary_button("刷新授权信息", SET_REFRESH_AUTH_EVENT))
        .child(build_revoke_card())
        .child(section_title("构建信息"));

    // 构建信息沿用关于页的数据行
    col = col
        .child(build_info_row(icons::time_svg(), "构建时间", &build_time_text()))
        .child(build_info_row(icons::user_svg(), "构建用户", option_env!("AB_BUILD_USER").unwrap_or("unknown")))
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
        .padding(12)
        .gap(6)
        .on(ui::Event::Click, SET_TOGGLE_KEY_EVENT)
        .child(
            ui::Element::new(ui::ElementType::Div, None)
                .flex()
                .flex_direction(ui::FlexDirection::Row)
                .align_center()
                .width_full()
                .child(ui::Element::new(ui::ElementType::P, Some("APIKey 状态")).size(15).text_color(COLOR_TEXT_PRIMARY))
                .child(ui::Element::new(ui::ElementType::Div, None).flex_grow(1.0))
                .child(
                    ui::Element::new(ui::ElementType::P, Some(if verified { "已验证" } else { "未验证" }))
                        .size(15)
                        .text_color(if verified { "#4ade80" } else { COLOR_TEXT_DANGER }),
                ),
        )
        .child(ui::Element::new(ui::ElementType::P, Some(&display)).size(14).text_color(COLOR_TEXT_SECONDARY))
        .child(ui::Element::new(ui::ElementType::P, Some(hint)).size(11).text_color(COLOR_TEXT_MUTED))
}

/// 余额卡：usedAmount/totalAmount + 进度条（字段缺失时给提示）
fn build_balance_card() -> ui::Element {
    let info = state::with_state(|s| s.server_info.clone());
    let mut card = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(12)
        .gap(8);

    let Some(result) = info.as_ref().and_then(|v| v.get("result")) else {
        return card.child(hint_text("暂无余额信息，请点击「刷新授权信息」"));
    };

    let used = amount_str(result, "usedAmount");
    let total = amount_str(result, "totalAmount");
    let title = if used.is_empty() || total.is_empty() {
        "用量信息暂不可用".to_string()
    } else {
        format!("{} / {} 元", used, total)
    };

    card = card.child(
        ui::Element::new(ui::ElementType::Div, None)
            .flex()
            .flex_direction(ui::FlexDirection::Row)
            .align_center()
            .width_full()
            .child(ui::Element::new(ui::ElementType::P, Some("用量信息")).size(15).text_color(COLOR_TEXT_PRIMARY))
            .child(ui::Element::new(ui::ElementType::Div, None).flex_grow(1.0))
            .child(ui::Element::new(ui::ElementType::P, Some(&title)).size(14).text_color(COLOR_TEXT_SECONDARY)),
    );

    if let (Ok(used_num), Ok(total_num)) = (used.parse::<f64>(), total.parse::<f64>()) {
        if total_num > 0.0 {
            let percent = ((used_num / total_num) * 100.0).min(100.0) as u32;
            card = card.child(
                ui::Element::new(ui::ElementType::Progress, None)
                    .width_full()
                    .height(15)
                    .prop("value", &percent.to_string()),
            );
        }
    }

    // 防御性展示后端可能返回的其他计费字段
    for (field, label) in [
        ("userType", "用户类型"),
        ("billingMode", "计费模式"),
        ("expiredAt", "到期时间"),
        ("remainingAmount", "剩余金额"),
    ] {
        if let Some(value) = result.get(field).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            card = card.child(detail_text(&format!("{}: {}", label, value)));
        }
    }
    card
}

/// AI 中转用量卡：总调用/token/费用 + 分模型明细
fn build_relay_usage_card() -> ui::Element {
    let Some(usage) = state::with_state(|s| s.relay_usage.clone()) else {
        return ui::Element::new(ui::ElementType::Div, None)
            .flex()
            .flex_direction(ui::FlexDirection::Column)
            .width_full()
            .bg(COLOR_CARD_BG)
            .radius(12)
            .padding(12)
            .gap(6)
            .child(section_text("AI 中转用量"))
            .child(hint_text("暂无数据（自定义供应商或未开通中转服务时不可用）"));
    };

    let mut card = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding(12)
        .gap(6)
        .child(section_text("AI 中转用量"))
        .child(detail_text(&format!(
            "调用 {} 次 · 费用 {} 元",
            usage.get("calls").and_then(|v| v.as_u64()).unwrap_or(0),
            usage.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0)
        )))
        .child(detail_text(&format!(
            "输入 {} · 输出 {} Tokens",
            usage.get("inputTokens").and_then(|v| v.as_u64()).unwrap_or(0),
            usage.get("outputTokens").and_then(|v| v.as_u64()).unwrap_or(0)
        )));

    if let Some(models) = usage.get("byModel").and_then(|v| v.as_array()) {
        for item in models {
            let model = item.get("model").and_then(|v| v.as_str()).unwrap_or("未知模型");
            let line = format!(
                "{}：{} 次，{} 元，入 {} / 出 {} Tokens",
                model,
                item.get("calls").and_then(|v| v.as_u64()).unwrap_or(0),
                item.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0),
                item.get("inputTokens").and_then(|v| v.as_u64()).unwrap_or(0),
                item.get("outputTokens").and_then(|v| v.as_u64()).unwrap_or(0),
            );
            card = card.child(detail_text(&line));
        }
    }
    card
}

/// 设备信息卡（来自手表 GET_DEVICEINFO）
fn build_device_info_card() -> ui::Element {
    let Some(info) = state::with_state(|s| s.device_info.clone()) else {
        return ui::Element::new(ui::ElementType::Div, None)
            .flex()
            .flex_direction(ui::FlexDirection::Column)
            .width_full()
            .bg(COLOR_CARD_BG)
            .radius(12)
            .padding(12)
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
        .padding(12)
        .gap(6)
        .child(section_text("设备信息"));

    for (label, key) in fields {
        let value = info.get(key).and_then(|v| v.as_str()).unwrap_or("-");
        card = card.child(detail_text(&format!("{}: {}", label, value)));
    }
    card
}

/// 撤销授权红色卡
fn build_revoke_card() -> ui::Element {
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .bg("#3f1d1d")
        .radius(12)
        .padding(14)
        .on(ui::Event::Click, SET_REVOKE_EVENT)
        .child(
            ui::Element::new(ui::ElementType::P, Some("撤销授权"))
                .size(15)
                .text_color(COLOR_TEXT_DANGER)
                .flex_grow(1.0),
        )
        .child(detail_text("删除本地 Key 与缓存"))
}

// ========== 关于 Tab ==========

fn build_about_tab() -> ui::Element {
    let help_card = build_link_card(
        icons::help_svg(),
        "帮助文档",
        "操作步骤与常见问题解答",
        OPEN_HELP_DOC_EVENT,
    );
    let qq_card = build_link_card(
        icons::qq_group_svg(),
        "QQ交流群",
        "1076096725",
        OPEN_QQ_GROUP_EVENT,
    );

    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(8)
        .child(section_title("更多内容"))
        .child(help_card)
        .child(qq_card)
        .child(section_title("构建信息"))
        .child(build_info_row(icons::time_svg(), "构建时间", &build_time_text()))
        .child(build_info_row(icons::user_svg(), "构建用户", option_env!("AB_BUILD_USER").unwrap_or("unknown")))
        .child(build_info_row(
            icons::branch_svg(),
            "当前分支",
            option_env!("AB_BUILD_GIT_BRANCH").unwrap_or("unknown"),
        ))
        .child(build_info_row(
            icons::hash_svg(),
            "当前hash",
            &short_git_hash(option_env!("AB_BUILD_GIT_HASH").unwrap_or("unknown")),
        ))
}

fn build_link_card(icon_svg: String, title: &str, desc: &str, event_id: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .bg(COLOR_CARD_BG)
        .radius(12)
        .padding_left(12)
        .padding_right(12)
        .padding_top(12)
        .padding_bottom(12)
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
                .child(ui::Element::new(ui::ElementType::P, Some(title)).size(15).text_color(COLOR_TEXT_PRIMARY))
                .child(ui::Element::new(ui::ElementType::P, Some(desc)).size(13).text_color(COLOR_TEXT_SECONDARY)),
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
        .child(ui::Element::new(ui::ElementType::P, Some(value)).size(13).text_color("#BBBBBB"))
}

// ========== 基础组件 ==========

fn primary_button(text: &str, event_id: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Button, Some(text))
        .without_default_styles()
        .width_full()
        .padding_top(10)
        .padding_bottom(10)
        .radius(8)
        .bg(COLOR_BTN_PRIMARY_BG)
        .text_color(COLOR_TEXT_PRIMARY)
        .size(14)
        .on(ui::Event::Click, event_id)
}

fn ghost_button(text: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Button, Some(text))
        .without_default_styles()
        .padding_left(12)
        .padding_right(12)
        .padding_top(8)
        .padding_bottom(8)
        .radius(8)
        .bg(COLOR_BTN_GHOST_BG)
        .text_color(COLOR_TEXT_SECONDARY)
        .size(13)
}

fn danger_button(text: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Button, Some(text))
        .without_default_styles()
        .padding_left(14)
        .padding_right(14)
        .padding_top(8)
        .padding_bottom(8)
        .radius(8)
        .bg(COLOR_BTN_DANGER_BG)
        .text_color(COLOR_TEXT_DANGER)
        .size(13)
}

fn title_text(text: &str, size: u32) -> ui::Element {
    ui::Element::new(ui::ElementType::P, Some(text))
        .size(size)
        .text_color("#FFFFFF")
}

fn section_text(text: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::P, Some(text))
        .size(16)
        .text_color(COLOR_TEXT_PRIMARY)
}

fn section_title(text: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::P, Some(text))
        .size(13)
        .text_color("#888888")
        .margin_left(12)
        .margin_top(10)
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

/// 毫秒时间戳 → 北京时间 `YYYY-MM-DD HH:MM`
fn format_timestamp(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let secs = ms / 1000 + 8 * 3600; // 转 UTC+8
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

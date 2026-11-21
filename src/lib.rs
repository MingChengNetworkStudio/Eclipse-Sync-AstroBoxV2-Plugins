// V4 插件侧导出：lifecycle / event 均为原生 async func
// 宿主 ui 模块别名 host_ui，避免与本 crate 的 pub mod ui 同名冲突
use astrobox_ng_wit::astrobox::psys_host_v4::{register, ui as host_ui};
use astrobox_ng_wit::exports::astrobox::psys_plugin_v4::{
    event::{self, EventType},
    lifecycle,
};

pub mod logger;
pub mod ui;

struct MyPlugin;

impl event::Guest for MyPlugin {
    async fn on_event(event_type: EventType, event_payload: String) -> String {
        match event_type {
            EventType::InterconnectMessage => {
                ui::handle_interconnect_message(&event_payload).await;
            }
            EventType::DeviceAction => {
                // 设备连接/断开：立即刷新设备列表
                ui::event_handler::refresh_device_list().await;
            }
            _ => {}
        }

        // V4 导出直接返回结果；空字符串表示无业务返回值
        String::new()
    }

    async fn on_ui_event(
        event_id: String,
        ev: host_ui::Event,
        event_payload: String,
    ) -> String {
        ui::ui_event_processor(ev, &event_id, &event_payload).await;
        String::new()
    }

    async fn on_ui_render(element_id: String) {
        ui::render_main_ui(&element_id).await;
    }

    async fn on_card_render(card_id: String) {
        render_status_card(&card_id);
    }
}

impl lifecycle::Guest for MyPlugin {
    async fn on_load() {
        logger::init();
        tracing::info!("Eclipse同步器插件加载");

        // 注册状态文本卡片，失败时记录原因
        if let Err(reason) = register::register_card(
            register::CardType::Text,
            crate::ui::STATUS_CARD_ID.into(),
            crate::ui::STATUS_CARD_NAME.into(),
        )
        .await
        {
            tracing::warn!("注册状态卡片失败: {reason}");
        }

        // 从磁盘恢复激活状态
        let config = ui::persist::load_config();
        ui::state::restore_from_disk(config);

        // 预填充设备列表
        ui::event_handler::initial_refresh().await;

        // 已激活则后台拉取授权信息与用量（网络超时不阻塞插件加载）
        if ui::state::is_activated() {
            astrobox_ng_wit::spawn(async move {
                ui::activation::fetch_get_info().await;
                ui::activation::fetch_relay_usage().await;
            });
        }
    }
}

/// 渲染状态卡片：激活状态 + 设备数 + 会话数摘要
fn render_status_card(card_id: &str) {
    let activated = ui::state::is_activated();
    let device_count = ui::state::with_state(|s| s.devices.len());
    let session_count = ui::state::with_state(|s| s.sessions.len());
    let auth_text = if activated { "已激活" } else { "未激活" };

    let text = format!(
        "{}  ·  设备: {} 台  ·  会话: {}",
        auth_text, device_count, session_count
    );
    host_ui::render_to_text_card(card_id, &text);
}

astrobox_ng_wit::export!(MyPlugin);

pub mod activation;
pub mod api_client;
pub mod build;
pub mod chat;
pub mod event_handler;
pub mod files;
pub mod icons;
pub mod persist;
pub mod prompts;
pub mod protocol;
pub mod state;

pub use build::render_main_ui;
pub use event_handler::handle_interconnect_message;
pub use event_handler::ui_event_processor;

/// Eclipse同步器文本卡片 ID
pub const STATUS_CARD_ID: &str = "eclipse-sync-status";
/// Eclipse同步器文本卡片名称
pub const STATUS_CARD_NAME: &str = "Eclipse同步器 · 状态";

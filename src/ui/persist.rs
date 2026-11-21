use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// 持久化配置文件路径（插件私有工作目录下）
const CONFIG_PATH: &str = "./eclipse-sync.config.json";
pub const CURRENT_VERSION: u32 = 1;

/// 落盘配置：目前只保存激活 APIKey，其余聊天数据均以手表端为准
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct OnDiskConfig {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub api_key: String,
}

/// 从磁盘加载配置，文件缺失或损坏时返回默认值
pub fn load_config() -> OnDiskConfig {
    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return OnDiskConfig::default();
    }

    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(err) => {
            tracing::warn!("persist: 读取 {} 失败: {}", CONFIG_PATH, err);
            return OnDiskConfig::default();
        }
    };

    match serde_json::from_str::<OnDiskConfig>(&text) {
        Ok(c) => c,
        Err(err) => {
            tracing::warn!("persist: 解析 {} 失败: {}", CONFIG_PATH, err);
            OnDiskConfig::default()
        }
    }
}

/// 原子写入配置到磁盘（先写 tmp 再 rename）
pub fn save_config(config: &OnDiskConfig) {
    let text = match serde_json::to_string_pretty(config) {
        Ok(t) => t,
        Err(err) => {
            tracing::error!("persist: 配置序列化失败: {}", err);
            return;
        }
    };

    let tmp_path = format!("{}.tmp", CONFIG_PATH);
    if let Err(err) = fs::write(&tmp_path, &text) {
        tracing::error!("persist: 写入 {} 失败: {}", tmp_path, err);
        return;
    }
    if let Err(err) = fs::rename(&tmp_path, CONFIG_PATH) {
        tracing::warn!("persist: rename 失败（{}），回退为直接写入", err);
        if let Err(err) = fs::write(CONFIG_PATH, &text) {
            tracing::error!("persist: 直接写入也失败: {}", err);
        }
    }
}

/// 撤销授权时删除配置文件（不存在也视为成功）
pub fn delete_config() {
    let path = Path::new(CONFIG_PATH);
    if path.exists() {
        if let Err(err) = fs::remove_file(path) {
            tracing::error!("persist: 删除 {} 失败: {}", CONFIG_PATH, err);
        }
    }
}

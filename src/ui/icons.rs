//! SVG 图标集合。
//! 设备/关于类图标为项目原有；聊天/文件/AI 设置类图标复制自同作者的
//! 永昼天气同步器（EternalAstroboxPlugins）icons.rs，保持 256 viewBox、currentColor 风格统一。

/// 设备图标（手表/手环轮廓）
pub fn device_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024"><path d="M853.33 512c0-106.67-51.2-204.8-128-268.8L682.67 0 341.33 0l-42.66 243.2C221.87 307.2 170.67 405.33 170.67 512s51.2 204.8 128 268.8L341.33 1024l341.34 0 42.66-243.2C802.13 716.8 853.33 618.67 853.33 512zM256 512c0-140.8 115.2-256 256-256 140.8 0 256 115.2 256 256s-115.2 256-256 256C371.2 768 256 652.8 256 512z" fill="currentColor"/></svg>"#.to_string()
}

/// 关于 Tab 图标（信息圆圈）
pub fn about_tab_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024"><path d="M509.92 795.84c157.904 0 285.92-128.016 285.92-285.92C795.84 352 667.808 224 509.92 224 352 224 224 352 224 509.92c0 157.904 128 285.92 285.92 285.92z m0 48C325.504 843.84 176 694.32 176 509.92 176 325.504 325.504 176 509.92 176c184.416 0 333.92 149.504 333.92 333.92 0 184.416-149.504 333.92-333.92 333.92z m-2.448-400.704h16a16 16 0 0 1 16 16v201.728a16 16 0 0 1-16 16h-16a16 16 0 0 1-16-16V459.136a16 16 0 0 1 16-16z m0-100.176h16a16 16 0 0 1 16 16v23.648a16 16 0 0 1-16 16h-16a16 16 0 0 1-16-16v-23.648a16 16 0 0 1 16-16z" fill="currentColor"/></svg>"#.to_string()
}

/// 帮助文档图标（书本）
pub fn help_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" fill="none"/><path d="M48,216a24,24,0,0,1,24-24H208V32H72A24,24,0,0,0,48,56Z" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><polyline points="48 216 48 224 192 224" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/></svg>"#.to_string()
}

/// QQ群图标（@符号）
pub fn qq_group_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" fill="none"/><circle cx="128" cy="128" r="40" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><path d="M184,208c-15.21,10.11-36.37,16-56,16a96,96,0,1,1,96-96c0,22.09-8,40-28,40s-28-17.91-28-40V88" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/></svg>"#.to_string()
}

/// 外链箭头图标
pub fn more_link_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" fill="none"/><polyline points="216 104 215.99 40.01 152 40" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><line x1="136" y1="120" x2="216" y2="40" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><path d="M184,136v72a8,8,0,0,1-8,8H48a8,8,0,0,1-8-8V80a8,8,0,0,1,8-8h72" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/></svg>"#.to_string()
}

/// 时间图标
pub fn time_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" fill="none"/><circle cx="128" cy="128" r="96" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><polyline points="128 72 128 128 184 128" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/></svg>"#.to_string()
}

/// 用户图标
pub fn user_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" fill="none"/><circle cx="128" cy="136" r="32" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><path d="M80,192a60,60,0,0,1,96,0" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><rect x="32" y="48" width="192" height="160" rx="8" transform="translate(256) rotate(90)" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><line x1="96" y1="64" x2="160" y2="64" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/></svg>"#.to_string()
}

/// 分支图标
pub fn branch_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" fill="none"/><path d="M80,168V144a16,16,0,0,1,16-16h88a16,16,0,0,0,16-16V88" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><line x1="80" y1="88" x2="80" y2="168" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><circle cx="80" cy="64" r="24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><circle cx="200" cy="64" r="24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><circle cx="80" cy="192" r="24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/></svg>"#.to_string()
}

/// 井号图标
pub fn hash_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" fill="none"/><line x1="48" y1="96" x2="224" y2="96" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><line x1="176" y1="40" x2="144" y2="216" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><line x1="112" y1="40" x2="80" y2="216" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><line x1="32" y1="160" x2="208" y2="160" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/></svg>"#.to_string()
}

// ===== 以下复制自 EternalAstroboxPlugins/icons.rs（同作者资产，风格一致） =====

/// 聊天 Tab 图标（纸飞机）
pub fn chat_tab_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" fill="none"/><line x1="108" y1="148" x2="160" y2="96" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><path d="M223.69,42.18a8,8,0,0,0-9.87-9.87l-192,58.22a8,8,0,0,0-1.25,14.93L108,148l42.54,87.42a8,8,0,0,0,14.93-1.25Z" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/></svg>"#.to_string()
}

/// 设置 Tab 图标（调节滑块）
pub fn settings_tab_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" fill="none"/><line x1="184" y1="80" x2="216" y2="80" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><line x1="40" y1="80" x2="152" y2="80" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><line x1="120" y1="176" x2="216" y2="176" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><line x1="40" y1="176" x2="88" y2="176" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><line x1="152" y1="56" x2="152" y2="104" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><line x1="88" y1="152" x2="88" y2="200" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/></svg>"#.to_string()
}

/// AI 设置 Tab 图标（齿轮）
pub fn ai_settings_tab_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" fill="none"/><path fill="currentColor" d="m165.5 148l53.33 53.33l-16 16l-53.33-53.33v-8.43l-2.89-2.98A68.97 68.97 0 0 1 101.5 170A69.33 69.33 0 0 1 32 101.33A69.33 69.33 0 0 1 101.5 32A69.33 69.33 0 0 1 171 101.33c0 17.17-6.27 32.98-16.76 45.12l2.98 2.89zm-64-101.33l-5.87.32c-2.56 5.55-6.49 14.93-9.37 26.35h30.48c-2.88-11.42-6.81-20.8-9.37-26.35c-1.92-.32-3.84-.32-5.87-.32M147.55 74.67a52.63 52.63 0 0 0-28.54-23.68c2.56 5.65 5.87 13.87 8.32 23.68zM57.45 74.67h2.03c2.45-9.81 5.76-18.03 8.32-23.68A52.63 52.63 0 0 0 57.45 74.67M51.2 101.33c0 5.33.85 10.99 2.45 16h22.82l-1.28-16l1.28-16H53.65c-1.6 5.01-2.45 10.67-2.45 16m104.03 16c1.6-5.01 2.45-10.67 2.45-16s-.85-10.99-2.45-16h-22.82c.43 5.33.43 10.67 0 16zM78.37 85.33l-1.28 16l1.28 16h34.56c.43-5.33.43-10.67 0-16zM96 149.33c1.92 0 3.84 0 5.65-.32c2.67-5.55 6.71-14.93 9.6-26.35H81.33c2.88 11.42 6.93 20.8 9.6 26.35zm46.23-26.66h-2.03c-2.45 9.81-5.76 18.03-8.32 23.68A52.63 52.63 0 0 0 147.55 122.67zm-92.22 0a52.63 52.63 0 0 0 28.54 23.68c-2.56-5.65-5.87-13.87-8.32-23.68z"/></svg>"#.to_string()
}

/// 文件管理 Tab 图标（图片/文件卡片）
pub fn files_tab_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024"><path d="M1024 910.22V113.78C1024 51.2 972.8 0 910.22 0H113.78C51.2 0 0 51.2 0 113.78v796.44C0 972.8 51.2 1024 113.78 1024h796.44C972.8 1024 1024 972.8 1024 910.22zM312.89 597.33l142.22 170.67 199.11-256L910.22 853.33H113.78l199.11-256z" fill="currentColor"/></svg>"#.to_string()
}

/// 会话历史图标（列表）
pub fn history_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" fill="none"/><line x1="40" y1="64" x2="216" y2="64" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><line x1="40" y1="128" x2="216" y2="128" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><line x1="40" y1="192" x2="216" y2="192" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/></svg>"#.to_string()
}

/// 刷新图标
pub fn refresh_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="currentColor" d="M12.077 19q-2.931 0-4.966-2.033q-2.034-2.034-2.034-4.964t2.034-4.966T12.077 5q1.783 0 3.339.847q1.555.847 2.507 2.365V5h1v5.23h-5.23v-1h3.7q-.782-1.495-2.198-2.363T12.077 6q-2.5 0-4.25 1.75T6.077 12t1.75 4.25t4.25 1.75q1.925 0 3.475-1.1t2.175-2.9h1.062q-.662 2.246-2.514 3.623T12.077 19"/></svg>"#.to_string()
}

/// 钱包/付费图标（爱发电手势，Eternal 用于剩余金额）
pub fn wallet_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" fill="none"/><path d="M48,208H16a8,8,0,0,1-8-8V160a8,8,0,0,1,8-8H48" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><path d="M112,160h32l67-15.41a16.61,16.61,0,0,1,21,16h0a16.59,16.59,0,0,1-9.18,14.85L184,192l-64,16H48V152l25-25a24,24,0,0,1,17-7H140a20,20,0,0,1,20,20h0a20,20,0,0,1-20,20Z" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><path d="M96.73,120C87,107.72,80,94.56,80,80c0-21.69,17.67-40,39.46-40A39.12,39.12,0,0,1,156,64a39.12,39.12,0,0,1,36.54-24C214.33,40,232,58.31,232,80c0,29.23-28.18,55.07-50.22,71.32" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/></svg>"#.to_string()
}

/// AI 用量/公告图标（喇叭）
pub fn usage_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect width="256" height="256" fill="none"/><path d="M224,120V72a8,8,0,0,0-10.27-7.67L39.73,111.06A16,16,0,0,0,24,126.81v10.38a16,16,0,0,0,15.73,15.75L80,153.28V184a24,24,0,0,0,24,24h0a24,24,0,0,0,24-24V161.44l85.73,22.23A8,8,0,0,0,224,176V128" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/><path d="M152,183.68V192a24,24,0,0,1-24,24h0a24,24,0,0,1-24-24V184" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="16"/></svg>"#.to_string()
}

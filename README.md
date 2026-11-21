# Eclipse同步器（EclipseSync AstroBox Plugin）

AstroBox V2 插件，为手表快应用 [Eclipse](https://docs.b4qaq.cn/docs/eclipse/) 提供手机端的**激活、流式聊天、文件管理与用量查看**能力。

- 语言/运行时：Rust → `wasm32-wasip2`，[astrobox-ng-wit](https://crates.io/crates/astrobox-ng-wit) 0.4（API Level 4 / WASI 3）
- 通信对象：Eclipse 快应用（包名 `moe.mcns.Eclipse`），基于 AstroBox 互联通道（QAIC）
- 授权服务：铭诚 API 网关 `https://api.b4qaq.cn`

## 功能

### 激活流程

首次进入显示全屏激活向导（无 Tab），与永昼天气同步器同款状态机：

1. 读取手表上已有的 APIKey：存在则可直接采用，不存在自动获取设备信息（product/deviceId/serial）
2. 浏览器打开支付页完成购买，返回后点击「验证支付」（`verifyCheck`，与手表端一致不带 type 参数）
3. 激活成功后 Key 落盘（`eclipse-sync.config.json`），并通过 `PUT_SETTINGS` 回传手表
4. `getInfo` 返回 201 时自动判定授权失效，清空本地状态回到激活页

### 聊天 Tab

- 标准 AI 应用形态：会话列表欢迎页（随机问候语）→ 对话页
- 顶部返回/会话标题、**AI 设定选择器**、模型选择器；底部输入栏固定贴屏幕下方
- **流式输出**：插件直连供应商 OpenAI 兼容 `/chat/completions`（`stream:true` SSE），delta 实时渲染，支持中途停止
- 对话完成后 `PUT_MESSAGES` 把 user/assistant 两条消息自动写回手表，手机聊完手表可见
- 会话支持新建、打开（分块加载更早消息）、删除；会话标题由手表端负责，插件不修改
- AI 设定（系统提示词）通过 `PUT_SESSION_UPDATE` 绑定到会话，发送时作为 system 消息注入

### 文件管理 Tab

- `GET_FILES_GROUPED` 按会话分组浏览手表工作区文件
- 文本文件页面内预览（`GET_SESSION_FILE`），预览态也可下载
- 下载走宿主系统保存对话框（`save-file-start/write-chunk/finish`），二进制文件由手表 base64 回传后本地解码
- 单文件上限 256KB（手表端协议限制）

### AI 设置 Tab

- **供应商**：下拉切换激活供应商（同步到手表）；支持自定义供应商的新建、编辑、删除（`PUT/DEL_PROVIDER`）
- **模型管理**：结构化的增删改控件，每行一个模型可独立编辑/删除，可随时添加
- 当前生效供应商卡片展示接口地址、当前模型与完整模型清单
- **AI 设定**：系统提示词的新建、编辑、删除（`GET/PUT/DEL_PROMPTS`），Textarea 编辑正文

### 设置 Tab

- APIKey 脱敏显示，点击切换显隐
- 账户余额：usedAmount/totalAmount 大字号展示 + 进度条，计费模式/到期时间等附加字段
- 充值入口：2 元 / 5 元 / 10 元 / 25 元四档（planOrder 1-4），点击打开对应支付页
- AI 中转用量：总调用/费用/输入输出 Tokens 指标网格 + 分模型明细卡（`/api/v2/ai/v1/usage`）
- 设备信息、刷新授权（后台执行带 loading）、撤销授权（二次确认）
- 帮助文档、QQ 交流群、构建信息（原关于页已并入此 Tab）

## 工作原理

```
┌─────────────┐   QAIC 互联消息    ┌──────────────┐   OpenAI 兼容协议   ┌────────────┐
│  Eclipse    │ ◄────────────────► │  本插件(wasm) │ ◄──────────────────► │ AI 供应商   │
│ 手表快应用   │  {type,status,data} │  AstroBox    │   SSE 流式响应       │ 官方/自定义 │
└─────────────┘                    └──────────────┘                     └────────────┘
                                          │
                                          │ 激活/用量
                                          ▼
                                   https://api.b4qaq.cn
```

- **事件驱动通信**：发起互联请求前在 PENDING 表登记 `PendingKind`，发送后立即返回；回执到达 `on_event` 时取出并分发，绝不在事件任务里阻塞等待
- **鉴权**：官方中转使用复合 Key `Authorization: Bearer Eclipse-<设备Key>`；自定义供应商使用其自身 Key
- 多设备环境固定操作第一台已连接设备

## 目录结构

```
src/
├── lib.rs                # 插件入口、生命周期、状态卡片
├── logger.rs             # tracing 日志（控制台 + logs/ 按天滚动）
└── ui/
    ├── state.rs          # 全局 PluginState（Mutex）与访问函数、随机问候
    ├── persist.rs        # eclipse-sync.config.json 原子读写（Key + 设备信息）
    ├── protocol.rs       # 互联协议客户端：PENDING 登记 + 全部指令封装
    ├── activation.rs     # 激活状态机、getInfo/usage 拉取、撤销授权
    ├── chat.rs           # 会话/供应商/AI 设定逻辑、SSE 流式对话与同步
    ├── files.rs          # 文件分组、预览、宿主保存对话框下载
    ├── prompts.rs        # AI 设定增删改
    ├── api_client.rs     # waki HTTP：普通请求、SSE 解析、gzip 解压、JSON 封装
    ├── event_handler.rs  # 互联回执分发与全部 UI 事件路由
    ├── build.rs          # UI Builder 全量组件树
    └── icons.rs          # SVG 图标（currentColor 风格）
```

## 构建

需要 Rust 工具链与 `wasm32-wasip2` target：

```bash
rustup target add wasm32-wasip2

# 调试构建（产物输出到 dist/）
python scripts/build_dist.py --target wasm32-wasip2

# release 构建
python scripts/build_dist.py --release --target wasm32-wasip2

# release 并打包 .abp
python scripts/build_dist.py --release --package --target wasm32-wasip2
```

构建脚本读取 `manifest.json` 的 `entry` 字段命名 wasm 产物，并自动注入 git 信息（`AB_BUILD_TIME` 等环境变量）。

## 安装

1. 将 `dist/Eclipse同步器.abp` 发送到手机，用 AstroBox 打开安装
2. 在 AstroBox 中连接安装了 Eclipse 快应用的手表
3. 打开插件按引导完成激活

## 许可证

[GNU AGPL v3](./LICENSE)

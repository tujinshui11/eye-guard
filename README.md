# 艾斯（Ace / EyeGuard）

Windows 桌面护眼工具：**屏幕色温调节（防蓝光）+ 亮度调节 + 定时休息提醒 + 日落跟随 + 用眼统计**。

基于 **Rust + Tauri 2**（`windows` crate 直调 gdi32 gamma ramp——硬件级色温偏移，全屏生效）。

> **v0.3.0 起更名为「艾斯」**（原「护眼助手」）。v0.2.0 起为 Rust/Tauri 重写版（旧版 Electron 实现见 git 历史，设置自动继承）。

## 功能

### 显示调节
- **7 档场景模式**：原色 6500K / 冷白提神 8000K / 办公 5500K / 阅读暖白 5000K / 傍晚 4500K / 夜晚 3400K / 深夜 2700K（色温 + 亮度组合，一键应用）
  - 预设值依据见 `docs/eye-parameters-research.md`（f.lux 实践值、办公照明 CCT 建议、昼夜节律 melanopic EDI 研究）
  - 实际可用范围受显卡驱动限制（本机实测暖端边界约 3300K），超出时自动安全钳制并在界面提示
- **色温调节**：2000K–10000K 滑块（暖 ↔ 冷全线）
- **亮度调节**：背光（WMI）主控 + 黑纱补充；台式机/外接屏无背光通道时自动降级为纯黑纱
- **多显示器**：逐屏应用（`EnumDisplayDevices` + 逐屏 DC），副屏同样生效

### 休息提醒
- 20-20-20 规则（AAO/AOA 推荐），工作/休息周期可配
- 温和卡片（不抢键盘焦点）/ 全屏遮罩 两种样式
- 支持**推迟 5 分钟** / **跳过本次** / 暂停 1 小时
- 空闲检测：人不在时工作时间顺延；全屏（游戏/演示）时自动免打扰

### 自动化
- **色彩敏感应用**：白名单应用进入前台时自动恢复原色（不干扰专业色彩判断），离开后自动恢复
- **日落跟随**：按所在城市计算日出日落，日落前 60 分钟**余弦缓入缓出**变暖、日出后回升；随季节逐日自动跟随；亮度与色温错峰过渡；手动调整后 1 小时内不覆盖
  - 定位：优先 IP 自动定位（国内库城市名 + 坐标双源），失败可手动选（10 个预设城市）；**零系统定位权限、离线可用**
- **感光监测**（默认关闭）：优先环境光传感器（ALS），无硬件时回退摄像头测光（本机计算亮度数值，不保存不传输），环境骤暗时提醒或自动切换
- **时间调度**：到点自动切换模式，或弹卡片询问；启动/睡眠唤醒自动对齐

### 用眼统计
- 今日 / 本周护眼时长、休息完成与跳过次数
- **24 小时时段分布图** + **近 7 日趋势图**（零依赖手写 SVG，随四主题自动换色）
- 数据仅存本机（`%APPDATA%\护眼助手\usage.json`，目录名沿用旧版以保证设置继承），保留 60 天自动裁剪

### 界面
- **分板块导航**：显示调节 / 休息提醒 / 自动化 / 用眼统计 / 通用
- **暖调磨砂玻璃**：无边框圆角窗口 + Windows Acrylic（实时透出桌面）
- **四主题**：深色（暖调石墨，默认）/ 浅色 / 暖橙 / 胖鱼（鲸娘风）
- 自绘下拉弹层、板块切换过渡动画（尊重系统"减少动效"偏好）

### 系统集成
- 托盘常驻：7 档模式切换 / 色温微调 / 恢复原色 / 暂停提醒 / 退出
- 全局热键：`Ctrl+Alt+↑` / `Ctrl+Alt+↓` 色温 ±200K
- 开机自启（启动后驻留托盘）
- **安全恢复**：退出自动还原屏幕原色；崩溃后下次启动自动修复（dirty 标记自愈）

## 下载与安装

从 [Releases](https://github.com/tujinshui11/eye-guard/releases) 下载（打 tag 由 CI 自动构建）：

- **NSIS 安装器**（`*setup.exe`）：双击安装，开始菜单/卸载齐全；**内置 WebView2 bootstrapper**——目标机器缺组件时自动补装
- 系统要求：Windows 10/11

## 从旧版升级

- 设置、主题、调度、感光及屏幕备份**自动继承**（`%APPDATA%\护眼助手\`，目录名沿用旧版）
- **开机自启需在新版中重新勾选一次**（注册表项名不同）
- 新旧版**不能同时运行**（共用屏幕 gamma 状态与备份文件）——切换前请先退出旧版

## 构建与测试（开发者）

```bash
# 依赖：Node.js + Rust 工具链（stable-msvc）
cd src-tauri

cargo test              # Rust 单测（152 项）
cargo build --release   # 产出 target/release/eye-guard.exe

cd ..
node --test tests/      # 前端契约测试

# 实机探针（--ignored，按需手动运行）
cargo test --release --test gamma_selftest -- --ignored   # 真实屏幕短暂变色后复原
cargo test --release --test camera_probe -- --ignored     # 摄像头采样探针
```

CI（`.github/workflows/build.yml`）在 push 到 `master`/`rust` 或打 `v*` tag 时自动执行「测试 + 构建」；打 tag 时额外产出 NSIS 安装器并创建 Release。

## 架构

```
src-tauri/src/
  ├─ 业务层（平台无关）   modes / usage / sun / scheduler / break_timer / settings / temperature
  ├─ 平台层（cfg 门控）   gamma / brightness / appscan / appicon / presence / appwatch
  │                        ├─ Windows → 原生实现（windows crate）
  │                        └─ 其它平台 → platform/stub/（降级实现，编译可用）
  └─ 装配层               lib / commands / state / *_rt（运行时线程）
src/renderer/            原生 HTML/CSS/JS（无框架、无构建步骤，经 shim.js 桥接 IPC）
```

**跨平台状态**：Windows 完整可用；macOS 已具备平台抽象层（Cargo 依赖平台化 + 模块 cfg 门控 + 降级 stub），原生实现待补（需 macOS 环境验证）。

设计文档：`docs/superpowers/specs/2026-10-01--rust-rewrite-design.md`
参数依据：`docs/eye-parameters-research.md`
同类调研：`docs/competitive-features-research.md`

## 已知限制

- **驱动 50% 规则**：部分显卡要求 gamma ramp 各通道最大值 ≥ 32768，否则整体拒绝写入。本机实测暖端下限约 3300K，低于此值会被安全钳制并提示。
- **外接显示器/台式机**通常无背光通道，亮度走黑纱（视觉等效但幅度有限）。
- **多显示器**：gamma 已逐屏应用；副屏原始校准未单独快照（同型号双屏无感知差异）。
- **全屏独占应用**：可能重置 gamma；ramp 守护会重新应用色温。
- **感光监测**依赖摄像头（无 ALS 硬件的机器），需授予摄像头权限。
- **与旧版（Electron）互斥**：不能同时运行。

## 许可

MIT

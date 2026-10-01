# 护眼助手（EyeGuard）

Windows 桌面护眼工具：**屏幕色温调节（防蓝光）+ 亮度调节 + 定时休息提醒 + 时间调度 + 感光监测**。
基于 **Rust + Tauri 2**（`windows` crate 直调 gdi32 gamma ramp——硬件级色温偏移，全屏生效）。

> v0.2.0 起为 **Rust/Tauri 重写版**（旧版 Electron 实现见 git 历史；设置自动继承）。

## 功能

- **7 档场景模式**：原色 6500K / 冷白提神 8000K / 办公 5500K / 阅读暖白 5000K / 傍晚 4500K / 夜晚 3400K / 深夜 2700K（色温 + 亮度组合，一键应用）
  - 预设值依据见 `docs/eye-parameters-research.md`（f.lux 实践值、办公照明 CCT 建议、昼夜节律研究）
  - 实际可用范围受显卡驱动限制（本机实测暖端边界约 3300K），超出时自动安全钳制并在界面提示
- **色温调节**：2000K–10000K 滑块（暖 ↔ 冷全线）；冷区（>6500K）红绿衰减、蓝保持
- **亮度调节**：50%–100%（透明遮罩实现，不与色温争抢空间）
- **时间调度**：到点自动切换模式，或弹卡片询问（[切换]/[忽略]/[今天不再问]）；启动/睡眠唤醒自动对齐当前时段
- **感光监测**（默认关闭）：优先用环境光传感器（有 ALS 硬件的机器），无硬件自动回退摄像头每 60s 短暂测光（本机计算亮度数值，不保存不传输），环境骤暗时提醒或自动切换
- **三主题**：深海（默认，DeepSeek 风格）/ 暖橙 / 胖鱼（鲸娘风：星夜星点背景 + 糖果胶囊按钮）
- **定时休息提醒**：工作/休息周期可配；温和卡片（不抢键盘焦点）/ 全屏遮罩 两种样式；支持推迟 / 跳过；托盘可暂停 1 小时
- **托盘常驻**：7 档模式切换 / 色温微调 / 恢复原色 / 暂停提醒 / 退出
- **全局热键**：`Ctrl+Alt+↑` / `Ctrl+Alt+↓` 色温 ±200K
- **安全恢复**：退出自动还原屏幕原色；崩溃后下次启动自动修复（dirty 标记自愈）

## 下载与安装

从 [Releases](https://github.com/tujinshui11/eye-guard/releases) 下载（每次打 tag 由 CI 自动构建）：

- **NSIS 安装器**（`*setup.exe`）：双击安装，开始菜单/卸载齐全；**内置 WebView2 bootstrapper**——目标机器缺组件时自动补装
- **便携版 zip**：解压即用（包内附使用说明与 WebView2 离线补装引导）
- 系统要求：Windows 10/11

## 从旧版（Electron 版）升级

- 设置、主题、调度、感光及屏幕备份**自动继承**（%APPDATA%\护眼助手\
- **开机自启需在新版中重新勾选一次**（注册表项名不同）
- 新旧版**不能同时运行**（共用屏幕 gamma 状态与备份文件）——切换前请先退出旧版

## 构建与测试（开发者）

```bash
cargo build --release --manifest-path src-tauri/Cargo.toml   # 构建（产物 src-tauri/target/release/eye-guard.exe）
cargo test --manifest-path src-tauri/Cargo.toml              # 单元测试（93 例，对照旧版行为契约）
cargo test --release --test gamma_selftest -- --ignored      # 实机 gamma 自检（真实屏幕短暂变色后复原）
cargo test --release --test camera_probe -- --ignored        # 摄像头采样探针（短暂占用）
node --test tests/theme.test.js                              # 前端主题契约测试
```

CI（`.github/workflows/build.yml`）：push 自动「测试 + 构建」；tag `v*` 自动构建 **NSIS 安装器 + 便携 zip** 并创建 Release（草稿）。

## 架构

| 模块（src-tauri/src/） | 职责 |
|---|---|
| `gamma.rs` | gdi32 `Get/SetDeviceGammaRamp`（windows crate 直绑） |
| `temperature.rs` | 色温算法（黑体近似、6500K 归一、冷暖双向反解）+ 驱动安全钳制 |
| `modes.rs` | 7 档场景模式表（纯数据） |
| `display.rs` | 显示控制器：原始值备份 / 恢复 / 脏标记崩溃自愈（gamma IO 可注入测试） |
| `overlay.rs` | 亮度遮罩（透明全屏、点击穿透） |
| `break_timer.rs` | 休息状态机（时间戳基准，跨睡眠正确） |
| `scheduler.rs` | 时间调度判定（窗口命中 / 防重 / 启动对齐 / 时钟回拨防护） |
| `ambient/` | 感光监测：analyzer（EMA 平滑 + P75 基线 + 回滞 + 冷却）/ als（WinRT）/ camera（nokhwa） |
| `settings.rs` | 配置持久化（深合并 / 损坏自愈 / 原子写 / v1→v2 迁移） |
| `tray.rs` / `hotkeys.rs` | 托盘菜单 / 全局热键 |
| `propose.rs` / `break_rt.rs` / `schedule_rt.rs` / `ambient/monitor.rs` | 建议卡片 / 休息系统 / 调度壳 / 感光壳 |
| `commands.rs` / `state.rs` / `lib.rs` | IPC 命令 15 条 / 全局状态 / 应用装配 |
| `break_rt.rs` | 休息提醒系统（1s tick + 窗口） |

渲染进程：`src/renderer/`（设置面板 / 休息提醒窗口 / 建议卡片窗口 / 亮度遮罩页，经 `shim.js` 桥接 Tauri IPC）。

设计文档：`docs/superpowers/specs/2026-10-01--rust-rewrite-design.md`

## 已知限制

- **驱动 50% 规则**：部分显卡要求 gamma ramp 各通道最大值 ≥ 32768，否则整体拒绝写入。本机实测暖端下限约 3300K（亮度 100%），低于此值会被安全钳制并提示。
- **多显示器**：色温作用于屏幕设备 DC；亮度遮罩覆盖主显示器（多屏为后续迭代）。
- **全屏独占应用**：可能重置 gamma；ramp 守护会重新应用色温。
- **与旧版（Electron）互斥**：不能同时运行。
- **感光触发延迟**：防闪烁设计需连续数个采样确认（默认约 3–5 分钟）后才触发提醒/切换。
- **单实例锁**：桌面残留旧实例会挡住新实例启动——先退出旧实例再启动新版。

## 开发环境备注（本机）

- Rust 工具链（rustup + MSVC）+ VS Build Tools + WebView2 Runtime；cargo 经 `~\.cargo\config.toml` 的代理拉取依赖。
- 探针脚本（手动运行）：`src-tauri/tests/` 下 `gamma_selftest` / `camera_probe` / `active_ramp_probe`（均 `--ignored`，按需触发）。

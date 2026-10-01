# 护眼助手（EyeGuard）

Windows 桌面护眼工具：**屏幕色温调节（防蓝光）+ 亮度调节 + 定时休息提醒**。
基于 Electron + koffi FFI 直调 Windows gamma ramp（硬件级色温偏移，全屏生效）。

## 功能

- **色温调节**：2000K–6500K 滑块 + 预设（办公 5500K / 傍晚 4500K / 夜晚 3400K / 深夜 2700K / 关闭）
  - 预设值依据见 `docs/eye-parameters-research.md`（f.lux 实践值、办公照明 CCT 建议、昼夜节律研究）
  - 实际可用范围受显卡驱动限制（本机实测边界约 3300K），超出时自动安全钳制并在界面提示
- **亮度调节**：50%–100%（透明遮罩实现，不与色温争抢空间）；建议与室内环境光匹配（参照旁置白纸）
- **定时休息提醒**：工作/休息周期可配（默认 20 分钟 / 20 秒，对齐 AAO/AOA「20-20-20」；可自定义节奏）
  - 温和卡片（默认，不抢键盘焦点）/ 全屏遮罩 两种样式
  - 支持 推迟 5 分钟 / 跳过；托盘可暂停 1 小时
- **托盘常驻**：预设切换 / 色温微调 / 恢复原色 / 暂停提醒 / 退出
- **全局热键**：`Ctrl+Alt+↑` / `Ctrl+Alt+↓` 色温 ±200K
- **安全恢复**：退出自动还原屏幕原色；崩溃后下次启动自动修复（dirty 标记自愈）

## 使用

```bash
npm install        # 安装依赖（如 Electron 二进制缺失见下文「开发环境备注」）
npm start          # 开发模式运行
npm run package    # 打包便携版 → dist/护眼助手-win32-x64/护眼助手.exe
npm test           # 单元测试（node:test，36 用例）
npm run selftest   # FFI 集成自检（真实屏幕会短暂变色后复原）
```

## 架构

| 模块（src/main/） | 职责 |
|---|---|
| `gamma.js` | koffi FFI 封装（gdi32 `Get/SetDeviceGammaRamp`） |
| `temperature.js` | 色温算法（黑体近似、6500K 归一）+ 驱动安全钳制 |
| `display.js` | 显示控制器：原始值备份 / 恢复 / 脏标记崩溃自愈 |
| `overlay.js` | 亮度遮罩（透明全屏、点击穿透） |
| `breakTimer.js` | 休息状态机（时间戳基准，跨睡眠正确） |
| `settings.js` | 配置持久化（深合并 / 损坏自愈 / 原子写） |
| `hotkeys.js` / `tray.js` | 全局热键 / 托盘菜单 |
| `index.js` | 生命周期、单实例、IPC 路由、开机自启 |

渲染进程：`src/renderer/`（设置面板 / 休息提醒窗口 / 亮度遮罩页）。

设计文档：`docs/superpowers/specs/2026-10-01--design.md`

## 已知限制

- **驱动 50% 规则**：部分显卡（如 Intel 集显）要求 gamma ramp 各通道最大值 ≥ 32768，否则整体拒绝写入。本机实测色温下限约 3300K（亮度 100%），低于此值会被安全钳制并提示。
- **多显示器**：色温作用于屏幕设备 DC；亮度遮罩覆盖主显示器（多屏逐显示器支持为后续迭代）。
- **全屏独占应用**：可能重置 gamma 或盖住亮度遮罩；ramp 守护每 15 秒会重新应用色温。
- **开发模式**：`getLoginItemSettings` 回读不可靠（注册表写入正常，打包后回读恢复准确）。

## 开发环境备注（本机）

- **Electron 二进制安装**：本环境 npm 的 install-scripts 安全门会拦截 postinstall。如遇 `node_modules/electron/dist` 缺失：
  1. `npm install-scripts approve electron` 后重装；或
  2. 手动解压 electron zip 到 `node_modules/electron/dist/` 并以**无换行**方式写 `path.txt`（内容 `electron.exe`）。
- **koffi**：预编译包（`@koromix/koffi-win32-x64`）自动就位，无需编译工具链。
- **打包**：`@electron/packager` 默认 asar 且自动解包 `**/*.node`（原生模块无需手动处理）。

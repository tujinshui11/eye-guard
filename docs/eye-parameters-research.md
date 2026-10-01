# 屏幕护眼参数调研（考究版）

> 2026-10-01 ｜ 目的：为护眼助手的预设参数建立可追溯的科学/行业依据，替代初版估值（4500/3400/2700 为经验估计）。
> 本文所有数值均附来源；存在争议的结论如实标注。

## 一、结论速览：场景化参数表

| 场景 | 建议色温 | 亮度 | 主要依据 |
|---|---|---|---|
| 白天办公 | **5500K**（轻度柔和） | 匹配环境光（显示白 ≈ 旁置白纸） | 办公照明 CCT 建议 4000–5500K；白天保留大部分节律信号（f.lux 白天=不动） |
| 傍晚（日落前后） | **4500K** | 随环境光缓降 | f.lux 的 6500→3400 过渡区间 |
| 夜晚（日落后） | **3400K** | 约 70% | f.lux 默认日落值（"卤素灯"色，f.lux 称移除约 3/4 蓝光、约 1/2 绿光） |
| 深夜（睡前 2–3 小时） | **2700K**（越低越好，受硬件约束） | 更低（约 60%） | CircadianShield：日落后 2700–3000K；晚间的昼夜节律目标是低 melanopic 剂量 |
| 白天（不干预） | **6500K**（原色） | — | 白天蓝光有益（维持警觉与昼夜节律同步），f.lux 白天不改变色温 |

**休息节奏**：**20-20-20**（AAO/AOA 推荐：每 20 分钟，看 20 英尺外 20 秒）。注意：有研究显示"精确 20 分钟表"未必优于其他休息频率，但"定期远眺休息"本身是共识——故应支持自定义节奏。

**屏幕距离**：一臂长（50–70cm / 20–28 英寸），屏幕上缘与视线齐平或略低。

## 二、关键来源与数值

### 2.1 f.lux（行业实践标杆，同一技术路线的成熟产品）

| 时段 | 色温 | 备注 |
|---|---|---|
| Daytime | 6500K | 默认不改变屏幕色彩 |
| Sunset | 3400K | "Halogen"（卤素灯色），官方称移除约 3/4 蓝光、约 1/2 绿光 |
| Bedtime | 1900K | "Candle"（烛光色） |

- f.lux FAQ 明确立场：**"部分研究表明白天蓝光有益，但深夜蓝光可能负面影响睡眠模式"**——白天不干预是刻意的节律保护，而非功能缺失。
- 使用建议：如默认太暖，可先从 "fluorescent"（荧光灯色）开始，适应后逐步调暖。
- 来源：justgetflux.com FAQ / welcome 页 / macOS 快速入门（computerbas.nl 参数整理）。

### 2.2 AAO / AOA（美国眼科学会 / 美国验光协会）

- **20-20-20 规则**：每 20 分钟，看 20 英尺（约 6 米）外 20 秒——两家学会均为第一推荐的数字用眼习惯。
- 屏幕距离约 20–26 英寸（约 50–66cm）。
- 强调 **主动眨眼**（盯屏时眨眼频率降至正常的 1/3–1/2，干眼是视疲劳主因之一）。
- 争议标注：有独立分析（Ocarina Health, 2026）指出"精确 20-20-20 时刻表"在对照实验中未显著优于宽松休息——建议把 20-20-20 作为**默认建议**而非铁律，保留用户调节空间。

### 2.3 昼夜节律光学（CIE S 026:2018 / melanopic EDI）

现代照明科学的权威指标是 **melanopic EDI**（黑视素等效日光照明度，单位 lux），已被 2022 国际共识与 WELL Building Standard v2 采用。

- 推荐目标（Brown 2022 等综述）：**白天 ≥250 lux melanopic ｜ 晚间 ≤10 lux ｜ 深夜 ≤1 lux**。
- 对本工具的含义：夜间色温与亮度的大幅压低，不只是"舒适"，而是**控制视网膜 melanopic 剂量以保护褪黑素分泌**——这是深夜档（2700K + 低亮度）的科学依据。
- 来源：CIE S 026/E:2018 定义；healthcanon.com 对 Brown 2022 目标的引用；Nature 2023（s42003-023-04598-4）关于晚间显示屏光对睡眠影响的对照研究。

### 2.4 亮度与对比度（工效学与实践）

- **亮度匹配原则**：屏幕白色应与旁边一张白纸在相同光照下的观感一致（CircadianShield，验光方向）；量化参考：普通办公室 ≈ **120 nits**，昏暗房间 **40–60 nits**。
- **对比度**：文本工作 60–70%，照片/视频 70–75%。
- ISO 9241-303（显示工效学）给出显示亮度/对比度与环境光的通用要求（第三方解读例：屏幕 200 cd/m² 配环境照度 300 lux 的场景下字符视角 20 arcmin 符合标准）。
- 环境光：显示器背后墙面应有照明（避免屏幕成为唯一光源形成极端对比）。

### 2.5 低蓝光认证体系（硬件侧参考）

- **TÜV Rheinland × Eyesafe Display Requirements 2.0**（2022）：以蓝光毒性因子 **BLTF ≤ 0.085** 为硬指标，配套 RPF（Radiance Protection Factor）0–100 消费者量表；背后有 250+ 研究与眼科/验光学顾问团。
- 硬件认证 vs 软件方案：硬件低蓝光（光谱层面）不牺牲色彩；**软件低蓝光（色温偏移，本工具的路径）必然引入暖色偏**——这是已知代价，UI 上以"如实显示实际值"应对。
- 来源：eyesafe.com 官方公告、tuv.com 低蓝光认证页。

### 2.6 办公照明色温（环境侧参考）

- 一般办公任务的环境光 CCT 建议 **4000–5500K**；高精度任务（设计/制图）趋向 5000K；会议室 5000K。
- 屏幕色温低于环境光的合理差值会让画面显暖；接近则视觉反差最小——**白天办公档 5500K 的选值依据**。
- 来源：philmarkoffice.com（办公 CCT 指南）、techflare.net、Elsevier 2025 办公光环境研究（S0272494425001112）。

## 三、修订方案（对照初版预设）

| 档位 | 初版（经验估值） | 修订值 | 修订依据 |
|---|---|---|---|
| 办公 | 4500K | **5500K** | 办公照度 CCT 建议上限（4000–5500K）；白天保留节律信号 |
| （新增）傍晚 | — | **4500K** | f.lux 昼→夜过渡的中间态，承接原 4500 的舒适定位 |
| 夜晚 | 3400K | 3400K（不变） | 与 f.lux 默认日落值精确一致（现获得依据） |
| 深夜 | 2700K | **2700K**（不变） | 与 CircadianShield 日落后建议（2700–3000K）一致；本机受驱动限制钳制至约 3300K（如实提示） |
| 关闭 | 6500K | 6500K（不变） | 白天/色彩工作场景的原色基线 |

**休息提醒默认**：40 分钟 / 5 分钟 → **20 分钟 / 20 秒**（对齐 AAO/AOA 唯一官方节奏；用户可自调）。

**亮度**：保持手动 50–100%（无环境光传感器），UI 提示改为"与环境光匹配（参照旁置白纸）"。

## 四、明确不采用的方案（及原因）

1. **白天全时段压蓝光到 4000K 以下**——与节律科学冲突（白天需要蓝光），f.lux 同路线产品也不这么做。
2. **自动昼夜跟随（日升日落调度）**——科学上支持（f.lux 模式），但属首版范围外；列入后续迭代。
3. **追求 1900K 睡前档**——硬件（本机 gamma ramp 50% 规则）不可达，且软件方案在极低色温下对比度损失显著；不做虚假承诺。

## 五、来源清单

- f.lux 官方：https://justgetflux.com/faq.html ；https://justgetflux.com/news/pages/welcome/ ；参数整理 https://computerbas.nl/docs/?doc=flux
- AAO：https://www.aao.org/eye-health/tips-prevention/computer-usage ；https://www.aao.org/eye-health/tips-prevention/computer-vision-syndrome
- AOA：https://www.aoa.org/healthy-eyes/eye-and-vision-conditions/computer-vision-syndrome/
- Eyesafe/TÜV：https://eyesafe.com/tuv-rheinland-eyesafe-announce-eyesafe-display-requirements-20/ ；https://www.tuv.com/world/en/low-blue-light-certification-for-electrical-products.html
- CircadianShield（亮度/对比度数值）：https://circadianshield.com/blog/best-monitor-settings-eye-health
- melanopic EDI：CIE S 026/E:2018（定义）；https://www.healthcanon.com/light-and-recovery/circadian-daylight-melanopic-edi （Brown 2022 目标引用）；https://www.nature.com/articles/s42003-023-04598-4
- 办公照明 CCT：https://philmarkoffice.com/ideal-color-temperature-office-work/ ；https://www.sciencedirect.com/science/article/pii/S0272494425001112
- 20-20-20 争议：https://ocarinahealth.org/2026/06/07/screen-era-eye-strain-20-20-20-rule-evidence/
- ISO 9241-303：https://www.iso.org/obp/ui/#!iso:std:57992:en

> 注：本文为工程决策依据整理，非医学建议；个体差异（干眼、屈光、偏头痛等）可能改变最优参数。

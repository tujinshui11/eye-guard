//! 亮度控制系统（C 方案）——背光主控 + 黑纱补充
//!
//! 双层结构：
//!   1. **系统背光**（WMI `WmiSetBrightness`）——真实降低屏幕发光，省电护眼；
//!      通过 `WmiMonitorBrightness` 读回当前值用于同步外部变更（Fn 键）。
//!   2. **黑纱补充**（overlay 遮罩）——当目标亮度低于背光下限时，
//!      用遮罩 alpha 继续压暗（乘法叠加，可突破硬件下限）。
//!
//! 设计要点：
//!   - 背光不可用（台式机/外接屏）→ 自动降级为纯黑纱模式（同 v0.2.2 行为）；
//!   - 所有 WMI 调用失败均静默降级（fail-open），不阻塞亮度调节主流程；
//!   - `BrightnessController` 持有可注入的 `BacklightIo`，测试用 mock 驱动。

/// 背光 IO 抽象——生产用 WMI，测试用 mock
pub trait BacklightIo: Send {
    /// 读取当前背光百分比（0–100）；不可用返回 None
    fn get(&self) -> Option<u32>;
    /// 写入背光百分比（0–100）；成功返回 true
    fn set(&self, percent: u32) -> bool;
}

/// 结果类型：本次调节的最终归属
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrightnessOutcome {
    /// 全部由背光承担（目标 ≥ 背光下限）
    BacklightOnly { backlight: u32 },
    /// 背光承担下限部分，剩余由黑纱补充
    BacklightPlusMask { backlight: u32, mask_alpha: f64 },
    /// 背光不可用，全部由黑纱承担（旧行为）
    MaskOnly { mask_alpha: f64 },
}

/// 亮度控制器：管理背光 + 黑纱的分配逻辑
pub struct BrightnessController<I: BacklightIo> {
    io: I,
    /// 背光可用性（首次探测后缓存；None = 未探测）
    backlight_available: Option<bool>,
    /// 最后一次成功写入的背光值（外部变更同步用：读回与它不符 = 用户 Fn 改了）
    pub last_written: Option<u32>,
    /// 启动时捕获的系统背光原值（退出/恢复原色时还原用）
    initial_backlight: Option<u32>,
    /// 背光下限（百分比）——低于此值背光不再降，由黑纱补充
    pub backlight_floor: u32,
}

impl<I: BacklightIo> BrightnessController<I> {
    pub fn new(io: I) -> Self {
        Self {
            io,
            backlight_available: None,
            last_written: None,
            initial_backlight: None,
            backlight_floor: 20,
        }
    }

    /// 捕获启动时的背光原值（应用启动时调用一次；已捕获则幂等）
    ///
    /// 注意：必须在任何写入之前调用，否则捕获到的是被我们改过的值。
    pub fn capture_initial(&mut self) {
        if self.initial_backlight.is_some() {
            return;
        }
        self.initial_backlight = self.io.get();
    }

    /// 还原背光到启动原值（退出应用 / 「恢复原色」时调用）
    ///
    /// - 未捕获原值（无硬件/从未启动过）→ 无操作
    /// - 当前值已等于原值 → 不重复写（幂等）
    /// 返回是否执行了写入。
    pub fn restore_initial(&mut self) -> bool {
        let Some(initial) = self.initial_backlight else {
            return false;
        };
        if self.io.get() == Some(initial) {
            return false;
        }
        let ok = self.io.set(initial);
        if ok {
            self.last_written = Some(initial);
        }
        ok
    }

    /// 探测背光可用性（读一次即知；结果缓存）
    pub fn is_available(&mut self) -> bool {
        if let Some(v) = self.backlight_available {
            return v;
        }
        let avail = self.io.get().is_some();
        self.backlight_available = Some(avail);
        avail
    }

    /// 读缓存的可用性（不触发探测；未探测时视为不可用）——供 plan 的调用方使用
    pub fn cached_available(&self) -> bool {
        self.backlight_available.unwrap_or(false)
    }

    /// 读取当前系统背光（用于外部变更同步；不可用返回 None）
    pub fn current_backlight(&self) -> Option<u32> {
        self.io.get()
    }

    /// 计算亮度分配：目标百分比 → (背光值, 黑纱 alpha)
    ///
    /// 语义：
    ///   - 无背光：alpha = (100-b)/100（与 v0.2.2 一致；b ∈ [50,100]）
    ///   - 有背光且 b ≥ floor：背光 = b，黑纱 = 0（背光独立承担）
    ///   - 有背光且 b < floor：背光 = floor，黑纱 α = (floor-b)/floor
    ///     （把 [0, floor] 区间的剩余光量用遮罩继续乘下去；
    ///      例如 floor=20、b=10 → α=0.5 → 出光 20%×0.5=10%）
    pub fn plan(&self, target: u32, available: bool) -> (u32, f64) {
        let b = target.min(100);
        if !available {
            let alpha = (100.0 - b as f64) / 100.0;
            return (0, alpha);
        }
        if b >= self.backlight_floor {
            (b, 0.0)
        } else {
            let floor = self.backlight_floor.max(1);
            let alpha = (floor as f64 - b as f64) / floor as f64;
            (floor, alpha.clamp(0.0, 1.0))
        }
    }

    /// 应用亮度：分配 → 写背光（如可用）→ 返回黑纱 alpha
    ///
    /// 返回的 alpha 供调用方驱动 overlay；负值/异常均落在 [0,1]。
    pub fn apply(&mut self, target: u32) -> (BrightnessOutcome, f64) {
        let available = self.is_available();
        let (backlight, alpha) = self.plan(target, available);
        if available {
            let ok = self.io.set(backlight);
            if !ok {
                // 写失败：降级为纯黑纱（避免亮度失控）
                self.backlight_available = Some(false);
                let fallback_alpha = (100.0 - target.min(100) as f64) / 100.0;
                return (
                    BrightnessOutcome::MaskOnly {
                        mask_alpha: fallback_alpha,
                    },
                    fallback_alpha,
                );
            }
            self.last_written = Some(backlight);
        }
        let outcome = if !available {
            BrightnessOutcome::MaskOnly { mask_alpha: alpha }
        } else if alpha > 0.001 {
            BrightnessOutcome::BacklightPlusMask {
                backlight,
                mask_alpha: alpha,
            }
        } else {
            BrightnessOutcome::BacklightOnly { backlight }
        };
        (outcome, alpha)
    }

    /// 检测外部背光变更（用户按 Fn 键/系统自动调光）：读回值与 last_written 不符
    ///
    /// 返回 Some(当前系统值) 表示发生了外部变更；未探测/不可用/无写入记录时返回 None。
    pub fn detect_external_change(&self) -> Option<u32> {
        if self.backlight_available != Some(true) {
            return None;
        }
        let expected = self.last_written?;
        let actual = self.io.get()?;
        if actual != expected {
            Some(actual)
        } else {
            None
        }
    }
}

// ============================================================
// WMI 实现（生产）
// ============================================================

/// 生产 IO：WMI `WmiMonitorBrightness` / `WmiMonitorBrightnessMethods`
pub struct WmiBacklight;

impl BacklightIo for WmiBacklight {
    fn get(&self) -> Option<u32> {
        wmi::read_brightness()
    }

    fn set(&self, percent: u32) -> bool {
        wmi::set_brightness(percent.clamp(0, 100) as u8)
    }
}

pub use wmi::{debug_read_brightness, set_brightness_diag};

mod wmi {
    //! WMI 亮度读写（windows crate COM 绑定）
    //!
    //! 命名空间 `root\wmi`：
    //!   - `WmiMonitorBrightness.CurrentBrightness` (byte) — 读
    //!   - `WmiMonitorBrightnessMethods.WmiSetBrightness(Timeout, Brightness)` — 写

    use windows::core::{BSTR, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED,
    };
    use windows::Win32::System::Variant::{VARIANT, VT_BSTR, VT_UI1, VT_UI4};
    use windows::Win32::System::Wmi::{
        CIM_UINT32, CIM_UINT8, IWbemClassObject, IWbemContext, IWbemLocator, IWbemServices,
        WbemLocator, WBEM_FLAG_FORWARD_ONLY, WBEM_GENERIC_FLAG_TYPE, WBEM_INFINITE,
    };

    const NS: &str = "ROOT\\WMI";

    /// COM 初始化守卫（RAII；每线程配对 CoInitializeEx/CoUninitialize）
    struct ComGuard {
        needs_uninit: bool,
    }

    impl ComGuard {
        fn new() -> Self {
            // S_OK / S_FALSE（已初始化，引用计数+1）均需配对 CoUninitialize；
            // RPC_E_CHANGED_MODE 等失败情形不再调用（避免引用计数失衡）
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if hr.is_ok() {
                // 进程级安全初始化（WMI 客户端标准模式；必须先于 CoCreateInstance）
                // 重复调用会返回 RPC_E_TOO_LATE——忽略即可
                use windows::Win32::System::Com::{
                    CoInitializeSecurity, EOAC_NONE, RPC_C_AUTHN_LEVEL_DEFAULT,
                    RPC_C_IMP_LEVEL_IMPERSONATE,
                };
                unsafe {
                    let _ = CoInitializeSecurity(
                        None,
                        -1,
                        None,
                        None,
                        RPC_C_AUTHN_LEVEL_DEFAULT,
                        RPC_C_IMP_LEVEL_IMPERSONATE,
                        None,
                        EOAC_NONE,
                        None,
                    );
                }
            }
            Self {
                needs_uninit: hr.is_ok(),
            }
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.needs_uninit {
                unsafe { CoUninitialize() };
            }
        }
    }

    /// 连接 `root\wmi` 命名空间
    ///
    /// 关键：ConnectServer 后必须 CoSetProxyBlanket 设置代理安全级别，
    /// 否则后续 ExecQuery 会报 0x80041003（WBEM_E_ACCESS_DENIED）——
    /// PowerShell 自动处理这一步，原生 COM 必须显式设置。
    fn connect() -> Option<IWbemServices> {
        use windows::Win32::System::Com::{
            CoSetProxyBlanket, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE,
        };
        use windows::Win32::System::Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE};
        unsafe {
            let locator: IWbemLocator =
                CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).ok()?;
            let services = locator
                .ConnectServer(
                    &BSTR::from(NS),
                    &BSTR::default(),
                    &BSTR::default(),
                    &BSTR::default(),
                    0,
                    &BSTR::default(),
                    None::<&IWbemContext>,
                )
                .ok()?;

            // 设置代理安全：调用级认证 + 模拟级身份（WMI 标准设置）
            let _ = CoSetProxyBlanket(
                &services,
                RPC_C_AUTHN_WINNT,   // 0x0A：NTLM
                RPC_C_AUTHZ_NONE,    // 0：不做授权
                None,                // 无服务器主体名
                RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
            );
            Some(services)
        }
    }

    /// 查询单实例、取指定属性为 u32
    fn query_u32(services: &IWbemServices, class: &str, prop: &str) -> Option<u32> {
        unsafe {
            let query = BSTR::from(format!("SELECT {prop} FROM {class}"));
            let lang = BSTR::from("WQL");
            let enumerator = services
                .ExecQuery(
                    &lang,
                    &query,
                    WBEM_FLAG_FORWARD_ONLY,
                    None::<&IWbemContext>,
                )
                .ok()?;

            let mut objects: [Option<IWbemClassObject>; 1] = [None];
            let mut returned: u32 = 0;
            let hr = enumerator.Next(WBEM_INFINITE, &mut objects, &mut returned);
            if hr.is_err() || returned == 0 {
                return None;
            }
            let obj = objects[0].take()?;

            let name: Vec<u16> = prop.encode_utf16().chain(std::iter::once(0)).collect();
            let mut val = VARIANT::default();
            obj.Get(PCWSTR(name.as_ptr()), 0, &mut val, None, None)
                .ok()?;
            // 读 u8 或 u32 两种可能的返回类型
            let v = &val.Anonymous.Anonymous;
            match v.vt {
                t if t == VT_UI1 => Some(v.Anonymous.bVal as u32),
                t if t == VT_UI4 => Some(v.Anonymous.ulVal),
                _ => None,
            }
        }
    }

    /// 构造数值 VARIANT（写入用）
    fn variant_u8(v: u8) -> VARIANT {
        use windows::Win32::System::Variant::{VARIANT_0, VARIANT_0_0, VARIANT_0_0_0};
        let mut val = VARIANT::default();
        val.Anonymous = VARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_UI1,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { bVal: v },
            }),
        };
        val
    }

    fn variant_u32(v: u32) -> VARIANT {
        use windows::Win32::System::Variant::{VARIANT_0, VARIANT_0_0, VARIANT_0_0_0};
        let mut val = VARIANT::default();
        val.Anonymous = VARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_UI4,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { ulVal: v },
            }),
        };
        val
    }

    /// WMI 对 CIM_UINT32 参数要求 VT_I4 装载（WMI 编码怪癖：I4 可承载 u32）
    fn variant_i4(v: i32) -> VARIANT {
        use windows::Win32::System::Variant::{VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_I4};
        let mut val = VARIANT::default();
        val.Anonymous = VARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_I4,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { lVal: v },
            }),
        };
        val
    }

    /// 读取当前背光（0–100）
    pub fn read_brightness() -> Option<u32> {
        let _guard = ComGuard::new();
        let services = connect()?;
        query_u32(&services, "WmiMonitorBrightness", "CurrentBrightness")
    }

    /// 诊断：逐步骤执行读取流程并输出失败点（供探针/排障使用）
    ///
    /// 返回诊断字符串；不改变任何系统状态。
    pub fn debug_read_brightness() -> String {
        use std::fmt::Write as _;
        use windows::Win32::System::Com::{
            CoInitializeSecurity, CoSetProxyBlanket, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_AUTHN_LEVEL_DEFAULT, RPC_C_IMP_LEVEL_IMPERSONATE,
        };
        use windows::Win32::System::Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE};
        let mut out = String::new();

        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        let _ = writeln!(out, "CoInitializeEx: {:?}", hr);

        let sec = unsafe {
            CoInitializeSecurity(
                None,
                -1,
                None,
                None,
                RPC_C_AUTHN_LEVEL_DEFAULT,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
                None,
            )
        };
        let _ = writeln!(out, "CoInitializeSecurity: {:?}", sec);

        let locator: windows::core::Result<IWbemLocator> =
            unsafe { CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER) };
        match &locator {
            Ok(_) => { let _ = writeln!(out, "CoCreateInstance(WbemLocator): OK"); }
            Err(e) => {
                let _ = writeln!(out, "CoCreateInstance(WbemLocator): ERR {e}");
                return out;
            }
        }

        let services = unsafe {
            locator
                .unwrap()
                .ConnectServer(
                    &BSTR::from(NS),
                    &BSTR::default(),
                    &BSTR::default(),
                    &BSTR::default(),
                    0,
                    &BSTR::default(),
                    None::<&IWbemContext>,
                )
        };
        match &services {
            Ok(_) => { let _ = writeln!(out, "ConnectServer({NS}): OK"); }
            Err(e) => {
                let _ = writeln!(out, "ConnectServer: ERR {e}");
                return out;
            }
        }
        let services = services.unwrap();

        let blanket = unsafe {
            CoSetProxyBlanket(
                &services,
                RPC_C_AUTHN_WINNT,
                RPC_C_AUTHZ_NONE,
                None,
                RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
            )
        };
        let _ = writeln!(out, "CoSetProxyBlanket: {:?}", blanket);

        unsafe {
            let query = BSTR::from("SELECT CurrentBrightness FROM WmiMonitorBrightness");
            let lang = BSTR::from("WQL");
            let enumerator =
                services.ExecQuery(&lang, &query, WBEM_FLAG_FORWARD_ONLY, None::<&IWbemContext>);
            match &enumerator {
                Ok(_) => { let _ = writeln!(out, "ExecQuery: OK"); }
                Err(e) => {
                    let _ = writeln!(out, "ExecQuery: ERR {e}");
                    return out;
                }
            }
            let enumerator = enumerator.unwrap();

            let mut objects: [Option<IWbemClassObject>; 1] = [None];
            let mut returned: u32 = 0;
            let hr = enumerator.Next(WBEM_INFINITE, &mut objects, &mut returned);
            let _ = writeln!(out, "Next: hr={hr:?} returned={returned}");
            if returned == 0 {
                return out;
            }
            let Some(obj) = objects[0].take() else {
                let _ = writeln!(out, "objects[0] is None");
                return out;
            };

            let name: Vec<u16> = "CurrentBrightness"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut val = VARIANT::default();
            let g = obj.Get(PCWSTR(name.as_ptr()), 0, &mut val, None, None);
            let _ = writeln!(out, "Get(CurrentBrightness): {:?}", g);
            if g.is_ok() {
                let inner = &val.Anonymous.Anonymous;
                let _ = writeln!(out, "vt = {} (VT_UI1={} VT_UI4={})", inner.vt.0, VT_UI1.0, VT_UI4.0);
                let raw = match inner.vt {
                    t if t == VT_UI1 => Some(inner.Anonymous.bVal as u32),
                    t if t == VT_UI4 => Some(inner.Anonymous.ulVal),
                    _ => None,
                };
                let _ = writeln!(out, "decoded = {raw:?}");
            }
        }
        out
    }

    /// 写入背光（0–100）；成功返回 true
    pub fn set_brightness(percent: u8) -> bool {
        set_brightness_diag(percent).0
    }

    /// 写入背光（诊断版）：返回 (是否成功, 诊断字符串)
    ///
    /// 逐步输出各环节结果，供探针定位失败点。
    pub fn set_brightness_diag(percent: u8) -> (bool, String) {
        use std::fmt::Write as _;
        let mut out = String::new();

        let _guard = ComGuard::new();
        let Some(services) = connect() else {
            let _ = writeln!(out, "connect: FAILED");
            return (false, out);
        };
        let _ = writeln!(out, "connect: OK");
        unsafe {
            // 1. 取方法实例，并读取其实例路径（__PATH）
            let query = BSTR::from("SELECT * FROM WmiMonitorBrightnessMethods");
            let lang = BSTR::from("WQL");
            let enumerator = match services.ExecQuery(
                &lang,
                &query,
                WBEM_FLAG_FORWARD_ONLY,
                None::<&IWbemContext>,
            ) {
                Ok(e) => {
                    let _ = writeln!(out, "ExecQuery(methods): OK");
                    e
                }
                Err(e) => {
                    let _ = writeln!(out, "ExecQuery(methods): ERR {e}");
                    return (false, out);
                }
            };
            let mut objects: [Option<IWbemClassObject>; 1] = [None];
            let mut returned: u32 = 0;
            let hr = enumerator.Next(WBEM_INFINITE, &mut objects, &mut returned);
            let _ = writeln!(out, "Next: hr={hr:?} returned={returned}");
            if hr.is_err() || returned == 0 {
                return (false, out);
            }
            let Some(inst) = objects[0].take() else {
                let _ = writeln!(out, "objects[0] None");
                return (false, out);
            };

            let path_name: Vec<u16> = "__PATH".encode_utf16().chain(std::iter::once(0)).collect();
            let mut path_val = VARIANT::default();
            let g = inst.Get(PCWSTR(path_name.as_ptr()), 0, &mut path_val, None, None);
            let _ = writeln!(out, "Get(__PATH): {g:?}");
            if g.is_err() {
                return (false, out);
            }
            let path_inner = &path_val.Anonymous.Anonymous;
            let _ = writeln!(out, "__PATH vt={} (BSTR={})", path_inner.vt.0, VT_BSTR.0);
            if path_inner.vt != VT_BSTR {
                return (false, out);
            }
            let object_path: BSTR = (*path_inner.Anonymous.bstrVal).clone();
            let _ = writeln!(out, "object_path={:?}", object_path.to_string());

            // 2. 取类定义 → 方法输入签名 → 实例化入参
            let mut class_obj: Option<IWbemClassObject> = None;
            let go = services.GetObject(
                &BSTR::from("WmiMonitorBrightnessMethods"),
                WBEM_GENERIC_FLAG_TYPE(0),
                None::<&IWbemContext>,
                Some(&mut class_obj),
                None,
            );
            let _ = writeln!(out, "GetObject(class): {go:?} present={}", class_obj.is_some());
            if go.is_err() {
                return (false, out);
            }
            let Some(class_obj) = class_obj else {
                return (false, out);
            };

            let method_name: Vec<u16> = "WmiSetBrightness"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut in_sig: Option<IWbemClassObject> = None;
            let mut out_sig: Option<IWbemClassObject> = None;
            let gm = class_obj.GetMethod(PCWSTR(method_name.as_ptr()), 0, &mut in_sig, &mut out_sig);
            let _ = writeln!(
                out,
                "GetMethod: {gm:?} in_sig={} out_sig={}",
                in_sig.is_some(),
                out_sig.is_some()
            );
            if gm.is_err() {
                return (false, out);
            }
            let Some(in_sig) = in_sig else {
                return (false, out);
            };
            let spawn = in_sig.SpawnInstance(0);
            let _ = writeln!(out, "SpawnInstance: ok={}", spawn.is_ok());
            let Ok(in_params) = spawn else {
                return (false, out);
            };

            // 3. 填入参：Timeout=0（立即生效），Brightness=目标值
            let timeout_name: Vec<u16> = "Timeout".encode_utf16().chain(std::iter::once(0)).collect();
            let bright_name: Vec<u16> = "Brightness"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let p1 = in_params.Put(PCWSTR(timeout_name.as_ptr()), 0, &variant_i4(0), CIM_UINT32.0);
            let _ = writeln!(out, "Put(Timeout=0): {p1:?}");
            let p2 = in_params.Put(PCWSTR(bright_name.as_ptr()), 0, &variant_u8(percent), CIM_UINT8.0);
            let _ = writeln!(out, "Put(Brightness={percent}): {p2:?}");
            if p1.is_err() || p2.is_err() {
                return (false, out);
            }

            // 4. 执行方法
            let mut out_params: Option<IWbemClassObject> = None;
            let em = services.ExecMethod(
                &object_path,
                &BSTR::from("WmiSetBrightness"),
                WBEM_GENERIC_FLAG_TYPE(0),
                None::<&IWbemContext>,
                &in_params,
                Some(&mut out_params),
                None,
            );
            let _ = writeln!(out, "ExecMethod: {em:?}");
            (em.is_ok(), out)
        }
    }
}

// ============================================================
// 测试（mock IO）
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    struct MockIo {
        value: std::sync::Mutex<Option<u32>>,
        fail_set: bool,
    }

    impl MockIo {
        fn available(v: u32) -> Self {
            Self {
                value: std::sync::Mutex::new(Some(v)),
                fail_set: false,
            }
        }
        fn unavailable() -> Self {
            Self {
                value: std::sync::Mutex::new(None),
                fail_set: false,
            }
        }
        fn failing_set() -> Self {
            Self {
                value: std::sync::Mutex::new(Some(100)),
                fail_set: true,
            }
        }
    }

    impl BacklightIo for MockIo {
        fn get(&self) -> Option<u32> {
            *self.value.lock().unwrap()
        }
        fn set(&self, percent: u32) -> bool {
            if self.fail_set {
                return false;
            }
            *self.value.lock().unwrap() = Some(percent);
            true
        }
    }

    #[test]
    fn plan_backlight_only_when_target_above_floor() {
        let c = BrightnessController::new(MockIo::available(100));
        let (bl, alpha) = c.plan(80, true);
        assert_eq!(bl, 80);
        assert!(alpha <= 0.001, "背光独立承担时无遮罩");
    }

    #[test]
    fn plan_floor_exact_boundary() {
        let c = BrightnessController::new(MockIo::available(100));
        let (bl, alpha) = c.plan(20, true);
        assert_eq!(bl, 20);
        assert!(alpha <= 0.001);
    }

    #[test]
    fn plan_below_floor_splits_to_mask() {
        let c = BrightnessController::new(MockIo::available(100));
        let (bl, alpha) = c.plan(10, true);
        assert_eq!(bl, 20, "背光停在下限");
        assert!((alpha - 0.5).abs() < 0.001, "10/20 → 半透遮罩");
    }

    #[test]
    fn plan_no_backlight_uses_mask_only() {
        let c = BrightnessController::new(MockIo::unavailable());
        let (bl, alpha) = c.plan(50, false);
        assert_eq!(bl, 0);
        assert!((alpha - 0.5).abs() < 0.001);
    }

    #[test]
    fn apply_writes_backlight_when_available() {
        let mut c = BrightnessController::new(MockIo::available(100));
        let (outcome, alpha) = c.apply(70);
        assert_eq!(outcome, BrightnessOutcome::BacklightOnly { backlight: 70 });
        assert!(alpha <= 0.001);
        assert_eq!(c.current_backlight(), Some(70), "值已写入 mock");
    }

    #[test]
    fn apply_below_floor_writes_floor_plus_mask() {
        let mut c = BrightnessController::new(MockIo::available(100));
        let (outcome, alpha) = c.apply(5);
        assert_eq!(
            outcome,
            BrightnessOutcome::BacklightPlusMask {
                backlight: 20,
                mask_alpha: 0.75
            }
        );
        assert!((alpha - 0.75).abs() < 0.001);
        assert_eq!(c.current_backlight(), Some(20));
    }

    #[test]
    fn apply_falls_back_to_mask_when_write_fails() {
        let mut c = BrightnessController::new(MockIo::failing_set());
        let (outcome, alpha) = c.apply(60);
        assert!(matches!(outcome, BrightnessOutcome::MaskOnly { .. }));
        assert!((alpha - 0.4).abs() < 0.001);
    }

    #[test]
    fn apply_unavailable_uses_mask_only() {
        let mut c = BrightnessController::new(MockIo::unavailable());
        let (outcome, alpha) = c.apply(50);
        assert!(matches!(outcome, BrightnessOutcome::MaskOnly { .. }));
        assert!((alpha - 0.5).abs() < 0.001);
    }

    #[test]
    fn available_cached_after_first_probe() {
        // 第一次探测后缓存（避免每次滑块拖动都探测）
        let mut c = BrightnessController::new(MockIo::available(100));
        assert!(c.is_available());
        assert!(c.is_available());
        assert_eq!(c.backlight_available, Some(true));
    }

    #[test]
    fn capture_initial_records_before_any_write() {
        let mut c = BrightnessController::new(MockIo::available(85));
        c.capture_initial();
        assert_eq!(c.initial_backlight, Some(85), "捕获启动原值");
        // 幂等：再次捕获不覆盖
        let _ = c.apply(40);
        c.capture_initial();
        assert_eq!(c.initial_backlight, Some(85), "二次捕获不覆盖首次值");
    }

    #[test]
    fn restore_initial_writes_back_and_is_idempotent() {
        let mut c = BrightnessController::new(MockIo::available(90));
        c.capture_initial();
        let _ = c.apply(30);
        assert_eq!(c.current_backlight(), Some(30));

        // 还原 → 背光回到 90
        assert!(c.restore_initial(), "应执行还原写入");
        assert_eq!(c.current_backlight(), Some(90));

        // 再次还原：已等于原值 → 不重复写（返回 false）
        assert!(!c.restore_initial(), "已还原时幂等无操作");
    }

    #[test]
    fn restore_initial_noop_without_capture() {
        let mut c = BrightnessController::new(MockIo::available(100));
        let _ = c.apply(50);
        assert!(!c.restore_initial(), "未捕获原值时不动作");
        assert_eq!(c.current_backlight(), Some(50), "值保持不动");
    }
}

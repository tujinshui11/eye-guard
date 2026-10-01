//! ALS（环境光传感器）支持——对照移植 `src/main/als.js` 的语义
//!
//! 实现变更（对照 JS 的 PowerShell 桥）：windows crate 直调 WinRT `LightSensor`
//! （去进程启动开销；JS 版单次采样 ~0.3–1s → Rust 版直调）。
//!
//! 三态语义（与 JS 版一致）：
//!   无硬件（GetDefault Err）→ available=false
//!   硬件存在但本次无读数（GetCurrentReading Err）→ available=true, lux=None
//!   有效读数 → available=true, lux=Some(lux)

use windows::Devices::Sensors::LightSensor;

/// ALS 读取结果（三态）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlsReading {
    pub available: bool,
    pub lux: Option<f64>,
}

/// 读取一次 ALS
pub fn read_als() -> AlsReading {
    let sensor = match LightSensor::GetDefault() {
        Ok(s) => s,
        Err(_) => {
            return AlsReading {
                available: false,
                lux: None,
            }
        }
    };
    match sensor.GetCurrentReading() {
        Ok(reading) => match reading.IlluminanceInLux() {
            Ok(lux) => AlsReading {
                available: true,
                lux: Some(lux as f64),
            },
            Err(_) => AlsReading {
                available: true,
                lux: None,
            },
        },
        Err(_) => AlsReading {
            available: true,
            lux: None,
        },
    }
}

/// 探测本机是否有可用的 ALS 硬件（读一次即知）
pub fn detect_als() -> bool {
    read_als().available
}

// ============================================================
// 探针（手动运行）：检测本机 ALS 行为
//   cargo test --test als_probe -- --ignored --nocapture
// ============================================================
#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "硬件探针：需手动运行（读取本机 ALS 一次）"]
    fn als_probe() {
        let r = super::read_als();
        println!("[als] available={} lux={:?}", r.available, r.lux);
        println!("[als] detect={}", super::detect_als());
    }
}

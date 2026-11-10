//! 운영체제 도구 출력 파서 (순수 함수 — 모든 플랫폼에서 단위 테스트 가능).
use super::{Acceleration, GpuInfo, Vram};
use serde_json::Value;

const MIB: u64 = 1024 * 1024;

pub fn vendor_from_name(name: &str) -> &'static str {
    let n = name.to_ascii_lowercase();
    if n.contains("nvidia") || n.contains("geforce") || n.contains("quadro") || n.contains("rtx") {
        "NVIDIA"
    } else if n.contains("amd") || n.contains("radeon") || n.contains("advanced micro") {
        "AMD"
    } else if n.contains("intel") {
        "Intel"
    } else if n.contains("apple") {
        "Apple"
    } else if n.contains("microsoft") {
        "Microsoft"
    } else {
        "기타"
    }
}

pub fn pci_vendor_name(id: &str) -> &'static str {
    match id.to_ascii_lowercase().as_str() {
        "0x10de" => "NVIDIA",
        "0x1002" => "AMD",
        "0x8086" => "Intel",
        _ => "기타",
    }
}

pub fn friendly_cpu_vendor(vendor_id: &str, brand: &str) -> String {
    let v = format!("{vendor_id} {brand}").to_ascii_lowercase();
    if v.contains("apple") {
        "Apple".into()
    } else if v.contains("intel") {
        "Intel".into()
    } else if v.contains("amd") {
        "AMD".into()
    } else if v.contains("qualcomm") || v.contains("snapdragon") {
        "Qualcomm".into()
    } else if vendor_id.is_empty() {
        "확인 불가".into()
    } else {
        vendor_id.to_string()
    }
}

/// `nvidia-smi --query-gpu=name,memory.total --format=csv,noheader,nounits`
pub fn parse_nvidia_smi(out: &str) -> Vec<GpuInfo> {
    out.lines()
        .filter_map(|l| {
            let (name, mem) = l.rsplit_once(',')?;
            let mib: u64 = mem.trim().parse().ok()?;
            Some(GpuInfo { vendor: "NVIDIA".into(), name: name.trim().to_string(), vram: Vram::Exact(mib * MIB) })
        })
        .collect()
}

fn as_array(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a.clone(),
        Value::Null => vec![],
        other => vec![other.clone()],
    }
}

/// Windows PowerShell 출력(JSON) 파서.
///
/// `AdapterRAM`은 32비트 값이라 4GB 이상을 표현하지 못한다. 레지스트리의
/// `HardwareInformation.qwMemorySize`(64비트)가 있으면 그것을 정확한 값으로 쓰고,
/// 없고 AdapterRAM이 상한 근처면 "4GB 이상"으로만 표시한다.
pub fn parse_windows_gpu_json(json: &str) -> Vec<GpuInfo> {
    let Ok(root) = serde_json::from_str::<Value>(json.trim()) else { return vec![] };
    let reg: Vec<(String, u64)> = as_array(&root["registry"])
        .iter()
        .filter_map(|r| {
            let desc = r["DriverDesc"].as_str()?.to_string();
            let q = r["HardwareInformation.qwMemorySize"].as_u64()?;
            Some((desc, q))
        })
        .collect();
    as_array(&root["controllers"])
        .iter()
        .filter_map(|c| {
            let name = c["Name"].as_str()?.trim().to_string();
            let compat = c["AdapterCompatibility"].as_str().unwrap_or("");
            let vendor = vendor_from_name(&format!("{compat} {name}"));
            // 원격 데스크톱/가상 디스플레이 어댑터는 제외
            if vendor == "Microsoft" || name.to_ascii_lowercase().contains("virtual") {
                return None;
            }
            let integrated = vendor == "Intel"
                || (vendor == "AMD" && name.to_ascii_lowercase().contains("graphics") && !name.to_ascii_lowercase().contains("rx"));
            let vram = if let Some((_, q)) = reg.iter().find(|(d, q)| d == &name && *q > 0) {
                if integrated { Vram::Shared } else { Vram::Exact(*q) }
            } else if integrated {
                Vram::Shared
            } else {
                match c["AdapterRAM"].as_u64() {
                    Some(b) if b >= 4095 * MIB => Vram::AtLeast(4096 * MIB),
                    Some(b) if b > 0 => Vram::Exact(b),
                    _ => Vram::Unknown,
                }
            };
            Some(GpuInfo { vendor: vendor.to_string(), name, vram })
        })
        .collect()
}

/// macOS `system_profiler SPDisplaysDataType -json` 파서.
pub fn parse_macos_displays_json(json: &str) -> Vec<GpuInfo> {
    let Ok(root) = serde_json::from_str::<Value>(json) else { return vec![] };
    as_array(&root["SPDisplaysDataType"])
        .iter()
        .filter_map(|g| {
            let name = g["sppci_model"].as_str().or_else(|| g["_name"].as_str())?.to_string();
            let vendor_raw = g["spdisplays_vendor"].as_str().unwrap_or("");
            let vendor = vendor_from_name(&format!("{vendor_raw} {name}"));
            // 예: "spdisplays_vram": "8 GB" / "4096 MB", 공유 메모리는 spdisplays_vram_shared
            let vram = if let Some(s) = g["spdisplays_vram"].as_str() {
                parse_size(s).map(Vram::Exact).unwrap_or(Vram::Unknown)
            } else if g.get("spdisplays_vram_shared").is_some() || vendor == "Apple" {
                Vram::Shared
            } else {
                Vram::Unknown
            };
            Some(GpuInfo { vendor: vendor.to_string(), name, vram })
        })
        .collect()
}

fn parse_size(s: &str) -> Option<u64> {
    let mut it = s.split_whitespace();
    let n: f64 = it.next()?.parse().ok()?;
    let unit = it.next()?.to_ascii_uppercase();
    let mul = match unit.as_str() {
        "GB" => 1024 * MIB,
        "MB" => MIB,
        _ => return None,
    };
    Some((n * mul as f64) as u64)
}

pub fn decide_acceleration(is_apple_silicon: bool, gpus: &[GpuInfo]) -> Acceleration {
    if is_apple_silicon {
        return Acceleration::AppleMetal;
    }
    let dedicated = |g: &&GpuInfo| matches!(g.vram, Vram::Exact(b) | Vram::AtLeast(b) if b >= 3 * 1024 * MIB);
    if gpus.iter().filter(dedicated).any(|g| g.vendor == "NVIDIA") {
        Acceleration::NvidiaCuda
    } else if gpus.iter().filter(dedicated).any(|g| g.vendor == "AMD") {
        Acceleration::AmdGpu
    } else {
        Acceleration::CpuOnly
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvidia_smi() {
        let g = parse_nvidia_smi("NVIDIA GeForce RTX 4070, 12282\nNVIDIA GeForce RTX 3060, 12288\n");
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].vram, Vram::Exact(12282 * MIB));
    }

    #[test]
    fn windows_registry_overrides_32bit_adapter_ram() {
        let json = r#"{"controllers":[
            {"Name":"NVIDIA GeForce RTX 4070","AdapterCompatibility":"NVIDIA","AdapterRAM":4293918720},
            {"Name":"Intel(R) UHD Graphics 770","AdapterCompatibility":"Intel Corporation","AdapterRAM":1073741824},
            {"Name":"Microsoft Remote Display Adapter","AdapterCompatibility":"Microsoft","AdapterRAM":null}],
          "registry":[{"DriverDesc":"NVIDIA GeForce RTX 4070","HardwareInformation.qwMemorySize":12878610432},
                      {"DriverDesc":"Intel(R) UHD Graphics 770","HardwareInformation.qwMemorySize":1073741824}]}"#;
        let g = parse_windows_gpu_json(json);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].vram, Vram::Exact(12878610432));
        assert_eq!(g[1].vram, Vram::Shared);
        assert_eq!(decide_acceleration(false, &g), Acceleration::NvidiaCuda);
    }

    #[test]
    fn windows_without_registry_does_not_guess() {
        // 컨트롤러가 하나일 때 PowerShell은 배열이 아닌 객체를 출력할 수 있다.
        let json = r#"{"controllers":{"Name":"AMD Radeon RX 7800 XT","AdapterCompatibility":"Advanced Micro Devices, Inc.","AdapterRAM":4293918720},"registry":[]}"#;
        let g = parse_windows_gpu_json(json);
        assert_eq!(g[0].vram, Vram::AtLeast(4096 * MIB));
        assert_eq!(decide_acceleration(false, &g), Acceleration::AmdGpu);
        assert!(parse_windows_gpu_json("not json").is_empty());
    }

    #[test]
    fn macos_displays() {
        let apple = r#"{"SPDisplaysDataType":[{"_name":"Apple M2 Pro","sppci_model":"Apple M2 Pro","spdisplays_vendor":"sppci_vendor_Apple","sppci_cores":"16"}]}"#;
        let g = parse_macos_displays_json(apple);
        assert_eq!(g[0].vendor, "Apple");
        assert_eq!(g[0].vram, Vram::Shared);
        let intel = r#"{"SPDisplaysDataType":[{"sppci_model":"AMD Radeon Pro 5500M","spdisplays_vendor":"sppci_vendor_amd","spdisplays_vram":"8 GB"},{"sppci_model":"Intel UHD Graphics 630","spdisplays_vendor":"Intel","spdisplays_vram_shared":"1536 MB"}]}"#;
        let g = parse_macos_displays_json(intel);
        assert_eq!(g[0].vram, Vram::Exact(8 * 1024 * MIB));
        assert_eq!(g[1].vram, Vram::Shared);
    }

    #[test]
    fn integrated_only_is_cpu() {
        let g = vec![GpuInfo { vendor: "Intel".into(), name: "Intel Iris Xe".into(), vram: Vram::Shared }];
        assert_eq!(decide_acceleration(false, &g), Acceleration::CpuOnly);
        assert_eq!(decide_acceleration(true, &[]), Acceleration::AppleMetal);
    }
}

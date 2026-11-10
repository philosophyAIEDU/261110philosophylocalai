//! Hardware Detection Layer — 컴퓨터 사양 진단.
//!
//! CPU/RAM/디스크는 `sysinfo`로, GPU는 운영체제별 시스템 도구로 확인한다.
//! - Windows: PowerShell CIM(`Win32_VideoController`) + 레지스트리 `qwMemorySize`, `nvidia-smi`
//! - macOS  : `system_profiler SPDisplaysDataType -json` (Apple Silicon은 통합 메모리)
//! - Linux  : `nvidia-smi`, `/sys/class/drm` (개발·테스트용)
//!
//! VRAM을 확실히 알 수 없으면 `Vram::Unknown` / `Vram::AtLeast`로 표시하고 추측하지 않는다.
pub mod parse;

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

pub const GB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "bytes", rename_all = "snake_case")]
pub enum Vram {
    /// 전용 그래픽 메모리 크기를 정확히 확인함
    Exact(u64),
    /// 최소 이 크기 이상 (예: Windows 32비트 필드 상한 4GB)
    AtLeast(u64),
    /// 시스템 메모리와 공유 (Apple Silicon 통합 메모리, 내장 그래픽)
    Shared,
    /// 확인 불가
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuInfo {
    pub vendor: String,
    pub name: String,
    pub vram: Vram,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Acceleration {
    /// Apple Silicon Metal (통합 메모리)
    AppleMetal,
    /// NVIDIA CUDA
    NvidiaCuda,
    /// AMD 그래픽 (Ollama 지원 여부는 모델·드라이버에 따라 다름)
    AmdGpu,
    /// GPU 가속 없음 — CPU로 실행
    CpuOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    pub cpu_vendor: String,
    pub cpu_model: String,
    pub cpu_physical_cores: usize,
    pub cpu_logical_cores: usize,
    pub total_ram: u64,
    pub available_ram: u64,
    pub gpus: Vec<GpuInfo>,
    pub acceleration: Acceleration,
    pub is_apple_silicon: bool,
    /// 앱 데이터(모델 저장) 드라이브의 여유 공간
    pub disk_free: u64,
    pub disk_total: u64,
    /// 진단 중 확인하지 못한 항목에 대한 안내 (한국어)
    pub notes: Vec<String>,
}

impl HardwareProfile {
    /// 가속에 쓸 수 있는 "확실한" 전용 VRAM (가장 큰 GPU 기준).
    pub fn best_dedicated_vram(&self) -> Option<u64> {
        self.gpus
            .iter()
            .filter_map(|g| match g.vram {
                Vram::Exact(b) | Vram::AtLeast(b) => Some(b),
                _ => None,
            })
            .max()
    }

    /// 교수님께 보여 드릴 쉬운 한국어 요약.
    pub fn summary_ko(&self) -> Vec<String> {
        let ram_gb = self.total_ram as f64 / GB as f64;
        let mut out = Vec::new();
        let headline = if ram_gb < 7.5 {
            "교수님의 컴퓨터는 가벼운 AI 모델로 간단한 대화와 짧은 문서 요약을 실행할 수 있습니다."
        } else if ram_gb < 15.5 {
            "교수님의 컴퓨터는 일반적인 문서 분석과 AI 채팅을 실행하기에 적합합니다."
        } else if ram_gb < 31.5 {
            "교수님의 컴퓨터는 문서 분석과 AI 채팅을 여유 있게 실행할 수 있습니다."
        } else {
            "교수님의 컴퓨터는 고성능 AI 모델까지 실행할 수 있는 충분한 사양입니다."
        };
        out.push(headline.to_string());
        match self.acceleration {
            Acceleration::AppleMetal => out.push("Apple 칩의 그래픽 가속을 사용해 AI가 빠르게 응답합니다.".into()),
            Acceleration::NvidiaCuda => out.push("NVIDIA 그래픽 카드를 사용해 AI가 빠르게 응답할 수 있습니다.".into()),
            Acceleration::AmdGpu => out.push("AMD 그래픽 카드가 있습니다. 환경에 따라 그래픽 가속이 적용되지 않을 수 있습니다.".into()),
            Acceleration::CpuOnly => out.push("그래픽 가속 없이 실행되므로, 더 큰 AI 모델은 실행 속도가 느려질 수 있습니다.".into()),
        }
        if ram_gb < 15.5 {
            out.push("더 큰 AI 모델은 실행 속도가 느려지거나 실행되지 않을 수 있습니다.".into());
        }
        let free_gb = self.disk_free as f64 / GB as f64;
        if free_gb < 10.0 {
            out.push(format!("저장 공간이 {:.0}GB 남아 있습니다. AI 모델을 내려받기 전에 공간을 확보해 주세요.", free_gb));
        }
        out
    }
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let mut c = Command::new(cmd);
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = c.output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn detect_nvidia() -> Vec<GpuInfo> {
    run("nvidia-smi", &["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"])
        .map(|s| parse::parse_nvidia_smi(&s))
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn detect_gpus(notes: &mut Vec<String>) -> Vec<GpuInfo> {
    let nvidia = detect_nvidia();
    let script = r#"$ErrorActionPreference='SilentlyContinue';
$v = Get-CimInstance Win32_VideoController | Select-Object Name,AdapterCompatibility,AdapterRAM;
$r = Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0*' | Select-Object DriverDesc,'HardwareInformation.qwMemorySize';
@{controllers=@($v);registry=@($r)} | ConvertTo-Json -Depth 4 -Compress"#;
    let mut gpus = run("powershell.exe", &["-NoProfile", "-NonInteractive", "-Command", script])
        .map(|s| parse::parse_windows_gpu_json(&s))
        .unwrap_or_else(|| {
            notes.push("그래픽 카드 정보를 확인하지 못했습니다.".into());
            Vec::new()
        });
    // nvidia-smi 값이 가장 정확하므로 우선한다.
    for n in nvidia {
        if let Some(g) = gpus.iter_mut().find(|g| g.name.contains(&n.name) || n.name.contains(&g.name)) {
            g.vram = n.vram.clone();
        } else {
            gpus.push(n);
        }
    }
    gpus
}

#[cfg(target_os = "macos")]
fn detect_gpus(notes: &mut Vec<String>) -> Vec<GpuInfo> {
    run("system_profiler", &["SPDisplaysDataType", "-json"])
        .map(|s| parse::parse_macos_displays_json(&s))
        .unwrap_or_else(|| {
            notes.push("그래픽 정보를 확인하지 못했습니다.".into());
            Vec::new()
        })
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn detect_gpus(_notes: &mut Vec<String>) -> Vec<GpuInfo> {
    let mut gpus = detect_nvidia();
    if gpus.is_empty() {
        if let Ok(rd) = std::fs::read_dir("/sys/class/drm") {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if !name.starts_with("card") || name.contains('-') {
                    continue;
                }
                let dev = e.path().join("device");
                let vendor_id = std::fs::read_to_string(dev.join("vendor")).unwrap_or_default();
                let vendor = parse::pci_vendor_name(vendor_id.trim());
                let vram = std::fs::read_to_string(dev.join("mem_info_vram_total"))
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .map(Vram::Exact)
                    .unwrap_or(Vram::Unknown);
                gpus.push(GpuInfo { vendor: vendor.to_string(), name: format!("{vendor} 그래픽"), vram });
            }
        }
    }
    gpus
}

/// 컴퓨터 사양을 진단한다. `data_dir`은 모델이 저장될 폴더 (여유 공간 확인용).
pub fn detect(data_dir: &Path) -> HardwareProfile {
    use sysinfo::{Disks, System};
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    let mut notes = Vec::new();

    let cpu = sys.cpus().first();
    let cpu_model = cpu.map(|c| c.brand().trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "확인 불가".into());
    let cpu_vendor = cpu.map(|c| parse::friendly_cpu_vendor(c.vendor_id(), &cpu_model)).unwrap_or_else(|| "확인 불가".into());
    let arch = std::env::consts::ARCH.to_string();
    let is_apple_silicon = cfg!(target_os = "macos") && arch == "aarch64";

    let mut gpus = detect_gpus(&mut notes);
    if is_apple_silicon {
        for g in gpus.iter_mut() {
            g.vram = Vram::Shared;
        }
    }
    let acceleration = parse::decide_acceleration(is_apple_silicon, &gpus);
    if gpus.iter().any(|g| matches!(g.vram, Vram::Unknown | Vram::AtLeast(_))) && !is_apple_silicon {
        notes.push("그래픽 메모리 크기를 정확히 확인하지 못한 장치가 있어, 추천은 보수적으로 계산했습니다.".into());
    }

    // data_dir이 속한 디스크 (가장 긴 마운트 지점 일치)
    let disks = Disks::new_with_refreshed_list();
    let target = std::fs::canonicalize(data_dir).unwrap_or_else(|_| {
        data_dir.ancestors().find(|p| p.exists()).map(|p| p.to_path_buf()).unwrap_or_else(|| data_dir.to_path_buf())
    });
    let (disk_free, disk_total) = disks
        .iter()
        .filter(|d| target.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| (d.available_space(), d.total_space()))
        .unwrap_or_else(|| {
            notes.push("저장 공간을 확인하지 못했습니다.".into());
            (0, 0)
        });

    HardwareProfile {
        os_name: System::name().unwrap_or_else(|| std::env::consts::OS.to_string()),
        os_version: System::os_version().unwrap_or_default(),
        arch,
        cpu_vendor,
        cpu_model,
        cpu_physical_cores: sys.physical_core_count().unwrap_or(0),
        cpu_logical_cores: sys.cpus().len(),
        total_ram: sys.total_memory(),
        available_ram: sys.available_memory(),
        gpus,
        acceleration,
        is_apple_silicon,
        disk_free,
        disk_total,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_runs_on_this_machine() {
        let p = detect(&std::env::temp_dir());
        assert!(p.total_ram > 0);
        assert!(p.cpu_logical_cores > 0);
        assert!(p.disk_total > 0, "디스크 정보를 찾아야 함");
        assert!(!p.summary_ko().is_empty());
    }
}

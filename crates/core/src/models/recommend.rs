//! 모델 추천 엔진.
//!
//! RAM 크기만 비교하지 않고 다음을 함께 본다.
//! 1. **메모리 예산**: 가속 방식에 따라 모델이 쓸 수 있는 메모리가 다르다.
//!    - Apple Silicon: 통합 메모리의 약 65% (GPU 가속)
//!    - 전용 GPU: VRAM의 90%에 들어가면 "빠름", 넘으면 RAM과 나눠 실행(느려짐)
//!    - CPU 전용: 전체 RAM의 약 60% (운영체제·다른 프로그램 몫 제외)
//! 2. **속도 등급**: 가속 여부·모델 규모·CPU 코어 수로 추정 (실측값 아님)
//! 3. **품질 점수**: 사용 목적별 가중치(한국어/요약/추론)
//! 4. **저장 공간**: 다운로드 용량 + 여유분
//! 5. **라이선스**: 표시하여 사용자가 확인하도록 함
use super::{Catalog, CatalogModel};
use crate::hardware::{Acceleration, HardwareProfile, GB};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    /// 일반 대화·문서 초안
    #[default]
    General,
    /// 강의자료·문서 요약
    Documents,
    /// 연구자료 분석·추론
    Research,
    /// 데이터 해석·보고서
    Data,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    /// 여유 있게 실행
    Comfortable,
    /// 실행 가능하나 다른 프로그램을 닫아야 할 수 있음
    Tight,
    /// 이 컴퓨터에는 너무 큼
    TooLarge,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Speed {
    Fast,
    Moderate,
    Slow,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// 빠르고 가벼운 AI
    Light,
    /// 균형 잡힌 추천 AI
    Balanced,
    /// 고성능 AI
    HighQuality,
}

#[derive(Debug, Clone, Serialize)]
pub struct Assessment {
    pub model: CatalogModel,
    pub fit: Fit,
    pub speed: Speed,
    pub quality_score: f64,
    pub enough_disk: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    pub tier: Tier,
    pub tier_label: String,
    pub assessment: Assessment,
    /// 추천 근거 (한국어)
    pub reasons: Vec<String>,
    /// 주의 사항 (한국어)
    pub cautions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecommendationResult {
    pub recommendations: Vec<Recommendation>,
    /// 모든 카탈로그 모델의 평가 (고급 화면용)
    pub all: Vec<Assessment>,
    pub memory_budget_gb: f64,
    pub explanation: String,
}

struct Budget {
    /// 가속된 상태로 쓸 수 있는 메모리 (GB)
    fast_gb: f64,
    /// 느리더라도 실행 가능한 최대 메모리 (GB)
    max_gb: f64,
}

fn budget(hw: &HardwareProfile) -> Budget {
    let ram = hw.total_ram as f64 / GB as f64;
    match hw.acceleration {
        Acceleration::AppleMetal => {
            let b = ram * 0.65;
            Budget { fast_gb: b, max_gb: b }
        }
        Acceleration::NvidiaCuda | Acceleration::AmdGpu => {
            let vram = hw.best_dedicated_vram().unwrap_or(0) as f64 / GB as f64;
            Budget { fast_gb: vram * 0.9, max_gb: (ram * 0.6).max(vram * 0.9) }
        }
        Acceleration::CpuOnly => Budget { fast_gb: 0.0, max_gb: ram * 0.6 },
    }
}

fn quality(m: &CatalogModel, p: Purpose) -> f64 {
    let (wk, ws, wr) = match p {
        Purpose::General => (0.45, 0.30, 0.25),
        Purpose::Documents => (0.35, 0.45, 0.20),
        Purpose::Research => (0.25, 0.30, 0.45),
        Purpose::Data => (0.25, 0.25, 0.50),
    };
    m.korean as f64 * wk + m.summarization as f64 * ws + m.reasoning as f64 * wr
}

pub fn assess(m: &CatalogModel, hw: &HardwareProfile, p: Purpose) -> Assessment {
    let b = budget(hw);
    let need = m.est_ram_gb;
    let fit = if need <= b.max_gb * 0.8 {
        Fit::Comfortable
    } else if need <= b.max_gb {
        Fit::Tight
    } else {
        Fit::TooLarge
    };
    let accelerated = need <= b.fast_gb;
    let speed = if accelerated {
        // 가속되더라도 아주 큰 모델은 중간 속도로 본다.
        if hw.acceleration == Acceleration::AppleMetal && m.params_b > 20.0 { Speed::Moderate } else { Speed::Fast }
    } else {
        // CPU(또는 일부만 GPU) 실행: 코어 수와 모델 규모로 추정
        let cores = hw.cpu_physical_cores.max(1) as f64;
        let load = m.params_b / cores; // 코어당 부담
        if load <= 0.6 {
            Speed::Fast
        } else if load <= 1.6 {
            Speed::Moderate
        } else {
            Speed::Slow
        }
    };
    let disk_free_gb = hw.disk_free as f64 / GB as f64;
    Assessment {
        model: m.clone(),
        fit,
        speed,
        quality_score: (quality(m, p) * 100.0).round() / 100.0,
        // 다운로드 중 임시 공간과 여유분 2GB
        enough_disk: hw.disk_total == 0 || disk_free_gb >= m.download_gb * 1.1 + 2.0,
    }
}

fn reasons_for(a: &Assessment, hw: &HardwareProfile, tier: Tier) -> (Vec<String>, Vec<String>) {
    let m = &a.model;
    let mut r = Vec::new();
    let mut c = Vec::new();
    r.push(match tier {
        Tier::Light => "응답이 빠르고 컴퓨터에 부담이 적은 모델입니다.".to_string(),
        Tier::Balanced => "교수님 컴퓨터에서 속도와 품질의 균형이 가장 좋을 것으로 예상되는 모델입니다.".to_string(),
        Tier::HighQuality => "교수님 컴퓨터에서 실행할 수 있는 모델 가운데 결과 품질이 가장 높을 것으로 기대되는 모델입니다.".to_string(),
    });
    r.push(format!(
        "실행에 약 {:.0}GB의 메모리가 필요하며, 교수님 컴퓨터의 메모리({:.0}GB)로 {} 실행할 수 있습니다.",
        m.est_ram_gb.ceil(),
        hw.total_ram as f64 / GB as f64,
        if a.fit == Fit::Comfortable { "여유 있게" } else { "빠듯하게" }
    ));
    r.push(match a.speed {
        Speed::Fast => "응답 속도: 빠름 (예상)".to_string(),
        Speed::Moderate => "응답 속도: 보통 (예상)".to_string(),
        Speed::Slow => "응답 속도: 느림 (예상) — 긴 답변은 시간이 걸릴 수 있습니다.".to_string(),
    });
    r.push(format!("내려받는 용량은 약 {:.1}GB입니다.", m.download_gb));
    r.push(format!("라이선스: {} ({})", m.license, m.publisher));
    if a.fit == Fit::Tight {
        c.push("메모리를 많이 사용합니다. 다른 프로그램을 닫고 사용하시기를 권합니다.".into());
    }
    if !a.enough_disk {
        c.push("저장 공간이 부족합니다. 공간을 확보한 뒤 내려받아 주세요.".into());
    }
    if m.license.contains("Gemma") {
        c.push("Gemma 이용 약관이 적용됩니다. 사용 전에 약관을 확인해 주세요.".into());
    }
    c.push("속도와 품질은 예상치이며, 실제 사용 환경에 따라 달라질 수 있습니다.".into());
    (r, c)
}

pub fn tier_label(t: Tier) -> &'static str {
    match t {
        Tier::Light => "빠르고 가벼운 AI",
        Tier::Balanced => "균형 잡힌 추천 AI",
        Tier::HighQuality => "고성능 AI",
    }
}

/// 품질이 높은 쪽, 같으면 메모리를 덜 쓰는 쪽이 크다.
fn by_quality(x: &Assessment, y: &Assessment) -> std::cmp::Ordering {
    x.quality_score.partial_cmp(&y.quality_score).unwrap().then(y.model.est_ram_gb.partial_cmp(&x.model.est_ram_gb).unwrap())
}

/// 하드웨어와 사용 목적에 맞는 모델 2~3개를 추천한다.
pub fn recommend(catalog: &Catalog, hw: &HardwareProfile, purpose: Purpose) -> RecommendationResult {
    let all: Vec<Assessment> = catalog.models.iter().map(|m| assess(m, hw, purpose)).collect();
    let runnable: Vec<&Assessment> = all.iter().filter(|a| a.fit != Fit::TooLarge).collect();
    let b = budget(hw);
    let mut picks: Vec<(Tier, &Assessment)> = Vec::new();

    // 고성능 후보: 실행 가능한 모델 중 품질 최고
    let best = runnable.iter().max_by(|x, y| by_quality(x, y)).copied();
    // 균형: 여유 있게 들어가고 느리지 않은 모델 중 품질 최고
    let mut balanced = runnable
        .iter()
        .filter(|a| a.fit == Fit::Comfortable && a.speed != Speed::Slow)
        .max_by(|x, y| by_quality(x, y))
        .or_else(|| runnable.iter().filter(|a| a.fit == Fit::Comfortable).max_by(|x, y| by_quality(x, y)))
        .or_else(|| runnable.iter().min_by(|x, y| x.model.est_ram_gb.partial_cmp(&y.model.est_ram_gb).unwrap()))
        .copied();
    // 균형 모델이 곧 최고 품질 모델이면, 그 모델을 '고성능'으로 두고
    // 한 단계 가벼운 모델을 '균형'으로 내려 세 가지 선택지를 만든다.
    if let (Some(bal), Some(top)) = (balanced, best) {
        if bal.model.id == top.model.id {
            let step_down = runnable
                .iter()
                .filter(|a| a.fit == Fit::Comfortable && a.model.est_ram_gb <= top.model.est_ram_gb * 0.75 && a.model.params_b >= 6.0)
                .max_by(|x, y| by_quality(x, y))
                .copied();
            if let Some(sd) = step_down {
                balanced = Some(sd);
            }
        }
    }

    if let Some(bal) = balanced {
        // 가벼움: 균형 모델보다 작고 빠른 모델 중 품질 최고 (초경량 1b는 다른 선택지가 없을 때만)
        let light = runnable
            .iter()
            .filter(|a| a.model.est_ram_gb < bal.model.est_ram_gb && a.speed <= bal.speed)
            .filter(|a| a.model.params_b >= 2.0)
            .filter(|a| a.speed == Speed::Fast || a.model.est_ram_gb <= bal.model.est_ram_gb * 0.7)
            .max_by(|x, y| by_quality(x, y))
            .or_else(|| runnable.iter().filter(|a| a.model.est_ram_gb < bal.model.est_ram_gb).max_by(|x, y| by_quality(x, y)))
            .copied();
        // 고성능: 균형 모델보다 품질이 높은, 실행 가능한 모델 중 품질 최고
        let high = runnable
            .iter()
            .filter(|a| a.quality_score > bal.quality_score && a.model.id != bal.model.id)
            .max_by(|x, y| by_quality(x, y))
            .copied();
        if let Some(l) = light {
            picks.push((Tier::Light, l));
        }
        picks.push((Tier::Balanced, bal));
        if let Some(h) = high {
            picks.push((Tier::HighQuality, h));
        }
    }

    let recommendations = picks
        .into_iter()
        .map(|(tier, a)| {
            let (reasons, cautions) = reasons_for(a, hw, tier);
            Recommendation { tier, tier_label: tier_label(tier).to_string(), assessment: a.clone(), reasons, cautions }
        })
        .collect::<Vec<_>>();

    let explanation = if recommendations.is_empty() {
        "교수님 컴퓨터의 메모리로 실행할 수 있는 모델을 찾지 못했습니다. 메모리 8GB 이상의 컴퓨터를 권장합니다.".to_string()
    } else {
        match hw.acceleration {
            Acceleration::AppleMetal => format!("Apple 칩의 통합 메모리 가운데 약 {:.0}GB를 AI가 사용할 수 있다고 보고 추천했습니다.", b.max_gb),
            Acceleration::NvidiaCuda | Acceleration::AmdGpu => format!(
                "그래픽 메모리 약 {:.0}GB 안에 들어가는 모델은 빠르게, 그보다 큰 모델은 시스템 메모리(약 {:.0}GB 사용 가능)로 느리게 실행된다고 보고 추천했습니다.",
                b.fast_gb, b.max_gb
            ),
            Acceleration::CpuOnly => format!("그래픽 가속 없이 시스템 메모리 가운데 약 {:.0}GB를 AI가 사용할 수 있다고 보고 추천했습니다.", b.max_gb),
        }
    };
    RecommendationResult { recommendations, all, memory_budget_gb: (b.max_gb * 10.0).round() / 10.0, explanation }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::{GpuInfo, Vram};

    fn hw(ram_gb: u64, cores: usize, accel: Acceleration, vram_gb: Option<u64>) -> HardwareProfile {
        HardwareProfile {
            os_name: "test".into(),
            os_version: "".into(),
            arch: "x86_64".into(),
            cpu_vendor: "Intel".into(),
            cpu_model: "test".into(),
            cpu_physical_cores: cores,
            cpu_logical_cores: cores * 2,
            total_ram: ram_gb * GB,
            available_ram: ram_gb * GB / 2,
            gpus: vram_gb
                .map(|v| vec![GpuInfo { vendor: "NVIDIA".into(), name: "x".into(), vram: Vram::Exact(v * GB) }])
                .unwrap_or_default(),
            acceleration: accel,
            is_apple_silicon: accel == Acceleration::AppleMetal,
            disk_free: 200 * GB,
            disk_total: 500 * GB,
            notes: vec![],
        }
    }

    fn ids(r: &RecommendationResult) -> Vec<(Tier, String)> {
        r.recommendations.iter().map(|x| (x.tier, x.assessment.model.id.clone())).collect()
    }

    #[test]
    fn low_end_8gb_cpu_gets_small_models_only() {
        let r = recommend(&Catalog::builtin(), &hw(8, 4, Acceleration::CpuOnly, None), Purpose::General);
        assert!(!r.recommendations.is_empty());
        for rec in &r.recommendations {
            assert!(rec.assessment.model.est_ram_gb <= 8.0 * 0.6 + 0.01, "{:?}", ids(&r));
        }
    }

    #[test]
    fn typical_16gb_laptop_gets_two_or_three() {
        let r = recommend(&Catalog::builtin(), &hw(16, 8, Acceleration::CpuOnly, None), Purpose::Documents);
        let n = r.recommendations.len();
        assert!((2..=3).contains(&n), "{:?}", ids(&r));
        let bal = r.recommendations.iter().find(|x| x.tier == Tier::Balanced).unwrap();
        assert_eq!(bal.assessment.fit, Fit::Comfortable);
        assert_ne!(bal.assessment.speed, Speed::Slow);
        // 추천 모델은 서로 달라야 한다
        let mut set = std::collections::HashSet::new();
        assert!(r.recommendations.iter().all(|x| set.insert(x.assessment.model.id.clone())));
    }

    #[test]
    fn apple_silicon_32gb_gets_larger_models_than_cpu_only() {
        let c = Catalog::builtin();
        let mac = recommend(&c, &hw(32, 10, Acceleration::AppleMetal, None), Purpose::General);
        let pc = recommend(&c, &hw(32, 6, Acceleration::CpuOnly, None), Purpose::General);
        let bal = |r: &RecommendationResult| r.recommendations.iter().find(|x| x.tier == Tier::Balanced).unwrap().assessment.model.params_b;
        assert!(bal(&mac) >= bal(&pc), "mac {:?} pc {:?}", ids(&mac), ids(&pc));
        assert_eq!(mac.recommendations.len(), 3);
    }

    #[test]
    fn gpu_vram_makes_models_fast() {
        let c = Catalog::builtin();
        let h = hw(32, 8, Acceleration::NvidiaCuda, Some(12));
        let a = assess(c.get("qwen3:8b").unwrap(), &h, Purpose::General);
        assert_eq!(a.speed, Speed::Fast);
        let big = assess(c.get("gemma3:27b").unwrap(), &h, Purpose::General);
        assert_eq!(big.fit, Fit::TooLarge); // 32GB*0.6=19.2 < 21
    }

    #[test]
    fn too_little_memory_recommends_nothing_but_explains() {
        let r = recommend(&Catalog::builtin(), &hw(2, 2, Acceleration::CpuOnly, None), Purpose::General);
        assert!(r.recommendations.is_empty());
        assert!(r.explanation.contains("찾지 못했습니다"));
    }

    #[test]
    fn disk_shortage_is_flagged() {
        let mut h = hw(16, 8, Acceleration::CpuOnly, None);
        h.disk_free = 3 * GB;
        let r = recommend(&Catalog::builtin(), &h, Purpose::General);
        assert!(r.recommendations.iter().any(|x| !x.assessment.enough_disk));
        assert!(r.recommendations.iter().filter(|x| !x.assessment.enough_disk).all(|x| x.cautions.iter().any(|c| c.contains("저장 공간"))));
    }

    #[test]
    fn purpose_changes_ranking() {
        let c = Catalog::builtin();
        let g = quality(c.get("gemma3:12b").unwrap(), Purpose::General);
        let q = quality(c.get("qwen3:14b").unwrap(), Purpose::General);
        let gr = quality(c.get("gemma3:12b").unwrap(), Purpose::Research);
        let qr = quality(c.get("qwen3:14b").unwrap(), Purpose::Research);
        assert!(qr - gr > q - g);
    }
}

#[cfg(test)]
mod print_table {
    use super::*;
    use crate::hardware::{GpuInfo, Vram};
    #[test]
    #[ignore]
    fn table() {
        let c = Catalog::builtin();
        for (ram, cores, acc, vram) in [(8, 4, Acceleration::CpuOnly, None), (8, 8, Acceleration::AppleMetal, None), (16, 6, Acceleration::CpuOnly, None), (16, 8, Acceleration::AppleMetal, None), (16, 8, Acceleration::NvidiaCuda, Some(8u64)), (32, 8, Acceleration::CpuOnly, None), (32, 10, Acceleration::AppleMetal, None), (32, 8, Acceleration::NvidiaCuda, Some(12)), (64, 12, Acceleration::AppleMetal, None), (64, 16, Acceleration::NvidiaCuda, Some(24))] {
            let hw = HardwareProfile { os_name: "t".into(), os_version: "".into(), arch: "".into(), cpu_vendor: "".into(), cpu_model: "".into(), cpu_physical_cores: cores, cpu_logical_cores: cores, total_ram: ram * GB, available_ram: 0,
                gpus: vram.map(|v| vec![GpuInfo { vendor: "NVIDIA".into(), name: "x".into(), vram: Vram::Exact(v * GB) }]).unwrap_or_default(), acceleration: acc, is_apple_silicon: acc == Acceleration::AppleMetal, disk_free: 500 * GB, disk_total: 900 * GB, notes: vec![] };
            let r = recommend(&c, &hw, Purpose::General);
            println!("{ram}GB {cores}c {acc:?} {vram:?}: {:?}", r.recommendations.iter().map(|x| format!("{:?}={} {:?}/{:?}", x.tier, x.assessment.model.id, x.assessment.fit, x.assessment.speed)).collect::<Vec<_>>());
        }
    }
}

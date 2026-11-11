//! Model Management Layer (엔진 부분) — 관리형 Ollama 실행 엔진.
//!
//! - 공식 GitHub 릴리스의 독립 실행 파일을 **고정된 버전 + 고정된 SHA-256**으로 내려받아
//!   앱 데이터 폴더에 설치한다 (관리자 권한 불필요, 시스템에 설치된 Ollama와 분리).
//! - 엔진은 `127.0.0.1`의 임의 포트에서, 클라우드 기능을 끈 채, 앱 전용 모델 폴더로 실행한다.
//! - 앱 종료 시 프로세스를 정리하고, 비정상 종료로 남은 프로세스는 다음 실행 때 정리한다.
use crate::error::{AppError, AppResult};
use crate::security::NetGuard;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use url::Url;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EngineAsset {
    pub file: String,
    /// 공식 릴리스 `sha256sum.txt`의 값. 없으면 설치를 거부한다 (fail-closed).
    pub sha256: Option<String>,
    /// 압축을 푼 폴더 기준 실행 파일 경로
    pub binary: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EngineManifest {
    pub engine: String,
    pub version: String,
    pub source: String,
    pub license: String,
    pub base_url: String,
    pub assets: HashMap<String, EngineAsset>,
}

impl EngineManifest {
    pub fn builtin() -> Self {
        serde_json::from_str(include_str!("engine-manifest.json")).expect("엔진 매니페스트 JSON 오류")
    }

    /// 현재 운영체제·CPU에 맞는 자산 키.
    pub fn platform_key() -> Option<&'static str> {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("windows", "x86_64") => Some("windows-x86_64"),
            ("windows", "aarch64") => Some("windows-aarch64"),
            ("macos", _) => Some("macos-universal"),
            ("linux", "x86_64") => Some("linux-x86_64"),
            ("linux", "aarch64") => Some("linux-aarch64"),
            _ => None,
        }
    }

    pub fn asset_for_this_platform(&self) -> AppResult<&EngineAsset> {
        let key = Self::platform_key().ok_or_else(|| AppError::Unsupported("이 운영체제에서는 AI 실행 엔진을 지원하지 않습니다".into()))?;
        self.assets.get(key).ok_or_else(|| AppError::Unsupported("이 컴퓨터용 AI 실행 엔진이 없습니다".into()))
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct InstallProgress {
    pub stage: String,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineStatus {
    pub installed: bool,
    pub version: String,
    pub running: bool,
    pub port: Option<u16>,
    /// 무결성 값이 앱에 등록되어 있어 설치가 가능한지
    pub installable: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct LocalOnlyReport {
    /// 엔진이 클라우드 기능 꺼짐을 확인해 줌
    pub cloud_disabled: bool,
    /// 이 컴퓨터의 외부 주소로는 엔진에 접속할 수 없음을 실제 연결 시도로 확인
    pub loopback_only: bool,
    /// 선택된 모델이 이 컴퓨터에 설치된 로컬 모델임
    pub model_local: bool,
    /// 세 항목이 모두 확인된 경우에만 true — '로컬 전용' 표시의 근거
    pub verified: bool,
    pub details: Vec<String>,
}

pub struct EngineManager {
    data_dir: PathBuf,
    manifest: EngineManifest,
    guard: Arc<NetGuard>,
    child: tokio::sync::Mutex<Option<(tokio::process::Child, u16)>>,
    /// 개발·테스트 전용: 설치 절차 없이 사용할 엔진 실행 파일
    dev_binary: Option<PathBuf>,
    /// 개발·테스트 전용: 엔진에 추가로 전달할 환경변수
    dev_env: Vec<(String, String)>,
}

impl EngineManager {
    pub fn new(data_dir: PathBuf, guard: Arc<NetGuard>) -> Self {
        Self { data_dir, manifest: EngineManifest::builtin(), guard, child: tokio::sync::Mutex::new(None), dev_binary: None, dev_env: Vec::new() }
    }

    /// 개발·테스트 전용. 릴리스 빌드의 앱 코드에서는 호출하지 않는다.
    pub fn with_dev_binary(mut self, bin: PathBuf) -> Self {
        self.dev_binary = Some(bin);
        self
    }

    /// 개발·테스트 전용.
    pub fn with_dev_env(mut self, k: &str, v: &str) -> Self {
        self.dev_env.push((k.to_string(), v.to_string()));
        self
    }

    pub fn manifest(&self) -> &EngineManifest {
        &self.manifest
    }

    fn engine_root(&self) -> PathBuf {
        self.data_dir.join("engine")
    }

    fn install_dir(&self) -> PathBuf {
        self.engine_root().join(&self.manifest.version)
    }

    pub fn models_dir(&self) -> PathBuf {
        self.data_dir.join("models")
    }

    fn pid_file(&self) -> PathBuf {
        self.engine_root().join("engine.pid")
    }

    pub fn binary_path(&self) -> AppResult<PathBuf> {
        if let Some(b) = &self.dev_binary {
            return Ok(b.clone());
        }
        Ok(self.install_dir().join(&self.manifest.asset_for_this_platform()?.binary))
    }

    pub fn is_installed(&self) -> bool {
        if self.dev_binary.is_some() {
            return true;
        }
        self.install_dir().join(".installed").exists() && self.binary_path().map(|p| p.exists()).unwrap_or(false)
    }

    pub async fn status(&self) -> EngineStatus {
        let port = self.child.lock().await.as_ref().map(|c| c.1);
        EngineStatus {
            installed: self.is_installed(),
            version: self.manifest.version.clone(),
            running: port.is_some(),
            port,
            installable: self.dev_binary.is_some()
                || self.manifest.asset_for_this_platform().map(|a| a.sha256.is_some()).unwrap_or(false),
        }
    }

    /// 공식 배포 파일을 내려받아 무결성을 확인하고 설치한다.
    pub async fn install(&self, mut on_progress: impl FnMut(InstallProgress) + Send, cancel: CancellationToken) -> AppResult<()> {
        if self.is_installed() {
            return Ok(());
        }
        let asset = self.manifest.asset_for_this_platform()?.clone();
        let expected = asset
            .sha256
            .clone()
            .ok_or_else(|| AppError::Blocked("이 빌드에는 AI 실행 엔진의 무결성 정보가 등록되어 있지 않아 설치할 수 없습니다".into()))?;
        let url = Url::parse(&self.manifest.base_url)
            .and_then(|b| b.join(&asset.file))
            .map_err(|_| AppError::Other("엔진 주소가 올바르지 않습니다".into()))?;
        let root = self.engine_root();
        tokio::fs::create_dir_all(&root).await?;
        let part = root.join(format!("{}.part", asset.file));

        // 이어받기: 받은 부분이 있으면 Range 요청
        let have = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
        let mut req = self.guard.engine_download_request(&url, &format!("엔진 v{} 내려받기", self.manifest.version))?;
        if have > 0 {
            req = req.header("Range", format!("bytes={have}-"));
        }
        on_progress(InstallProgress { stage: "AI 실행 엔진을 내려받는 중".into(), ..Default::default() });
        let resp = req.send().await.map_err(|_| AppError::Network("인터넷 연결을 확인해 주세요".into()))?;
        let status = resp.status();
        if !(status.is_success()) {
            return Err(AppError::Network(format!("AI 실행 엔진을 내려받지 못했습니다 (코드 {})", status.as_u16())));
        }
        let resumed = status.as_u16() == 206 && have > 0;
        let mut completed = if resumed { have } else { 0 };
        let total = resp.content_length().map(|l| l + completed).unwrap_or(0);
        if let Some(need) = total.checked_sub(completed) {
            check_free_space(&root, need * 3)?; // 압축 파일 + 압축 해제 여유
        }
        let mut file = tokio::fs::OpenOptions::new().create(true).write(true).append(resumed).truncate(!resumed).open(&part).await?;
        let mut stream = resp.bytes_stream();
        use futures_util::StreamExt;
        loop {
            let chunk = tokio::select! {
                _ = cancel.cancelled() => { file.flush().await.ok(); return Err(AppError::Cancelled); }
                c = stream.next() => c,
            };
            match chunk {
                Some(Ok(b)) => {
                    file.write_all(&b).await?;
                    completed += b.len() as u64;
                    on_progress(InstallProgress { stage: "AI 실행 엔진을 내려받는 중".into(), total_bytes: total, completed_bytes: completed, done: false });
                }
                Some(Err(_)) => {
                    file.flush().await.ok();
                    return Err(AppError::Network("내려받는 중 연결이 끊겼습니다. 다시 시도하면 이어서 받습니다.".into()));
                }
                None => break,
            }
        }
        file.flush().await?;
        drop(file);

        on_progress(InstallProgress { stage: "내려받은 파일이 올바른지 확인하는 중".into(), total_bytes: total, completed_bytes: completed, done: false });
        let p = part.clone();
        let actual = tokio::task::spawn_blocking(move || sha256_file(&p)).await.map_err(|_| AppError::Other("확인 작업 실패".into()))??;
        if !actual.eq_ignore_ascii_case(expected.trim()) {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(AppError::Blocked("내려받은 엔진 파일이 공식 배포 파일과 일치하지 않아 설치를 중단했습니다".into()));
        }

        on_progress(InstallProgress { stage: "설치하는 중".into(), total_bytes: total, completed_bytes: completed, done: false });
        let dest = self.install_dir();
        let tmp = root.join(format!("{}.extracting", self.manifest.version));
        let (p, t, name) = (part.clone(), tmp.clone(), asset.file.clone());
        tokio::task::spawn_blocking(move || {
            let _ = std::fs::remove_dir_all(&t);
            std::fs::create_dir_all(&t)?;
            extract_archive(&p, &name, &t)
        })
        .await
        .map_err(|_| AppError::Other("설치 작업 실패".into()))??;
        let _ = tokio::fs::remove_dir_all(&dest).await;
        tokio::fs::rename(&tmp, &dest).await?;
        tokio::fs::write(dest.join(".installed"), &actual).await?;
        let _ = tokio::fs::remove_file(&part).await;
        // 이전 버전 폴더 정리
        if let Ok(mut rd) = tokio::fs::read_dir(&root).await {
            while let Ok(Some(e)) = rd.next_entry().await {
                if e.path().is_dir() && e.file_name().to_string_lossy() != self.manifest.version {
                    let _ = tokio::fs::remove_dir_all(e.path()).await;
                }
            }
        }
        on_progress(InstallProgress { stage: "설치 완료".into(), total_bytes: total, completed_bytes: completed, done: true });
        Ok(())
    }

    /// 이전 실행에서 남은 엔진 프로세스를 정리한다. 우리 엔진 실행 파일인 경우에만 종료한다.
    fn cleanup_stale(&self) {
        let pid_file = self.pid_file();
        let Ok(s) = std::fs::read_to_string(&pid_file) else { return };
        let _ = std::fs::remove_file(&pid_file);
        let Ok(pid) = s.trim().parse::<u32>() else { return };
        let Ok(our_bin) = self.binary_path() else { return };
        use sysinfo::{Pid, ProcessesToUpdate, System};
        let mut sys = System::new();
        let pid = Pid::from_u32(pid);
        sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        if let Some(p) = sys.process(pid) {
            let same = p.exe().map(|e| same_file(e, &our_bin)).unwrap_or(false);
            if same {
                p.kill();
            }
        }
    }

    /// 엔진을 시작하고 포트를 돌려준다. 이미 실행 중이면 그 포트를 돌려준다.
    pub async fn start(&self) -> AppResult<u16> {
        let mut guard = self.child.lock().await;
        if let Some((child, port)) = guard.as_mut() {
            if matches!(child.try_wait(), Ok(None)) {
                return Ok(*port);
            }
            *guard = None;
        }
        if !self.is_installed() {
            return Err(AppError::Runtime("AI 실행 엔진이 아직 설치되지 않았습니다".into()));
        }
        self.cleanup_stale();
        let bin = self.binary_path()?;
        let models = self.models_dir();
        tokio::fs::create_dir_all(&models).await?;
        tokio::fs::create_dir_all(self.engine_root()).await?;
        let port = free_loopback_port()?;

        let mut cmd = tokio::process::Command::new(&bin);
        cmd.arg("serve");
        // 사용자 환경의 OLLAMA_* 설정(외부 바인딩, 요청 본문 로깅 등)을 물려받지 않는다.
        for (k, _) in std::env::vars_os() {
            if k.to_string_lossy().to_ascii_uppercase().starts_with("OLLAMA_") {
                cmd.env_remove(&k);
            }
        }
        for (k, v) in &self.dev_env {
            cmd.env(k, v);
        }
        // 보안 관련 값은 항상 마지막에 설정해 어떤 경우에도 덮어쓰이지 않게 한다.
        cmd.env("OLLAMA_HOST", format!("127.0.0.1:{port}"))
            .env("OLLAMA_NO_CLOUD", "1")
            .env("OLLAMA_MODELS", &models)
            .env("OLLAMA_NOHISTORY", "1")
            .stdin(std::process::Stdio::null())
            // 엔진 로그는 저장하지 않는다 (요청 내용이 섞일 가능성 차단).
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        {
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        #[cfg(target_os = "linux")]
        unsafe {
            // 앱 프로세스가 어떤 이유로든 끝나면 엔진도 함께 종료되게 한다.
            cmd.pre_exec(|| {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = cmd.spawn().map_err(|_| AppError::Runtime("AI 실행 엔진을 시작하지 못했습니다. 보안 프로그램이 실행을 막고 있는지 확인해 주세요.".into()))?;
        if let Some(pid) = child.id() {
            let _ = std::fs::write(self.pid_file(), pid.to_string());
        }
        #[cfg(windows)]
        if let Some(h) = child.raw_handle() {
            win_job::assign(h);
        }

        // 준비될 때까지 대기 (최대 40초)
        let url = Url::parse(&format!("http://127.0.0.1:{port}/api/version")).unwrap();
        let mut ready = false;
        for _ in 0..200 {
            if let Ok(Some(_)) = child.try_wait() {
                break; // 프로세스가 바로 종료됨
            }
            if let Ok(req) = self.guard.loopback_request(reqwest::Method::GET, &url) {
                if let Ok(r) = req.timeout(Duration::from_secs(2)).send().await {
                    if r.status().is_success() {
                        ready = true;
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        if !ready {
            let _ = child.kill().await;
            let _ = std::fs::remove_file(self.pid_file());
            return Err(AppError::Runtime("AI 실행 엔진이 시작되지 않았습니다. 컴퓨터를 다시 시작한 뒤 시도해 주세요.".into()));
        }
        *guard = Some((child, port));
        Ok(port)
    }

    /// 엔진 프로세스를 종료한다 (앱 종료 시 호출).
    pub async fn stop(&self) {
        if let Some((mut child, _)) = self.child.lock().await.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        let _ = std::fs::remove_file(self.pid_file());
    }

    /// 동기 문맥(앱 종료 훅)에서 호출하는 종료 함수.
    pub fn stop_blocking(&self) {
        if let Ok(mut g) = self.child.try_lock() {
            if let Some((mut child, _)) = g.take() {
                let _ = child.start_kill();
            }
        }
        // try_lock 실패 시에도 pid 파일 기준으로 정리
        self.cleanup_stale();
    }

    pub async fn port(&self) -> Option<u16> {
        self.child.lock().await.as_ref().map(|c| c.1)
    }

    pub async fn pid(&self) -> Option<u32> {
        self.child.lock().await.as_ref().and_then(|c| c.0.id())
    }
}

/// Windows: 앱 프로세스가 끝나면(강제 종료 포함) 엔진과 그 하위 프로세스를 함께 종료하는 Job Object.
#[cfg(windows)]
mod win_job {
    use std::sync::OnceLock;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    struct Job(isize);
    // 핸들 값만 보관하며, 앱이 끝날 때 운영체제가 닫는다 (그 순간 엔진이 종료된다).
    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    fn job() -> Option<isize> {
        JOB.get_or_init(|| unsafe {
            let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if h.is_null() {
                return None;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 {
                return None;
            }
            Some(Job(h as isize))
        })
        .as_ref()
        .map(|j| j.0)
    }

    pub fn assign(process: std::os::windows::io::RawHandle) {
        if let Some(j) = job() {
            unsafe {
                AssignProcessToJobObject(j as _, process as _);
            }
        }
    }
}

/// '로컬 전용' 상태를 **실제로 확인**한다. 설정값이 아니라 실행 중인 엔진을 검사한다.
pub async fn verify_local_only(runtime: &dyn super::AiRuntime, port: u16, model: Option<&str>) -> LocalOnlyReport {
    let mut r = LocalOnlyReport::default();
    // 1) 엔진이 보고하는 클라우드 차단 상태
    match runtime.health().await {
        Ok(h) if h.running => match h.cloud_disabled {
            Some(true) => {
                r.cloud_disabled = true;
                r.details.push("확인됨: AI 엔진의 클라우드 기능이 꺼져 있습니다.".into());
            }
            Some(false) => r.details.push("경고: AI 엔진의 클라우드 기능이 켜져 있습니다.".into()),
            None => r.details.push("확인 불가: AI 엔진이 클라우드 설정 상태를 알려 주지 않습니다.".into()),
        },
        _ => r.details.push("AI 엔진이 실행 중이 아닙니다.".into()),
    }
    // 2) 외부 주소로 접속 시도 → 거부되어야 함
    let ips = tokio::task::spawn_blocking(non_loopback_ips).await.unwrap_or_default();
    let mut exposed = false;
    for ip in &ips {
        let addr = SocketAddr::new(*ip, port);
        let ok = tokio::task::spawn_blocking(move || TcpStream::connect_timeout(&addr, Duration::from_millis(700)).is_ok())
            .await
            .unwrap_or(false);
        if ok {
            exposed = true;
        }
    }
    let loopback_ok = {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        tokio::task::spawn_blocking(move || TcpStream::connect_timeout(&addr, Duration::from_millis(700)).is_ok()).await.unwrap_or(false)
    };
    if loopback_ok && !exposed {
        r.loopback_only = true;
        r.details.push(if ips.is_empty() {
            "확인됨: AI 엔진은 이 컴퓨터 내부 주소에서만 열려 있습니다 (외부 네트워크 연결 없음).".into()
        } else {
            "확인됨: 이 컴퓨터의 네트워크 주소로는 AI 엔진에 접속할 수 없습니다 (내부 전용).".into()
        });
    } else if exposed {
        r.details.push("경고: AI 엔진이 외부 네트워크 주소에서 접속 가능한 상태입니다.".into());
    }
    // 3) 선택된 모델이 로컬 모델인지
    if let Some(m) = model {
        match runtime.list_models().await {
            Ok(list) => match list.iter().find(|x| x.name == m) {
                Some(x) if !x.is_remote => {
                    r.model_local = true;
                    r.details.push("확인됨: 선택한 AI 모델은 이 컴퓨터에 저장된 모델입니다.".into());
                }
                Some(_) => r.details.push("경고: 선택한 AI 모델은 원격에서 실행되는 모델입니다.".into()),
                None => r.details.push("선택한 AI 모델이 설치되어 있지 않습니다.".into()),
            },
            Err(_) => r.details.push("설치된 모델 목록을 확인하지 못했습니다.".into()),
        }
    } else {
        r.details.push("선택된 AI 모델이 없습니다.".into());
    }
    r.verified = r.cloud_disabled && r.loopback_only && r.model_local;
    r
}

/// 이 컴퓨터의 루프백이 아닌 IP 주소들. UDP `connect`는 패킷을 보내지 않고
/// 라우팅 테이블만 조회하므로 외부 통신이 발생하지 않는다.
pub fn non_loopback_ips() -> Vec<IpAddr> {
    let mut out = Vec::new();
    for (bind, target) in [("0.0.0.0:0", "192.0.2.1:9"), ("[::]:0", "[2001:db8::1]:9")] {
        if let Ok(s) = UdpSocket::bind(bind) {
            if s.connect(target).is_ok() {
                if let Ok(a) = s.local_addr() {
                    if !a.ip().is_loopback() && !a.ip().is_unspecified() && !out.contains(&a.ip()) {
                        out.push(a.ip());
                    }
                }
            }
        }
    }
    out
}

fn free_loopback_port() -> AppResult<u16> {
    let l = std::net::TcpListener::bind("127.0.0.1:0").map_err(|_| AppError::Runtime("사용 가능한 로컬 포트를 찾지 못했습니다".into()))?;
    Ok(l.local_addr().map_err(|_| AppError::Runtime("로컬 포트 확인 실패".into()))?.port())
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

pub fn sha256_file(path: &Path) -> AppResult<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(hex::encode(h.finalize()))
}

pub fn check_free_space(dir: &Path, need_bytes: u64) -> AppResult<()> {
    if let Some((free, _)) = crate::hardware::disk_space_for(dir) {
        if free < need_bytes {
            return Err(AppError::DiskSpace(format!(
                "약 {:.1}GB가 필요하지만 {:.1}GB만 남아 있습니다",
                need_bytes as f64 / 1e9,
                free as f64 / 1e9
            )));
        }
    }
    Ok(())
}

/// 압축을 안전하게 푼다 (상위 경로 탈출 차단).
pub fn extract_archive(archive: &Path, file_name: &str, dest: &Path) -> AppResult<()> {
    let bad = || AppError::File("엔진 압축 파일이 올바르지 않습니다".into());
    let f = std::fs::File::open(archive)?;
    if file_name.ends_with(".zip") {
        let mut z = zip::ZipArchive::new(f).map_err(|_| bad())?;
        for i in 0..z.len() {
            let mut entry = z.by_index(i).map_err(|_| bad())?;
            let Some(rel) = entry.enclosed_name() else { return Err(bad()) };
            let out = dest.join(rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&out)?;
            } else {
                if let Some(p) = out.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let mut w = std::fs::File::create(&out)?;
                std::io::copy(&mut entry, &mut w)?;
                #[cfg(unix)]
                if let Some(mode) = entry.unix_mode() {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode))?;
                }
            }
        }
        Ok(())
    } else {
        let reader: Box<dyn std::io::Read> = if file_name.ends_with(".tgz") || file_name.ends_with(".tar.gz") {
            Box::new(flate2::read::GzDecoder::new(f))
        } else if file_name.ends_with(".tar.zst") {
            Box::new(zstd::stream::read::Decoder::new(f).map_err(|_| bad())?)
        } else {
            return Err(AppError::Unsupported("알 수 없는 압축 형식".into()));
        };
        let mut ar = tar::Archive::new(reader);
        ar.set_preserve_permissions(true);
        for entry in ar.entries().map_err(|_| bad())? {
            let mut entry = entry.map_err(|_| bad())?;
            // unpack_in은 dest 밖으로 나가는 항목을 거부한다.
            if !entry.unpack_in(dest).map_err(|_| bad())? {
                return Err(bad());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn manifest_is_pinned_to_official_release() {
        let m = EngineManifest::builtin();
        assert!(m.base_url.starts_with("https://github.com/ollama/ollama/releases/download/v"));
        assert!(m.base_url.contains(&m.version));
        for k in ["windows-x86_64", "macos-universal"] {
            assert!(m.assets.contains_key(k));
        }
        for a in m.assets.values() {
            if let Some(h) = &a.sha256 {
                assert_eq!(h.len(), 64, "sha256 형식 오류");
            }
        }
    }

    #[test]
    fn zip_path_traversal_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let zpath = dir.path().join("evil.zip");
        {
            let mut zw = zip::ZipWriter::new(std::fs::File::create(&zpath).unwrap());
            let opt = zip::write::SimpleFileOptions::default();
            zw.start_file("../escaped.txt", opt).unwrap();
            zw.write_all(b"x").unwrap();
            zw.finish().unwrap();
        }
        let dest = dir.path().join("out");
        std::fs::create_dir_all(&dest).unwrap();
        assert!(extract_archive(&zpath, "evil.zip", &dest).is_err());
        assert!(!dir.path().join("escaped.txt").exists());
    }

    #[test]
    fn zip_and_tgz_extract() {
        let dir = tempfile::tempdir().unwrap();
        let zpath = dir.path().join("ok.zip");
        {
            let mut zw = zip::ZipWriter::new(std::fs::File::create(&zpath).unwrap());
            zw.start_file("lib/ollama/a.txt", zip::write::SimpleFileOptions::default()).unwrap();
            zw.write_all(b"hello").unwrap();
            zw.finish().unwrap();
        }
        let d1 = dir.path().join("o1");
        std::fs::create_dir_all(&d1).unwrap();
        extract_archive(&zpath, "ok.zip", &d1).unwrap();
        assert_eq!(std::fs::read_to_string(d1.join("lib/ollama/a.txt")).unwrap(), "hello");

        let tpath = dir.path().join("ok.tgz");
        {
            let gz = flate2::write::GzEncoder::new(std::fs::File::create(&tpath).unwrap(), flate2::Compression::fast());
            let mut tb = tar::Builder::new(gz);
            let mut h = tar::Header::new_gnu();
            h.set_size(2);
            h.set_mode(0o755);
            h.set_cksum();
            tb.append_data(&mut h, "ollama", &b"hi"[..]).unwrap();
            tb.into_inner().unwrap().finish().unwrap();
        }
        let d2 = dir.path().join("o2");
        std::fs::create_dir_all(&d2).unwrap();
        extract_archive(&tpath, "ok.tgz", &d2).unwrap();
        assert_eq!(std::fs::read_to_string(d2.join("ollama")).unwrap(), "hi");
    }

    #[test]
    fn free_space_check_detects_shortage() {
        let d = tempfile::tempdir().unwrap();
        assert!(check_free_space(d.path(), 1).is_ok());
        assert!(matches!(check_free_space(d.path(), u64::MAX / 2), Err(AppError::DiskSpace(_))));
    }

    #[test]
    fn sha256_known_value() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(sha256_file(&p).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[tokio::test]
    async fn install_fails_closed_without_pinned_hash() {
        let dir = tempfile::tempdir().unwrap();
        let g = Arc::new(NetGuard::new());
        g.set_download_consent(true);
        let mut em = EngineManager::new(dir.path().to_path_buf(), g);
        for a in em.manifest.assets.values_mut() {
            a.sha256 = None;
        }
        let r = em.install(|_| {}, CancellationToken::new()).await;
        assert!(matches!(r, Err(AppError::Blocked(_)) | Err(AppError::Unsupported(_))));
    }
}

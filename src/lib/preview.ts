// 개발 전용: 브라우저에서 화면 디자인만 확인하기 위한 예시 데이터.
// `import.meta.env.DEV`일 때만 불러오므로 배포용 빌드에는 포함되지 않는다.
// 여기의 응답은 실제 AI·실제 진단 결과가 아니다 (화면 상단에 '미리보기 모드'로 표시된다).
import { Backend, setBackend } from "./api";

const GBn = 1024 ** 3;
const cat = (id: string, name: string, pub_: string, dl: number, ram: number, lic: string, k: number, s: number, r: number, desc: string) => ({
  id, family: name.split(" ·")[0], publisher: pub_, display_name: name, params_b: dl * 1.3, quantization: "Q4_K_M", download_gb: dl, est_ram_gb: ram, context_k: 40,
  license: lic, license_url: "", korean: k, summarization: s, reasoning: r, thinking: false, description: desc,
});
const M = {
  q4: cat("qwen3:4b", "Qwen 3 · 가벼움", "Alibaba Cloud", 2.6, 4, "Apache-2.0", 3, 3, 3, "작지만 논리적인 정리에 강한 모델입니다."),
  q8: cat("qwen3:8b", "Qwen 3 · 균형", "Alibaba Cloud", 5.2, 7.5, "Apache-2.0", 3, 4, 4, "속도와 품질의 균형이 좋은 모델입니다. 문서 요약, 보고서 초안, 자료 정리에 두루 쓸 수 있습니다."),
  g12: cat("gemma3:12b", "Gemma 3 · 균형", "Google", 8.1, 10, "Gemma Terms of Use", 4, 4, 3, "한국어 문장이 자연스럽고 긴 문서 요약에 강한 모델입니다."),
};
const assess = (m: any, fit: string, speed: string) => ({ model: m, fit, speed, quality_score: 3.5, enough_disk: true });
const rec = (tier: string, label: string, a: any) => ({ tier, tier_label: label, assessment: a, reasons: ["교수님 컴퓨터에서 속도와 품질의 균형이 가장 좋을 것으로 예상되는 모델입니다.", `실행에 약 ${Math.ceil(a.model.est_ram_gb)}GB의 메모리가 필요하며, 교수님 컴퓨터의 메모리(16GB)로 여유 있게 실행할 수 있습니다.`, "응답 속도: 빠름 (예상)"], cautions: ["속도와 품질은 예상치이며, 실제 사용 환경에 따라 달라질 수 있습니다."] });

export function installPreviewBackend() {
  const listeners: Record<string, ((p: any) => void)[]> = {};
  const emit = (e: string, p: any) => (listeners[e] ?? []).forEach((cb) => cb(p));
  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
  const q = new URLSearchParams(location.search);
  const state = {
    onboarding_done: q.has("app"), installed: q.has("app") || q.has("reinstall") ? ["qwen3:8b"] : ([] as string[]), selected: q.has("app") ? "qwen3:8b" : (null as string | null),
    save_history: false, consent: false, cancelled: new Set<string>(),
    locked: q.has("locked"), has_passcode: q.has("locked") || q.has("pass"), auto_lock: 10, pseudo: true, update_consent: false, include_pre: true,
    notes: [{ id: "n1", student: "학생 A", date: "2026-11-03", content: "진로 고민. 대학원 진학과 취업 사이에서 결정을 어려워함." }, { id: "n2", student: "학생 B", date: "2026-10-28", content: "아르바이트 병행으로 과제 제출이 늦어짐." }],
  };
  const stream = async (jobId: string, text: string) => {
    for (const piece of text.match(/[\s\S]{1,6}/g) ?? []) {
      if (state.cancelled.has(jobId)) return { content: text.slice(0, 40), cancelled: true, eval_tokens: null, tokens_per_sec: null };
      emit("ai-token", { job_id: jobId, token: piece });
      await sleep(12);
    }
    return { content: text, cancelled: false, eval_tokens: 100, tokens_per_sec: 30 };
  };
  const SAMPLE = "## 예시 응답\n\n이것은 **화면 미리보기용 예시 문장**입니다. 실제 AI의 답변이 아닙니다.\n\n- 첫째 항목\n- 둘째 항목\n\n| 항목 | 값 |\n|---|---|\n| 예시 | 1 |\n";
  const profile = { row_count: 5, column_count: 3, total_missing: 1, columns: [
    { name: "학과", kind: "text", count: 5, missing: 0, unique: 2, numeric: null, top_values: [["교육학과", 3]], non_numeric: 0 },
    { name: "중간", kind: "numeric", count: 5, missing: 0, unique: 5, numeric: { mean: 82, median: 80, std_dev: 8.4, min: 70, max: 95, sum: 410 }, top_values: [], non_numeric: 0 },
    { name: "기말", kind: "numeric", count: 4, missing: 1, unique: 4, numeric: { mean: 82.5, median: 85, std_dev: 17.08, min: 60, max: 100, sum: 330 }, top_values: [], non_numeric: 0 } ] };

  const handlers: Record<string, (a: any) => any> = {
    app_status: () => ({ app_version: "0.2.0", engine: { installed: state.installed.length > 0 || q.has("reinstall"), version: "0.40.16", running: state.installed.length > 0, port: 41000, installable: true }, engine_name: "Ollama 0.40.16", selected_model: state.selected, onboarding_done: state.onboarding_done, download_consent: state.consent, save_history: state.save_history, purpose: "general",
      locked: state.locked, has_passcode: state.has_passcode, auto_lock_minutes: state.auto_lock, pseudonymize_students: state.pseudo, update_consent: state.update_consent, include_prerelease: state.include_pre }),
    unlock_app: async (a) => { await sleep(300); if (a.passcode !== "연구실커피는3시") throw { code: "wrong_passcode", message: "잠금 암호가 맞지 않습니다" }; state.locked = false; },
    lock_app: () => { state.locked = true; emit("app-locked", null); },
    user_activity: () => {},
    set_passcode: () => { state.has_passcode = true; },
    remove_passcode: () => { state.has_passcode = false; },
    set_auto_lock: (a) => { state.auto_lock = a.minutes; },
    set_pseudonymize: (a) => { state.pseudo = a.enabled; },
    factory_reset: () => { state.onboarding_done = false; state.locked = false; state.has_passcode = false; },
    security_checkup: () => ({ disk_encryption: "unknown", key_store: "os_keychain", has_passcode: state.has_passcode, auto_lock_minutes: state.auto_lock, save_history: state.save_history, pseudonymize_students: state.pseudo, download_consent: state.consent, update_consent: state.update_consent }),
    pick_document: () => ({ id: "d" + Math.random(), file_name: "강의자료.pdf", kind: "pdf", char_count: 18234, units: 24, warnings: [], preview: "" }),
    pick_table: () => handlers.load_table({}),
    reload_table_sheet: () => handlers.load_table({}),
    save_docx: () => "결과.docx", save_text: () => "결과.md", save_analysis_xlsx: () => "분석결과.xlsx",
    run_student_task: async (a) => ({ outcome: await stream(a.jobId, "## 상담 내용 요약\n\n- 학생1은 진로 고민을 이야기함 (예시)\n"), aliases: a.pseudonymize ? [{ alias: "학생1", original: "학생 A" }] : [] }),
    set_update_consent: (a) => { state.update_consent = a.consent; },
    set_include_prerelease: (a) => { state.include_pre = a.enabled; },
    check_update: async () => { await sleep(400); return { current_version: "0.2.0", latest_version: "0.3.0", update_available: true, is_prerelease: true, title: "필로소피 로컬 AI v0.3.0 (예시)", notes: "## 새 기능 (예시)\n- 미리보기용 예시 릴리스 노트입니다.", published_at: "2026-12-01T00:00:00Z", release_url: "", asset: { name: "Philosophy.Local.AI_0.3.0_x64-setup.exe", size: 7000000, url: "", sha256: "0".repeat(64) }, asset_note: null }; },
    download_update: async (a) => { for (let i = 1; i <= 10; i++) { emit("update-progress", { job_id: a.jobId, progress: { stage: i < 10 ? "새 버전을 내려받는 중" : "내려받은 파일이 올바른지 확인하는 중", total_bytes: 7e6, completed_bytes: i * 7e5 } }); await sleep(60); } return "Philosophy.Local.AI_0.3.0_x64-setup.exe"; },
    open_installer: () => {}, reveal_installer: () => {}, open_release_page: () => {},
    detect_hardware: async () => { await sleep(300); return { profile: { os_name: "예시 OS", os_version: "1.0", arch: "aarch64", cpu_vendor: "Apple", cpu_model: "예시 프로세서", cpu_physical_cores: 8, cpu_logical_cores: 8, total_ram: 16 * GBn, available_ram: 9 * GBn, gpus: [{ vendor: "Apple", name: "예시 그래픽", vram: { kind: "shared" } }], acceleration: "apple_metal", is_apple_silicon: true, disk_free: 212 * GBn, disk_total: 494 * GBn, notes: [] }, summary: ["교수님의 컴퓨터는 문서 분석과 AI 채팅을 여유 있게 실행할 수 있습니다.", "Apple 칩의 그래픽 가속을 사용해 AI가 빠르게 응답합니다."] }; },
    recommend_models: () => ({ recommendations: [rec("light", "빠르고 가벼운 AI", assess(M.q4, "comfortable", "fast")), rec("balanced", "균형 잡힌 추천 AI", assess(M.q8, "comfortable", "fast")), rec("high_quality", "고성능 AI", assess(M.g12, "tight", "fast"))], all: [assess(M.q4, "comfortable", "fast"), assess(M.q8, "comfortable", "fast"), assess(M.g12, "tight", "fast")], memory_budget_gb: 10.4, explanation: "Apple 칩의 통합 메모리 가운데 약 10GB를 AI가 사용할 수 있다고 보고 추천했습니다." }),
    complete_onboarding: () => { state.onboarding_done = true; },
    set_download_consent: (a) => { state.consent = a.consent; },
    install_engine: async (a) => { for (let i = 1; i <= 10; i++) { emit("engine-install-progress", { job_id: a.jobId, progress: { stage: "AI 실행 엔진을 내려받는 중", total_bytes: 1e8, completed_bytes: i * 1e7, done: false } }); await sleep(80); } },
    start_engine: () => 41000,
    pull_model: async (a) => { for (let i = 1; i <= 25; i++) { if (state.cancelled.has(a.jobId)) throw { code: "cancelled", message: "작업이 취소되었습니다" }; emit("pull-progress", { job_id: a.jobId, progress: { stage: i > 23 ? "내려받은 파일이 올바른지 확인하는 중" : "AI 모델을 내려받는 중", total_bytes: 5.2e9, completed_bytes: Math.min(i, 23) / 23 * 5.2e9, verifying: i > 23, done: false } }); await sleep(Number(q.get("slow") ?? 60)); } state.installed.push(a.name); },
    cancel_job: (a) => { state.cancelled.add(a.jobId); },
    test_model: async () => { await sleep(400); return { reply: "안녕하세요, 교수님. (미리보기 예시)", tokens_per_sec: 30 }; },
    select_model: (a) => { state.selected = a.name; },
    list_models: () => state.installed.map((n) => ({ name: n, size_bytes: 5.2e9, modified_at: "", parameter_size: "8.2B", quantization: "Q4_K_M", is_remote: false })),
    delete_model: (a) => { state.installed = state.installed.filter((n) => n !== a.name); if (state.selected === a.name) state.selected = null; },
    verify_local: () => ({ cloud_disabled: true, loopback_only: true, model_local: !!state.selected, verified: !!state.selected, details: ["확인됨: AI 엔진의 클라우드 기능이 꺼져 있습니다.", "확인됨: 이 컴퓨터의 네트워크 주소로는 AI 엔진에 접속할 수 없습니다 (내부 전용).", state.selected ? "확인됨: 선택한 AI 모델은 이 컴퓨터에 저장된 모델입니다." : "선택된 AI 모델이 없습니다."] }),
    chat_send: async (a) => ({ outcome: await stream(a.jobId, SAMPLE), conversation_id: null }),
    list_conversations: () => [], get_conversation: () => [], delete_conversation: () => {},
    load_document: (a) => ({ id: "d" + Math.random(), file_name: String(a.path).split("/").pop(), kind: "pdf", char_count: 18234, units: 24, warnings: [], preview: "" }),
    remove_document: () => {}, clear_loaded_files: () => {},
    list_tasks: () => [["lecture_summary", "lecture", "강의자료 요약", "자료의 전체 흐름을 한눈에 볼 수 있게 정리합니다."], ["lecture_concepts", "lecture", "핵심 개념 추출", "자료에 나오는 중요한 개념과 설명을 뽑아 냅니다."], ["lecture_quiz", "lecture", "퀴즈 초안 만들기", "자료 내용으로 풀 수 있는 퀴즈 초안을 만듭니다."], ["student_summary", "student", "상담 내용 요약", ""], ["student_plan", "student", "지도 계획 초안", ""], ["research_summary", "research", "연구자료 요약", "연구 목적·방법·결과를 중심으로 요약합니다."], ["research_compare", "research", "논문 비교", "여러 자료의 공통점과 차이점을 비교합니다."], ["admin_minutes", "admin", "회의록 요약", "회의 내용을 결정 사항 중심으로 정리합니다."], ["admin_letter", "admin", "공문 초안", "전달할 내용을 공문 형식의 초안으로 만듭니다."]].map(([id, category, label, description]) => ({ id, category, label, description, extra_hint: id === "admin_letter" ? "수신·제목·전달할 내용" : "" })),
    run_task: async (a) => { emit("ai-stage", { job_id: a.jobId, stage: "결과를 작성하는 중" }); return stream(a.jobId, SAMPLE); },
    doc_qa: async (a) => ({ outcome: await stream(a.jobId, "예시 답변입니다 [근거 2]."), evidence: [{ index: 2, text: "예시 근거 문단입니다." }] }),
    list_student_notes: (a) => state.notes.filter((n) => !a.query || n.student.includes(a.query) || n.content.includes(a.query)),
    add_student_note: (a) => { state.notes.unshift({ id: "n" + Math.random(), ...a }); },
    delete_student_note: (a) => { state.notes = state.notes.filter((n) => n.id !== a.id); },
    load_table: () => ({ id: "t1", file_name: "예시_성적.xlsx", sheet: "Sheet1", sheets: ["Sheet1"], headers: ["학과", "중간", "기말"], preview_rows: [["철학과", "80", "90"], ["교육학과", "95", "100"]], truncated: false, profile }),
    group_table: (a) => ({ group_column: a.spec.group_column, value_column: a.spec.value_column, agg: a.spec.agg, rows: [{ group: "교육학과", count: 3, value: 86.67 }, { group: "철학과", count: 2, value: 75 }], skipped: 0 }),
    data_report: async (a) => ({ stats_markdown: "| 항목 | 평균 |\n|---|---|\n| 기말 | 82.5 |\n", outcome: await stream(a.jobId, SAMPLE) }),
    export_docx: () => {}, export_text: () => {}, export_analysis_xlsx: () => {},
    privacy_info: () => ({ data_dir: "/예시/앱자료", database_path: "/예시/앱자료/app.db", models_dir: "/예시/앱자료/models", key_store: "os_keychain", save_history: state.save_history, download_consent: state.consent, stats: { conversations: 0, messages: 0, student_notes: state.notes.length, db_bytes: 40960 }, audit_log: state.installed.length ? [{ at: "2026-11-10 10:00:00", purpose: "model_download", host: "registry.ollama.ai", detail: "모델 qwen3:8b 내려받기", allowed: true }] : [], loaded_documents: 0, disk_free: 212 * GBn, models_bytes: state.installed.length * 5.2e9 }),
    set_save_history: (a) => { state.save_history = a.enabled; },
    delete_all_conversations: () => {}, delete_all_data: () => { state.notes = []; },
  };

  const backend: Backend = {
    call: async (cmd, args) => { const h = handlers[cmd]; if (!h) throw { code: "other", message: `미리보기에서 지원하지 않는 기능: ${cmd}` }; return h(args ?? {}); },
    on: async (event, cb) => { (listeners[event] ??= []).push(cb); return () => { listeners[event] = listeners[event].filter((x) => x !== cb); }; },
  };
  setBackend(backend);
}

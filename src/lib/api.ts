// 화면 ↔ 앱(Rust) 연결 계층.
// 웹뷰는 네트워크에 직접 접근하지 않는다. 모든 기능은 Tauri 명령(IPC)으로만 호출한다.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// ---- 타입 ----
export type Vram = { kind: "exact" | "at_least"; bytes: number } | { kind: "shared" | "unknown" };
export interface GpuInfo { vendor: string; name: string; vram: Vram }
export interface HardwareProfile {
  os_name: string; os_version: string; arch: string; cpu_vendor: string; cpu_model: string;
  cpu_physical_cores: number; cpu_logical_cores: number; total_ram: number; available_ram: number;
  gpus: GpuInfo[]; acceleration: "apple_metal" | "nvidia_cuda" | "amd_gpu" | "cpu_only";
  is_apple_silicon: boolean; disk_free: number; disk_total: number; notes: string[];
}
export interface HardwareReport { profile: HardwareProfile; summary: string[] }
export interface CatalogModel {
  id: string; family: string; publisher: string; display_name: string; params_b: number; quantization: string;
  download_gb: number; est_ram_gb: number; context_k: number; license: string; license_url: string;
  korean: number; summarization: number; reasoning: number; thinking: boolean; description: string;
}
export interface Catalog { catalog_version: string; note: string; models: CatalogModel[] }
export type Fit = "comfortable" | "tight" | "too_large";
export type Speed = "fast" | "moderate" | "slow";
export interface Assessment { model: CatalogModel; fit: Fit; speed: Speed; quality_score: number; enough_disk: boolean }
export interface Recommendation { tier: "light" | "balanced" | "high_quality"; tier_label: string; assessment: Assessment; reasons: string[]; cautions: string[] }
export interface RecommendationResult { recommendations: Recommendation[]; all: Assessment[]; memory_budget_gb: number; explanation: string }
export type Purpose = "general" | "documents" | "research" | "data";
export interface EngineStatus { installed: boolean; version: string; running: boolean; port: number | null; installable: boolean }
export interface AppStatus {
  app_version: string; engine: EngineStatus; engine_name: string; selected_model: string | null;
  onboarding_done: boolean; download_consent: boolean; save_history: boolean; purpose: Purpose;
}
export interface InstalledModel { name: string; size_bytes: number; modified_at: string; parameter_size: string | null; quantization: string | null; is_remote: boolean }
export interface Progress { stage: string; total_bytes: number; completed_bytes: number; verifying?: boolean; done: boolean }
export interface LocalOnlyReport { cloud_disabled: boolean; loopback_only: boolean; model_local: boolean; verified: boolean; details: string[] }
export interface ChatMessage { role: "user" | "assistant"; content: string }
export interface ChatOutcome { content: string; cancelled: boolean; eval_tokens: number | null; tokens_per_sec: number | null }
export interface ConversationMeta { id: string; kind: string; title: string; updated_at: string }
export interface DocInfo { id: string; file_name: string; kind: string; char_count: number; units: number | null; warnings: string[]; preview: string }
export interface TaskDef { id: string; category: string; label: string; description: string; extra_hint: string }
export interface Chunk { index: number; text: string }
export interface StudentNote { id: string; student: string; date: string; content: string }
export interface NumericStats { mean: number; median: number; std_dev: number | null; min: number; max: number; sum: number }
export interface ColumnProfile { name: string; kind: "numeric" | "text" | "empty"; count: number; missing: number; unique: number; numeric: NumericStats | null; top_values: [string, number][]; non_numeric: number }
export interface TableProfile { row_count: number; column_count: number; columns: ColumnProfile[]; total_missing: number }
export interface TableInfo { id: string; file_name: string; sheet: string | null; sheets: string[]; headers: string[]; preview_rows: string[][]; truncated: boolean; profile: TableProfile }
export type Agg = "count" | "sum" | "mean" | "min" | "max";
export interface GroupSpec { group_column: string; value_column: string | null; agg: Agg }
export interface GroupResult { group_column: string; value_column: string | null; agg: Agg; rows: { group: string; count: number; value: number | null }[]; skipped: number }
export interface AuditEntry { at: string; purpose: string; host: string; detail: string; allowed: boolean }
export interface PrivacyInfo {
  data_dir: string; database_path: string; models_dir: string; key_store: "os_keychain" | "key_file" | "memory";
  save_history: boolean; download_consent: boolean; stats: { conversations: number; messages: number; student_notes: number; db_bytes: number };
  audit_log: AuditEntry[]; loaded_documents: number; disk_free: number; models_bytes: number;
}

// ---- 호출 ----
type Listener = (payload: any) => void;
interface Backend {
  call<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  on(event: string, cb: Listener): Promise<() => void>;
  pickFile(name: string, extensions: string[]): Promise<string | null>;
  pickSave(defaultName: string, name: string, extensions: string[]): Promise<string | null>;
}

const tauriBackend: Backend = {
  call: (cmd, args) => invoke(cmd, args),
  on: async (event, cb) => listen(event, (e) => cb(e.payload)),
  pickFile: async (name, extensions) => {
    const r = await open({ multiple: false, directory: false, filters: [{ name, extensions }] });
    return typeof r === "string" ? r : null;
  },
  pickSave: async (defaultName, name, extensions) => (await save({ defaultPath: defaultName, filters: [{ name, extensions }] })) ?? null,
};

let backend: Backend = tauriBackend;
/** 개발용 미리보기(브라우저)에서만 호출된다. 배포용 앱에서는 호출되지 않는다. */
export function setBackend(b: Backend) {
  backend = b;
}
export type { Backend };

export const call = <T,>(cmd: string, args?: Record<string, unknown>) => backend.call<T>(cmd, args);
export const on = (event: string, cb: Listener) => backend.on(event, cb);
export const pickFile = (name: string, extensions: string[]) => backend.pickFile(name, extensions);
export const pickSave = (defaultName: string, name: string, extensions: string[]) => backend.pickSave(defaultName, name, extensions);

export function errMsg(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  if (typeof e === "string") return e;
  return "알 수 없는 오류가 발생했습니다.";
}
export const errCode = (e: unknown): string => (e && typeof e === "object" && "code" in e ? String((e as { code: unknown }).code) : "");

export const newJobId = () => `job-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

// ---- 표시용 도우미 ----
export const GB = 1024 ** 3;
export const fmtGB = (bytes: number, digits = 0) => `${(bytes / GB).toFixed(digits)}GB`;
export function fmtBytes(b: number): string {
  if (b >= GB) return `${(b / GB).toFixed(1)}GB`;
  if (b >= 1024 ** 2) return `${(b / 1024 ** 2).toFixed(0)}MB`;
  return `${Math.max(0, Math.round(b / 1024))}KB`;
}
export const fitLabel: Record<Fit, string> = { comfortable: "여유 있게 실행", tight: "빠듯하게 실행", too_large: "이 컴퓨터에는 큼" };
export const speedLabel: Record<Speed, string> = { fast: "빠름", moderate: "보통", slow: "느림" };
export function fmtNum(n: number | null | undefined): string {
  if (n === null || n === undefined) return "-";
  return Number.isInteger(n) ? n.toLocaleString("ko-KR") : n.toLocaleString("ko-KR", { maximumFractionDigits: 2 });
}

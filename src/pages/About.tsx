// 이 앱에 대하여 — 어떤 기술을 쓰는지, 학생 정보를 다룰 때 무엇을 확인해야 하는지 쉬운 말로 안내한다.
import { useEffect, useState } from "react";
import { Catalog, call } from "../lib/api";
import { Icon, Notice, PageHeader } from "../lib/ui";
import { useApp } from "../App";

export default function About() {
  const { status, go } = useApp();
  const [catalog, setCatalog] = useState<Catalog | null>(null);
  useEffect(() => { call<Catalog>("get_catalog").then(setCatalog).catch(() => {}); }, []);
  const current = catalog?.models.find((m) => m.id === status.selected_model);
  const engineVersion = status.engine_name.replace(/^Ollama\s*/, "");

  const parts: { what: string; name: string; detail: string }[] = [
    { what: "AI 실행 엔진", name: `Ollama ${engineVersion}`, detail: "AI 모델을 이 컴퓨터에서 실행해 주는 공개 소프트웨어입니다(MIT 라이선스). 공식 배포처(GitHub)에서 받은 파일만 쓰며, 받을 때 공식 SHA-256 값과 일치하는지 확인합니다. 이 컴퓨터 내부 주소(127.0.0.1)에서만 열리고, Ollama의 클라우드 기능은 꺼 둡니다." },
    { what: "지금 쓰는 AI 모델", name: current ? `${current.display_name} (${current.id})` : status.selected_model ?? "아직 선택하지 않음", detail: current ? `${current.publisher}에서 공개한 모델입니다. 라이선스: ${current.license}. 모델 파일은 이 컴퓨터의 앱 전용 폴더에 저장되어, 인터넷 없이 실행됩니다.` : "‘내 AI 모델 관리’에서 고를 수 있습니다." },
    { what: "앱 본체·화면", name: "Tauri 2 (Rust) + React", detail: "Windows·Mac용 데스크톱 앱입니다. 화면은 인터넷에 직접 접속할 수 없도록 막혀 있고, 파일 열기·저장은 교수님이 고른 파일만 사용합니다." },
    { what: "기록 저장", name: "SQLite + XChaCha20-Poly1305 암호화", detail: "대화·분석·상담 기록은 내용 하나하나를 암호화해 이 앱 폴더에만 저장합니다. 암호화 열쇠는 운영체제 보안 저장소(Windows 자격 증명 관리자 / Mac 키체인)에 두고, 앱 잠금 암호(Argon2id)로 한 번 더 잠급니다." },
    { what: "인터넷 사용", name: "내려받을 때만", detail: "AI 실행 엔진·AI 모델을 받을 때(교수님 동의 후)와 ‘앱 업데이트’를 누를 때만 GitHub·Ollama 공식 저장소에 접속합니다. 문서·질문·대화·기록은 어디로도 전송하지 않습니다." },
  ];

  return (
    <div className="space-y-6">
      <PageHeader title="이 앱에 대하여" sub="필로소피 로컬 AI가 어떤 기술로 만들어졌고, 교수님의 자료를 어떻게 다루는지 알려 드립니다." />

      <section className="card">
        <h2 className="h2 mb-2">한눈에 보기</h2>
        <p className="text-[15px]">
          이 앱은 공개 AI 실행 엔진 <b>Ollama</b>를 교수님 컴퓨터 안에서 실행하고, 공개 AI 모델(<b>Qwen 3</b>, <b>Gemma 3</b>)로 대화와 문서 분석을 합니다.
          ChatGPT 같은 인터넷 AI 서비스를 쓰지 않으므로, 입력한 자료가 외부 회사의 서버로 가지 않습니다.
        </p>
      </section>

      <section className="card">
        <h2 className="h2 mb-3">사용하는 기술</h2>
        <dl className="divide-y divide-border">
          {parts.map((p) => (
            <div key={p.what} className="grid gap-1 py-3 sm:grid-cols-[150px_1fr] sm:gap-4">
              <dt className="text-[13.5px] font-semibold text-muted">{p.what}</dt>
              <dd><div className="font-bold">{p.name}</div><div className="text-[13.5px] text-muted">{p.detail}</div></dd>
            </div>
          ))}
        </dl>
      </section>

      <section className="card">
        <h2 className="h2 mb-1">이 앱에서 쓸 수 있는 AI 모델</h2>
        <p className="sub mb-3 text-[13.5px]">모두 무료로 공개된 모델이며, 각 모델의 라이선스(이용 조건)를 따릅니다.</p>
        <div className="overflow-x-auto rounded-xl border border-border">
          <table className="w-full text-[13.5px]">
            <thead><tr className="bg-surface-2 text-left"><th className="px-3 py-2">모델</th><th className="px-3 py-2">만든 곳</th><th className="px-3 py-2">라이선스</th><th className="px-3 py-2 text-right">크기(약)</th></tr></thead>
            <tbody>
              {(catalog?.models ?? []).map((m) => (
                <tr key={m.id} className="border-t border-border">
                  <td className="px-3 py-1.5"><b>{m.display_name}</b> <span className="text-muted">{m.id}</span>{m.id === status.selected_model && <span className="chip ml-2 bg-ok-soft text-ok">사용 중</span>}</td>
                  <td className="px-3 py-1.5">{m.publisher}</td>
                  <td className="px-3 py-1.5">{m.license}</td>
                  <td className="px-3 py-1.5 text-right tabular-nums">{m.download_gb.toFixed(1)}GB</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <p className="mt-2 text-[12.5px] text-muted">Gemma 모델은 Google의 ‘Gemma 이용 약관’(금지 용도 정책 포함)이 적용됩니다. Qwen 모델은 Apache-2.0 라이선스입니다.</p>
      </section>

      <section className="card">
        <h2 className="h2 mb-2">학생 정보를 넣어도 되나요?</h2>
        <p className="mb-3 text-[14.5px]">
          이 앱은 학생 정보를 <b>외부로 보내지 않고, 암호화해 이 컴퓨터에만</b> 두도록 만들었습니다. 다만 ‘앱이 안전하다’는 것만으로 개인정보 처리가 허용되는 것은 아닙니다. 아래를 모두 확인한 뒤 사용해 주세요.
        </p>
        <ol className="space-y-2 text-[14px]">
          {[
            ["소속 대학의 규정 확인", "학생 개인정보를 개인 컴퓨터의 프로그램으로 처리해도 되는지, 학교의 개인정보 보호 지침과 개인정보 보호 담당 부서에 먼저 확인해 주세요."],
            ["보안 점검을 모두 초록색으로", "‘개인정보 보호 설정 → 보안 점검’에서 앱 잠금 암호, 자동 잠금, 컴퓨터 디스크 암호화(BitLocker/FileVault)를 갖춰 주세요."],
            ["꼭 필요한 정보만", "주민등록번호, 건강·가정 형편 같은 민감정보는 넣지 마세요. 실명 대신 ‘학생 A’ 같은 별칭을 쓰고, ‘가명 처리’를 켜 두세요."],
            ["내보낸 파일 관리", "Word·Excel로 저장하거나 복사한 내용은 이 앱의 암호화 밖에 있습니다. 공유 폴더·메신저로 보내지 마세요."],
            ["다 쓰면 지우기", "필요 없어진 상담 기록과 분석 기록은 바로 지우고, 컴퓨터를 반납할 때는 ‘전체 초기화’를 하세요."],
          ].map(([t, d], i) => (
            <li key={t} className="flex gap-3"><span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-purple-soft text-[12.5px] font-bold text-purple">{i + 1}</span><span><b>{t}</b> — <span className="text-muted">{d}</span></span></li>
          ))}
        </ol>
        <div className="mt-4"><Notice tone="warn" title="지금은 시험판입니다">
          코드 서명이 없고, Windows·Mac 실제 컴퓨터와 실제 AI 모델로 하는 시험이 아직 진행 중입니다. 실제 학생 정보는 가상 자료로 충분히 써 본 뒤에 넣어 주세요.
        </Notice></div>
        <button className="btn-ghost btn-sm mt-3" onClick={() => go("privacy")}><Icon name="shield" className="h-4 w-4" /> 보안 점검하러 가기</button>
      </section>

      <section className="card">
        <h2 className="h2 mb-2">버전과 라이선스</h2>
        <dl className="grid grid-cols-[150px_1fr] gap-y-1.5 text-[14px]">
          <dt className="text-muted">앱 버전</dt><dd className="tabular-nums">v{status.app_version} (시험판)</dd>
          <dt className="text-muted">앱 라이선스</dt><dd>MIT (소스코드 공개)</dd>
          <dt className="text-muted">AI 실행 엔진</dt><dd>Ollama {engineVersion} · MIT</dd>
          <dt className="text-muted">만든 곳</dt><dd>Philosophy AI Education — AI로 성장하고, 그 성장을 나눈다</dd>
        </dl>
        <button className="btn-ghost btn-sm mt-3" onClick={() => go("update")}><Icon name="download" className="h-4 w-4" /> 새 버전 확인하기</button>
      </section>
    </div>
  );
}

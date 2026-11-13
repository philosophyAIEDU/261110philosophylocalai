import { useEffect, useMemo, useState } from "react";
import { Agg, ChatOutcome, GroupResult, GroupSpec, TableInfo, call, errMsg, fmtNum } from "../lib/api";
import { Empty, ErrorNotice, ExportButtons, Icon, Md, Notice, PageHeader, SecurityTip, useAiJob } from "../lib/ui";
import { NeedModel, useApp } from "../App";

const AGGS: { id: Agg; label: string }[] = [{ id: "count", label: "개수" }, { id: "mean", label: "평균" }, { id: "sum", label: "합계" }, { id: "min", label: "최솟값" }, { id: "max", label: "최댓값" }];

/** 가로 막대 그래프 — 값은 프로그램이 계산한 집계 결과 그대로 */
function BarChart({ result }: { result: GroupResult }) {
  const rows = result.rows.filter((r) => r.value !== null).slice(0, 12);
  const max = Math.max(...rows.map((r) => Math.abs(r.value as number)), 0);
  if (!rows.length || max === 0) return <p className="sub">그래프로 표시할 값이 없습니다.</p>;
  const label = result.value_column ? `${result.value_column} ${AGGS.find((a) => a.id === result.agg)?.label}` : "개수";
  return (
    <figure>
      <figcaption className="mb-2 text-[13px] font-semibold text-muted">{result.group_column}별 {label}{result.rows.length > rows.length ? ` (상위 ${rows.length}개)` : ""}</figcaption>
      <div className="space-y-1.5" role="img" aria-label={`${result.group_column}별 ${label} 막대 그래프`}>
        {rows.map((r) => (
          <div key={r.group} className="grid grid-cols-[minmax(80px,180px)_1fr_auto] items-center gap-3 text-[13.5px]">
            <span className="truncate text-right" title={r.group}>{r.group}</span>
            <div className="h-5 rounded bg-surface-2"><div className="h-full rounded bg-purple" style={{ width: `${Math.max(1.5, (Math.abs(r.value as number) / max) * 100)}%` }} /></div>
            <span className="w-[72px] text-right font-semibold tabular-nums">{fmtNum(r.value)}</span>
          </div>
        ))}
      </div>
    </figure>
  );
}

export default function DataPage() {
  const { status } = useApp();
  const [table, setTable] = useState<TableInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [groupCol, setGroupCol] = useState("");
  const [valueCol, setValueCol] = useState("");
  const [agg, setAgg] = useState<Agg>("count");
  const [group, setGroup] = useState<GroupResult | null>(null);
  const [focus, setFocus] = useState("");
  const [stats, setStats] = useState("");
  const [saveMsg, setSaveMsg] = useState<string | null>(null);
  const job = useAiJob();
  const ready = !!status.selected_model && status.engine.running;

  useEffect(() => () => { void call("clear_loaded_files"); }, []);

  const load = async (sheet: string | null) => {
    setLoading(true); setError(null);
    try {
      // 파일 선택 창은 앱이 직접 띄운다 (화면은 파일 경로를 다루지 않음)
      const t = sheet === null ? await call<TableInfo | null>("pick_table") : await call<TableInfo>("reload_table_sheet", { sheet });
      if (!t) { setLoading(false); return; }
      setGroup(null); job.reset(); setStats("");
      setTable(t);
      const firstText = t.profile.columns.find((c) => c.kind === "text" && c.unique > 1 && c.unique <= Math.max(30, t.profile.row_count / 2));
      setGroupCol(firstText?.name ?? t.headers[0] ?? ""); setValueCol(""); setAgg("count");
    } catch (e) { setError(errMsg(e)); }
    setLoading(false);
  };
  const pick = () => load(null);

  const spec: GroupSpec | null = useMemo(() => (table && groupCol ? { group_column: groupCol, value_column: agg === "count" ? null : valueCol || null, agg } : null), [table, groupCol, valueCol, agg]);
  const specValid = !!spec && (agg === "count" || !!valueCol);

  const runGroup = async () => {
    if (!table || !spec || !specValid) return;
    setError(null);
    try { setGroup(await call<GroupResult>("group_table", { tableId: table.id, spec })); } catch (e) { setError(errMsg(e)); }
  };
  const report = async () => {
    if (!table) return;
    const res = await job.run((jobId) => call<{ stats_markdown: string; outcome: ChatOutcome }>("data_report", { jobId, tableId: table.id, spec: group && specValid ? spec : null, focus }));
    if (res) setStats(res.stats_markdown);
  };
  const saveXlsx = async () => {
    if (!table) return;
    setSaveMsg(null);
    try {
      const name = await call<string | null>("save_analysis_xlsx", { tableId: table.id, spec: group && specValid ? spec : null });
      if (name) setSaveMsg(`'${name}'(으)로 저장했습니다. 저장한 파일은 암호화되지 않습니다.`);
    } catch (e) { setSaveMsg(errMsg(e)); }
  };

  const numericCols = table?.profile.columns.filter((c) => c.kind === "numeric") ?? [];
  const fullReport = stats && job.text ? `# 데이터 분석 보고서\n\n## 계산된 통계 (프로그램이 계산한 값)\n\n${stats}\n\n## AI 해석\n\n${job.text}` : "";

  return (
    <div className="space-y-6">
      <PageHeader title="Excel 데이터 분석" sub="Excel·CSV 파일의 통계를 프로그램이 정확히 계산하고, AI가 그 결과를 해석해 보고서로 정리합니다."
        right={<button className="btn-primary shrink-0" onClick={pick} disabled={loading || job.running}><Icon name="plus" className="h-4 w-4" /> {loading ? "읽는 중…" : "파일 불러오기"}</button>} />
      <ErrorNotice error={error} onDismiss={() => setError(null)} />
      <SecurityTip title="보안 유의사항">성적처럼 학생 개인정보가 담긴 표는 이름·학번 열을 지운 사본으로 분석하시길 권합니다. AI에게는 개별 학생의 행이 아니라 계산된 통계표만 전달되며, 불러온 표는 메뉴를 떠나거나 앱이 잠기면 메모리에서 지워집니다.</SecurityTip>

      {!table ? (
        <Empty icon="table" title="분석할 파일을 불러와 주세요">Excel(.xlsx, .xls)과 CSV 파일을 지원합니다. 첫 줄을 항목 이름으로 읽습니다. 파일은 이 컴퓨터 안에서만 처리됩니다.</Empty>
      ) : (
        <>
          <section className="card">
            <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
              <h2 className="h2">{table.file_name}</h2>
              {table.sheets.length > 1 && (
                <label className="flex items-center gap-2 text-[14px]"><span className="text-muted">시트</span>
                  <select className="input w-auto py-1.5" value={table.sheet ?? ""} onChange={(e) => load(e.target.value)}>{table.sheets.map((s) => <option key={s}>{s}</option>)}</select>
                </label>
              )}
            </div>
            <div className="mb-4 grid grid-cols-3 gap-3">
              {[["행(자료 수)", table.profile.row_count], ["열(항목 수)", table.profile.column_count], ["빈 값", table.profile.total_missing]].map(([k, v]) => (
                <div key={k} className="rounded-xl bg-surface-2 px-4 py-3"><div className="text-[12.5px] text-muted">{k}</div><div className="text-[22px] font-bold tabular-nums">{fmtNum(v as number)}</div></div>
              ))}
            </div>
            {table.truncated && <Notice tone="warn">파일이 매우 커서 앞부분 20만 행만 불러왔습니다.</Notice>}
            <details>
              <summary className="cursor-pointer text-[13.5px] font-semibold text-muted">데이터 미리보기 (처음 {table.preview_rows.length}행)</summary>
              <div className="mt-2 overflow-x-auto rounded-xl border border-border">
                <table className="w-full text-[13px]">
                  <thead><tr className="bg-surface-2">{table.headers.map((h) => <th key={h} className="whitespace-nowrap px-3 py-2 text-left font-bold">{h}</th>)}</tr></thead>
                  <tbody>{table.preview_rows.map((r, i) => <tr key={i} className="border-t border-border">{r.map((c, j) => <td key={j} className="whitespace-nowrap px-3 py-1.5">{c}</td>)}</tr>)}</tbody>
                </table>
              </div>
            </details>
          </section>

          <section className="card">
            <h2 className="h2 mb-1">기초 통계</h2>
            <p className="sub mb-3 text-[13.5px]">아래 숫자는 AI가 아니라 프로그램이 직접 계산한 값입니다.</p>
            <div className="overflow-x-auto rounded-xl border border-border">
              <table className="w-full text-[13.5px]">
                <thead><tr className="bg-surface-2 text-left">{["항목", "종류", "값 개수", "빈 값", "평균", "중앙값", "표준편차", "최솟값", "최댓값"].map((h, i) => <th key={h} className={`whitespace-nowrap px-3 py-2 font-bold ${i >= 2 ? "text-right" : ""}`}>{h}</th>)}</tr></thead>
                <tbody>
                  {table.profile.columns.map((c) => (
                    <tr key={c.name} className="border-t border-border">
                      <td className="px-3 py-1.5 font-semibold">{c.name}</td>
                      <td className="px-3 py-1.5 text-muted">{c.kind === "numeric" ? "숫자" : c.kind === "text" ? `문자 (${c.unique}종류)` : "비어 있음"}</td>
                      <td className="px-3 py-1.5 text-right tabular-nums">{fmtNum(c.count)}</td>
                      <td className={`px-3 py-1.5 text-right tabular-nums ${c.missing ? "font-semibold text-warn" : ""}`}>{fmtNum(c.missing)}</td>
                      {[c.numeric?.mean, c.numeric?.median, c.numeric?.std_dev, c.numeric?.min, c.numeric?.max].map((v, i) => <td key={i} className="px-3 py-1.5 text-right tabular-nums">{fmtNum(v)}</td>)}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>

          <section className="card">
            <h2 className="h2 mb-3">항목별 집계와 그래프</h2>
            <div className="flex flex-wrap items-end gap-3">
              <div><label className="label" htmlFor="g-col">기준 항목</label><select id="g-col" className="input w-[200px]" value={groupCol} onChange={(e) => setGroupCol(e.target.value)}>{table.headers.map((h) => <option key={h}>{h}</option>)}</select></div>
              <div><label className="label" htmlFor="g-agg">계산 방법</label><select id="g-agg" className="input w-[130px]" value={agg} onChange={(e) => setAgg(e.target.value as Agg)}>{AGGS.map((a) => <option key={a.id} value={a.id}>{a.label}</option>)}</select></div>
              {agg !== "count" && (
                <div><label className="label" htmlFor="g-val">계산할 숫자 항목</label><select id="g-val" className="input w-[200px]" value={valueCol} onChange={(e) => setValueCol(e.target.value)}><option value="">선택</option>{numericCols.map((c) => <option key={c.name}>{c.name}</option>)}</select></div>
              )}
              <button className="btn-primary" disabled={!specValid} onClick={runGroup}>집계하기</button>
            </div>
            {group && (
              <div className="mt-5 grid gap-6 lg:grid-cols-2">
                <BarChart result={group} />
                <div className="max-h-[340px] overflow-auto rounded-xl border border-border">
                  <table className="w-full text-[13.5px]">
                    <thead className="sticky top-0"><tr className="bg-surface-2 text-left"><th className="px-3 py-2">{group.group_column}</th><th className="px-3 py-2 text-right">행 수</th><th className="px-3 py-2 text-right">{group.value_column ? `${group.value_column} ${AGGS.find((a) => a.id === group.agg)?.label}` : "개수"}</th></tr></thead>
                    <tbody>{group.rows.map((r) => <tr key={r.group} className="border-t border-border"><td className="px-3 py-1.5">{r.group}</td><td className="px-3 py-1.5 text-right tabular-nums">{fmtNum(r.count)}</td><td className="px-3 py-1.5 text-right font-semibold tabular-nums">{fmtNum(r.value)}</td></tr>)}</tbody>
                  </table>
                </div>
                {group.skipped > 0 && <p className="text-[13px] text-warn lg:col-span-2">※ 숫자가 아닌 값 {group.skipped}건은 계산에서 제외했습니다.</p>}
              </div>
            )}
          </section>

          <section className="card">
            <h2 className="h2 mb-1">분석 결과 보고서</h2>
            <p className="sub mb-3 text-[13.5px]">AI에게는 위에서 계산한 통계표만 전달됩니다. 개별 행(학생 한 명 한 명의 자료)은 전달하지 않습니다.</p>
            <NeedModel />
            <label className="label" htmlFor="focus">특히 궁금한 점 (선택)</label>
            <input id="focus" className="input mb-3" value={focus} onChange={(e) => setFocus(e.target.value)} placeholder="예: 학과별 평균 차이를 중심으로 설명해 주세요" />
            <div className="flex flex-wrap items-center gap-2">
              {job.running ? <button className="btn-ghost" onClick={job.stop}><Icon name="stop" className="h-4 w-4" /> 중지</button> : <button className="btn-primary" disabled={!ready} onClick={report}><Icon name="spark" className="h-4 w-4" /> AI 해석 보고서 만들기</button>}
              <button className="btn-ghost" onClick={saveXlsx}><Icon name="download" className="h-4 w-4" /> 통계 결과를 Excel로 저장</button>
              {saveMsg && <span className="text-[13px] text-muted">{saveMsg}</span>}
            </div>
            <ErrorNotice error={job.error} onDismiss={() => job.setError(null)} />
            {(job.running || job.text) && (
              <div className="mt-4 rounded-xl border border-border p-4" aria-live="polite">
                {job.text ? <Md>{job.text}</Md> : <div className="flex items-center gap-3 text-muted"><span className="typing"><span /><span /><span /></span>AI가 통계표를 읽고 있습니다</div>}
              </div>
            )}
            {!job.running && fullReport && <div className="mt-4"><ExportButtons title="데이터 분석 보고서" markdown={fullReport} /><p className="mt-2 text-[12.5px] text-muted">저장되는 보고서에는 프로그램이 계산한 통계표와 AI 해석이 함께 들어갑니다.</p></div>}
          </section>
        </>
      )}
    </div>
  );
}

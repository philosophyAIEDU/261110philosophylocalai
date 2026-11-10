//! Excel/CSV 분석 — 모든 수치는 이 모듈의 코드로 계산한다.
//!
//! LLM은 여기서 계산된 통계표를 "해석"만 하며, 숫자를 직접 계산하지 않는다.
use crate::documents::decode_text;
use crate::error::{AppError, AppResult};
use calamine::{Data, Reader};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

const MAX_ROWS: usize = 200_000;

#[derive(Debug, Clone, Serialize)]
pub struct Table {
    pub file_name: String,
    pub sheet: Option<String>,
    pub sheets: Vec<String>,
    pub headers: Vec<String>,
    /// 문자열로 정규화한 셀 값. 빈 셀은 빈 문자열.
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    Numeric,
    Text,
    Empty,
}

#[derive(Debug, Clone, Serialize)]
pub struct NumericStats {
    pub mean: f64,
    pub median: f64,
    /// 표본 표준편차 (n-1)
    pub std_dev: Option<f64>,
    pub min: f64,
    pub max: f64,
    pub sum: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnProfile {
    pub name: String,
    pub kind: ColumnKind,
    pub count: usize,
    pub missing: usize,
    pub unique: usize,
    pub numeric: Option<NumericStats>,
    /// 가장 많이 나온 값 (최대 5개)
    pub top_values: Vec<(String, usize)>,
    /// 숫자 열인데 숫자로 읽을 수 없었던 값의 수
    pub non_numeric: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableProfile {
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<ColumnProfile>,
    pub total_missing: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Agg {
    Count,
    Sum,
    Mean,
    Min,
    Max,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupRow {
    pub group: String,
    pub count: usize,
    pub value: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupResult {
    pub group_column: String,
    pub value_column: Option<String>,
    pub agg: Agg,
    pub rows: Vec<GroupRow>,
    /// 값 열에서 숫자가 아니어서 계산에서 제외된 행 수
    pub skipped: usize,
}

/// CSV/TSV/XLSX/XLS 파일을 불러온다. 첫 행을 머리글로 본다.
pub fn load(path: &Path, sheet: Option<&str>) -> AppResult<Table> {
    let file_name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let (raw, sheets, sheet_name): (Vec<Vec<String>>, Vec<String>, Option<String>) = match ext.as_str() {
        "csv" | "tsv" | "txt" => {
            let text = decode_text(&std::fs::read(path).map_err(|_| AppError::File("파일을 열 수 없습니다".into()))?);
            let delim = if ext == "tsv" || (text.lines().next().unwrap_or("").matches('\t').count() > text.lines().next().unwrap_or("").matches(',').count()) {
                b'\t'
            } else if text.lines().next().unwrap_or("").matches(';').count() > text.lines().next().unwrap_or("").matches(',').count() {
                b';'
            } else {
                b','
            };
            let mut rdr = csv::ReaderBuilder::new().has_headers(false).flexible(true).delimiter(delim).from_reader(text.as_bytes());
            let mut rows = Vec::new();
            for rec in rdr.records() {
                let rec = rec.map_err(|_| AppError::File("CSV 형식이 올바르지 않습니다".into()))?;
                rows.push(rec.iter().map(|s| s.trim().to_string()).collect());
                if rows.len() > MAX_ROWS + 1 {
                    break;
                }
            }
            (rows, vec![], None)
        }
        "xlsx" | "xlsm" | "xls" | "ods" => {
            let mut wb = calamine::open_workbook_auto(path).map_err(|_| AppError::File("Excel 파일을 열 수 없습니다. 암호가 걸려 있거나 손상되었을 수 있습니다.".into()))?;
            let sheets = wb.sheet_names().to_vec();
            let name = sheet.map(|s| s.to_string()).or_else(|| sheets.first().cloned()).ok_or_else(|| AppError::File("시트가 없습니다".into()))?;
            let range = wb.worksheet_range(&name).map_err(|_| AppError::File("시트를 읽을 수 없습니다".into()))?;
            let rows = range.rows().take(MAX_ROWS + 2).map(|r| r.iter().map(cell_to_string).collect()).collect();
            (rows, sheets, Some(name))
        }
        _ => return Err(AppError::Unsupported(format!(".{ext} 파일 (CSV 또는 Excel 파일을 선택해 주세요)"))),
    };
    let mut it = raw.into_iter().filter(|r: &Vec<String>| r.iter().any(|c| !c.is_empty()));
    let header_row = it.next().ok_or_else(|| AppError::File("데이터가 비어 있습니다".into()))?;
    let mut rows: Vec<Vec<String>> = it.collect();
    let truncated = rows.len() > MAX_ROWS;
    rows.truncate(MAX_ROWS);
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(header_row.len());
    let mut seen: HashMap<String, usize> = HashMap::new();
    let headers: Vec<String> = (0..width)
        .map(|i| {
            let base = header_row.get(i).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| format!("열{}", i + 1));
            let n = seen.entry(base.clone()).or_default();
            *n += 1;
            if *n > 1 { format!("{base}_{n}") } else { base }
        })
        .collect();
    for r in rows.iter_mut() {
        r.resize(width, String::new());
    }
    Ok(Table { file_name, sheet: sheet_name, sheets, headers, rows, truncated })
}

fn cell_to_string(c: &Data) -> String {
    match c {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_string(),
        Data::Float(f) => fmt_num(*f),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(d) => d.as_datetime().map(|dt| dt.format("%Y-%m-%d").to_string()).unwrap_or_else(|| d.to_string()),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(_) => String::new(),
    }
}

/// 사람이 읽기 좋은 숫자 표기 (정수는 소수점 없이, 그 외 최대 4자리).
pub fn fmt_num(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        let s = format!("{:.4}", f);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// "1,234", "85.5%", "₩3,000" 같은 표기도 숫자로 읽는다.
pub fn parse_number(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let cleaned: String = t.chars().filter(|c| !matches!(c, ',' | '%' | '₩' | '$' | ' ' | '원' | '점' | '명')).collect();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok().filter(|f| f.is_finite())
}

fn is_missing(s: &str) -> bool {
    let t = s.trim();
    t.is_empty() || matches!(t.to_ascii_lowercase().as_str(), "na" | "n/a" | "nan" | "null" | "-" | "none" | "#n/a")
}

fn numeric_stats(v: &mut [f64]) -> NumericStats {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len() as f64;
    let sum: f64 = v.iter().sum();
    let mean = sum / n;
    let median = if v.len() % 2 == 1 { v[v.len() / 2] } else { (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0 };
    let std_dev = if v.len() > 1 { Some((v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()) } else { None };
    NumericStats { mean, median, std_dev, min: v[0], max: v[v.len() - 1], sum }
}

/// 데이터 구조·기초 통계·결측치를 계산한다.
pub fn profile(t: &Table) -> TableProfile {
    let mut columns = Vec::new();
    let mut total_missing = 0;
    for (ci, name) in t.headers.iter().enumerate() {
        let vals: Vec<&str> = t.rows.iter().map(|r| r[ci].as_str()).collect();
        let present: Vec<&str> = vals.iter().copied().filter(|v| !is_missing(v)).collect();
        let missing = vals.len() - present.len();
        total_missing += missing;
        let mut nums: Vec<f64> = present.iter().filter_map(|v| parse_number(v)).collect();
        // 값의 90% 이상이 숫자면 숫자 열로 본다
        let kind = if present.is_empty() {
            ColumnKind::Empty
        } else if nums.len() as f64 >= present.len() as f64 * 0.9 {
            ColumnKind::Numeric
        } else {
            ColumnKind::Text
        };
        let mut freq: HashMap<&str, usize> = HashMap::new();
        for v in &present {
            *freq.entry(v).or_default() += 1;
        }
        let unique = freq.len();
        let mut top: Vec<(String, usize)> = freq.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        top.truncate(5);
        let non_numeric = if kind == ColumnKind::Numeric { present.len() - nums.len() } else { 0 };
        let numeric = if kind == ColumnKind::Numeric && !nums.is_empty() { Some(numeric_stats(&mut nums)) } else { None };
        columns.push(ColumnProfile {
            name: name.clone(),
            count: present.len(),
            missing,
            unique,
            numeric,
            top_values: if kind == ColumnKind::Text { top } else { vec![] },
            kind,
            non_numeric,
        });
    }
    TableProfile { row_count: t.rows.len(), column_count: t.headers.len(), columns, total_missing }
}

/// 항목별 집계. `value_column`이 없으면 개수만 센다.
pub fn group_by(t: &Table, group_column: &str, value_column: Option<&str>, agg: Agg) -> AppResult<GroupResult> {
    let gi = t.headers.iter().position(|h| h == group_column).ok_or_else(|| AppError::Other("기준 열을 찾을 수 없습니다".into()))?;
    let vi = match value_column {
        Some(v) => Some(t.headers.iter().position(|h| h == v).ok_or_else(|| AppError::Other("값 열을 찾을 수 없습니다".into()))?),
        None => None,
    };
    if agg != Agg::Count && vi.is_none() {
        return Err(AppError::Other("합계·평균 등을 계산하려면 값 열을 선택해 주세요".into()));
    }
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, (usize, Vec<f64>)> = HashMap::new();
    let mut skipped = 0;
    for r in &t.rows {
        let key = if is_missing(&r[gi]) { "(빈 값)".to_string() } else { r[gi].clone() };
        let e = groups.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            (0, Vec::new())
        });
        e.0 += 1;
        if let Some(vi) = vi {
            match parse_number(&r[vi]) {
                Some(n) => e.1.push(n),
                None => skipped += 1,
            }
        }
    }
    let mut rows: Vec<GroupRow> = order
        .into_iter()
        .map(|g| {
            let (count, nums) = &groups[&g];
            let value = match agg {
                Agg::Count => Some(*count as f64),
                _ if nums.is_empty() => None,
                Agg::Sum => Some(nums.iter().sum()),
                Agg::Mean => Some(nums.iter().sum::<f64>() / nums.len() as f64),
                Agg::Min => nums.iter().cloned().reduce(f64::min),
                Agg::Max => nums.iter().cloned().reduce(f64::max),
            };
            GroupRow { group: g, count: *count, value }
        })
        .collect();
    rows.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal).then(a.group.cmp(&b.group)));
    Ok(GroupResult { group_column: group_column.to_string(), value_column: value_column.map(|s| s.to_string()), agg, rows, skipped })
}

pub fn agg_label(a: Agg) -> &'static str {
    match a {
        Agg::Count => "개수",
        Agg::Sum => "합계",
        Agg::Mean => "평균",
        Agg::Min => "최솟값",
        Agg::Max => "최댓값",
    }
}

/// 통계 결과를 Markdown 표로 만든다 — 보고서 본문과 LLM 입력에 그대로 쓰인다.
/// 개별 행(학생 개인 데이터)은 포함하지 않고 집계값만 포함한다.
pub fn profile_markdown(t: &Table, p: &TableProfile) -> String {
    let mut s = format!("- 파일: {}\n- 행 수: {}\n- 열 수: {}\n- 빈 값(결측치) 합계: {}\n\n", t.file_name, p.row_count, p.column_count, p.total_missing);
    let numeric: Vec<&ColumnProfile> = p.columns.iter().filter(|c| c.kind == ColumnKind::Numeric).collect();
    if !numeric.is_empty() {
        s.push_str("### 숫자 항목 기초 통계\n\n| 항목 | 값 개수 | 빈 값 | 평균 | 중앙값 | 표준편차 | 최솟값 | 최댓값 |\n|---|---:|---:|---:|---:|---:|---:|---:|\n");
        for c in numeric {
            let n = c.numeric.as_ref().unwrap();
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                c.name,
                c.count,
                c.missing,
                fmt2(n.mean),
                fmt2(n.median),
                n.std_dev.map(fmt2).unwrap_or_else(|| "-".into()),
                fmt2(n.min),
                fmt2(n.max)
            ));
        }
        s.push('\n');
    }
    let text: Vec<&ColumnProfile> = p.columns.iter().filter(|c| c.kind == ColumnKind::Text).collect();
    if !text.is_empty() {
        s.push_str("### 문자 항목 요약\n\n| 항목 | 값 개수 | 빈 값 | 서로 다른 값 |\n|---|---:|---:|---:|\n");
        for c in text {
            s.push_str(&format!("| {} | {} | {} | {} |\n", c.name, c.count, c.missing, c.unique));
        }
        s.push('\n');
    }
    s
}

pub fn group_markdown(g: &GroupResult) -> String {
    let label = match &g.value_column {
        Some(v) => format!("{v} {}", agg_label(g.agg)),
        None => "개수".to_string(),
    };
    let mut s = format!("### {}별 {}\n\n| {} | 행 수 | {} |\n|---|---:|---:|\n", g.group_column, label, g.group_column, label);
    for r in g.rows.iter().take(50) {
        s.push_str(&format!("| {} | {} | {} |\n", r.group, r.count, r.value.map(fmt2).unwrap_or_else(|| "-".into())));
    }
    if g.rows.len() > 50 {
        s.push_str(&format!("\n(상위 50개만 표시, 전체 {}개 항목)\n", g.rows.len()));
    }
    if g.skipped > 0 {
        s.push_str(&format!("\n※ 숫자가 아닌 값 {}건은 계산에서 제외했습니다.\n", g.skipped));
    }
    s
}

pub fn fmt2(f: f64) -> String {
    let r = (f * 100.0).round() / 100.0;
    fmt_num(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Table {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("성적.csv");
        std::fs::write(&p, "학과,이름,중간,기말,비고\n철학과,가,80,90,\n철학과,나,70,,재시험\n교육학과,다,\"1,000\",60,\n교육학과,라,90,100,\n교육학과,마,N/A,80,\n").unwrap();
        load(&p, None).unwrap()
    }

    #[test]
    fn loads_csv_with_headers() {
        let t = sample();
        assert_eq!(t.headers, vec!["학과", "이름", "중간", "기말", "비고"]);
        assert_eq!(t.rows.len(), 5);
    }

    #[test]
    fn profile_statistics_are_exact() {
        let t = sample();
        let p = profile(&t);
        assert_eq!(p.row_count, 5);
        let mid = &p.columns[2];
        assert_eq!(mid.kind, ColumnKind::Numeric);
        assert_eq!(mid.missing, 1); // N/A
        let n = mid.numeric.as_ref().unwrap();
        // 80, 70, 1000, 90
        assert_eq!(n.sum, 1240.0);
        assert_eq!(n.mean, 310.0);
        assert_eq!(n.median, 85.0);
        assert_eq!(n.min, 70.0);
        assert_eq!(n.max, 1000.0);
        let fin = &p.columns[3];
        let n = fin.numeric.as_ref().unwrap();
        // 90, 60, 100, 80 → 평균 82.5, 표본표준편차 sqrt(875/3)
        assert_eq!(n.mean, 82.5);
        assert!((n.std_dev.unwrap() - (875.0f64 / 3.0).sqrt()).abs() < 1e-9);
        assert_eq!(fin.missing, 1);
        assert_eq!(p.columns[0].kind, ColumnKind::Text);
        assert_eq!(p.columns[0].unique, 2);
        assert_eq!(p.columns[0].top_values[0], ("교육학과".to_string(), 3));
        assert_eq!(p.columns[4].missing, 4);
        assert_eq!(p.total_missing, 1 + 1 + 4);
    }

    #[test]
    fn group_by_mean_and_count() {
        let t = sample();
        let g = group_by(&t, "학과", Some("기말"), Agg::Mean).unwrap();
        let get = |name: &str| g.rows.iter().find(|r| r.group == name).unwrap();
        assert_eq!(get("교육학과").value, Some(80.0)); // (60+100+80)/3
        assert_eq!(get("철학과").value, Some(90.0)); // 90만 유효
        assert_eq!(g.skipped, 1);
        let c = group_by(&t, "학과", None, Agg::Count).unwrap();
        assert_eq!(c.rows[0].group, "교육학과");
        assert_eq!(c.rows[0].value, Some(3.0));
        assert!(group_by(&t, "학과", None, Agg::Sum).is_err());
        assert!(group_by(&t, "없는열", None, Agg::Count).is_err());
    }

    #[test]
    fn xlsx_roundtrip_matches_csv_numbers() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("data.xlsx");
        let mut wb = rust_xlsxwriter::Workbook::new();
        let ws = wb.add_worksheet();
        ws.write_string(0, 0, "반").unwrap();
        ws.write_string(0, 1, "점수").unwrap();
        for (i, (c, s)) in [("A", 10.0), ("A", 20.0), ("B", 30.5), ("B", 40.0)].iter().enumerate() {
            ws.write_string(i as u32 + 1, 0, *c).unwrap();
            ws.write_number(i as u32 + 1, 1, *s).unwrap();
        }
        wb.save(&p).unwrap();
        let t = load(&p, None).unwrap();
        assert_eq!(t.sheets.len(), 1);
        let pr = profile(&t);
        let n = pr.columns[1].numeric.as_ref().unwrap();
        assert_eq!(n.sum, 100.5);
        assert_eq!(n.mean, 25.125);
        let g = group_by(&t, "반", Some("점수"), Agg::Sum).unwrap();
        assert_eq!(g.rows.iter().find(|r| r.group == "B").unwrap().value, Some(70.5));
    }

    #[test]
    fn markdown_contains_only_aggregates() {
        let t = sample();
        let md = profile_markdown(&t, &profile(&t));
        assert!(md.contains("| 기말 | 4 | 1 | 82.5 |"));
        // 개인 이름(행 데이터)은 통계표에 들어가지 않는다
        assert!(!md.contains("| 가 |"));
    }

    #[test]
    fn number_parsing() {
        assert_eq!(parse_number("1,234.5"), Some(1234.5));
        assert_eq!(parse_number("85%"), Some(85.0));
        assert_eq!(parse_number("90점"), Some(90.0));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number(""), None);
        assert_eq!(fmt_num(3.0), "3");
        assert_eq!(fmt_num(3.14159), "3.1416");
    }

    #[test]
    fn euc_kr_csv_and_duplicate_headers() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.csv");
        let (bytes, _, _) = encoding_rs::EUC_KR.encode("이름,점수,점수\n홍길동,1,2\n");
        std::fs::write(&p, &bytes).unwrap();
        let t = load(&p, None).unwrap();
        assert_eq!(t.headers, vec!["이름", "점수", "점수_2"]);
        assert_eq!(t.rows[0][0], "홍길동");
    }
}

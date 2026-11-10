//! Export Layer — 분석 결과를 Word(.docx), Excel(.xlsx), Markdown, 텍스트로 저장한다.
use crate::error::{AppError, AppResult};
use crate::tabular::{agg_label, ColumnKind, GroupResult, Table, TableProfile};
use docx_rs::{AlignmentType, Docx, Paragraph, Run, RunFonts, Table as DocxTable, TableCell, TableRow};
use std::path::Path;

#[derive(Debug, PartialEq)]
enum Block {
    Heading(usize, String),
    Bullet(String),
    Numbered(String),
    Table(Vec<Vec<String>>),
    Para(String),
}

fn strip_inline(s: &str) -> String {
    s.replace("**", "").replace("__", "").replace('`', "")
}

/// 보고서에 쓰이는 범위의 간단한 Markdown 해석기 (제목, 목록, 표, 문단).
fn parse_markdown(md: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut table: Vec<Vec<String>> = Vec::new();
    let flush = |table: &mut Vec<Vec<String>>, blocks: &mut Vec<Block>| {
        if !table.is_empty() {
            blocks.push(Block::Table(std::mem::take(table)));
        }
    };
    for line in md.lines() {
        let t = line.trim();
        if t.starts_with('|') && t.ends_with('|') && t.len() > 1 {
            let cells: Vec<String> = t[1..t.len() - 1].split('|').map(|c| strip_inline(c.trim())).collect();
            // 구분선(|---|---|)은 건너뛴다
            if !cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' '))) {
                table.push(cells);
            }
            continue;
        }
        flush(&mut table, &mut blocks);
        if t.is_empty() {
            continue;
        }
        let hashes = t.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && t.chars().nth(hashes) == Some(' ') {
            blocks.push(Block::Heading(hashes, strip_inline(t[hashes..].trim())));
        } else if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("• ")) {
            blocks.push(Block::Bullet(strip_inline(rest)));
        } else if t.split_once(". ").map(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())).unwrap_or(false) {
            blocks.push(Block::Numbered(strip_inline(t)));
        } else {
            blocks.push(Block::Para(strip_inline(t)));
        }
    }
    flush(&mut table, &mut blocks);
    blocks
}

fn fonts() -> RunFonts {
    RunFonts::new().east_asia("맑은 고딕").ascii("맑은 고딕").hi_ansi("맑은 고딕")
}

fn run(text: &str, size_pt: usize, bold: bool) -> Run {
    let mut r = Run::new().add_text(text).size(size_pt * 2).fonts(fonts());
    if bold {
        r = r.bold();
    }
    r
}

/// Markdown 텍스트를 Word 문서로 저장한다.
pub fn markdown_to_docx(title: &str, markdown: &str, path: &Path) -> AppResult<()> {
    let mut doc = Docx::new();
    if !title.trim().is_empty() {
        doc = doc.add_paragraph(Paragraph::new().add_run(run(title.trim(), 20, true)).align(AlignmentType::Center));
        doc = doc.add_paragraph(Paragraph::new());
    }
    for b in parse_markdown(markdown) {
        doc = match b {
            Block::Heading(level, text) => {
                let size = match level {
                    1 => 16,
                    2 => 14,
                    _ => 12,
                };
                doc.add_paragraph(Paragraph::new().add_run(run(&text, size, true)))
            }
            Block::Bullet(text) => doc.add_paragraph(Paragraph::new().add_run(run(&format!("•  {text}"), 11, false)).indent(Some(360), None, None, None)),
            Block::Numbered(text) => doc.add_paragraph(Paragraph::new().add_run(run(&text, 11, false)).indent(Some(360), None, None, None)),
            Block::Para(text) => doc.add_paragraph(Paragraph::new().add_run(run(&text, 11, false))),
            Block::Table(rows) => {
                let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
                let trs: Vec<TableRow> = rows
                    .iter()
                    .enumerate()
                    .map(|(ri, r)| {
                        TableRow::new(
                            (0..width)
                                .map(|ci| TableCell::new().add_paragraph(Paragraph::new().add_run(run(r.get(ci).map(|s| s.as_str()).unwrap_or(""), 10, ri == 0))))
                                .collect(),
                        )
                    })
                    .collect();
                doc.add_table(DocxTable::new(trs)).add_paragraph(Paragraph::new())
            }
        };
    }
    let file = std::fs::File::create(path).map_err(|_| AppError::File("파일을 저장할 수 없습니다. 저장 위치를 확인해 주세요.".into()))?;
    doc.build().pack(file).map_err(|_| AppError::File("Word 문서를 만드는 중 오류가 발생했습니다".into()))?;
    Ok(())
}

pub fn save_text(content: &str, path: &Path) -> AppResult<()> {
    std::fs::write(path, content).map_err(|_| AppError::File("파일을 저장할 수 없습니다. 저장 위치를 확인해 주세요.".into()))
}

/// 데이터 분석 결과(기초 통계, 집계)를 Excel 파일로 저장한다.
pub fn analysis_to_xlsx(table: &Table, profile: &TableProfile, group: Option<&GroupResult>, path: &Path) -> AppResult<()> {
    use rust_xlsxwriter::{Format, Workbook};
    let err = |_| AppError::File("Excel 파일을 만드는 중 오류가 발생했습니다".into());
    let mut wb = Workbook::new();
    let bold = Format::new().set_bold();

    let ws = wb.add_worksheet();
    ws.set_name("기초 통계").map_err(err)?;
    let head = ["항목", "종류", "값 개수", "빈 값", "서로 다른 값", "평균", "중앙값", "표준편차", "최솟값", "최댓값", "합계"];
    for (i, h) in head.iter().enumerate() {
        ws.write_string_with_format(0, i as u16, *h, &bold).map_err(err)?;
    }
    for (ri, c) in profile.columns.iter().enumerate() {
        let r = ri as u32 + 1;
        ws.write_string(r, 0, &c.name).map_err(err)?;
        ws.write_string(r, 1, match c.kind {
            ColumnKind::Numeric => "숫자",
            ColumnKind::Text => "문자",
            ColumnKind::Empty => "비어 있음",
        })
        .map_err(err)?;
        ws.write_number(r, 2, c.count as f64).map_err(err)?;
        ws.write_number(r, 3, c.missing as f64).map_err(err)?;
        ws.write_number(r, 4, c.unique as f64).map_err(err)?;
        if let Some(n) = &c.numeric {
            ws.write_number(r, 5, n.mean).map_err(err)?;
            ws.write_number(r, 6, n.median).map_err(err)?;
            if let Some(sd) = n.std_dev {
                ws.write_number(r, 7, sd).map_err(err)?;
            }
            ws.write_number(r, 8, n.min).map_err(err)?;
            ws.write_number(r, 9, n.max).map_err(err)?;
            ws.write_number(r, 10, n.sum).map_err(err)?;
        }
    }
    ws.set_column_width(0, 22).map_err(err)?;
    let info_row = profile.columns.len() as u32 + 3;
    ws.write_string(info_row, 0, format!("원본 파일: {} · 행 {}개 · 열 {}개", table.file_name, profile.row_count, profile.column_count)).map_err(err)?;

    if let Some(g) = group {
        let ws = wb.add_worksheet();
        ws.set_name("항목별 집계").map_err(err)?;
        let label = match &g.value_column {
            Some(v) => format!("{v} {}", agg_label(g.agg)),
            None => "개수".into(),
        };
        ws.write_string_with_format(0, 0, &g.group_column, &bold).map_err(err)?;
        ws.write_string_with_format(0, 1, "행 수", &bold).map_err(err)?;
        ws.write_string_with_format(0, 2, &label, &bold).map_err(err)?;
        for (ri, row) in g.rows.iter().enumerate() {
            let r = ri as u32 + 1;
            ws.write_string(r, 0, &row.group).map_err(err)?;
            ws.write_number(r, 1, row.count as f64).map_err(err)?;
            if let Some(v) = row.value {
                ws.write_number(r, 2, v).map_err(err)?;
            }
        }
        ws.set_column_width(0, 22).map_err(err)?;
    }
    wb.save(path).map_err(|_| AppError::File("파일을 저장할 수 없습니다. 저장 위치를 확인해 주세요.".into()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{documents, tabular};

    #[test]
    fn markdown_blocks() {
        let b = parse_markdown("# 제목\n\n본문 **강조** 입니다.\n- 항목 하나\n1. 첫째\n\n| 가 | 나 |\n|---|---:|\n| 1 | 2 |\n");
        assert_eq!(b[0], Block::Heading(1, "제목".into()));
        assert_eq!(b[1], Block::Para("본문 강조 입니다.".into()));
        assert_eq!(b[2], Block::Bullet("항목 하나".into()));
        assert_eq!(b[3], Block::Numbered("1. 첫째".into()));
        assert_eq!(b[4], Block::Table(vec![vec!["가".into(), "나".into()], vec!["1".into(), "2".into()]]));
    }

    #[test]
    fn docx_export_is_readable_by_our_extractor() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("보고서.docx");
        markdown_to_docx("회의록 요약", "## 결정 사항\n- 교육과정 개편안 승인\n\n| 항목 | 값 |\n|---|---|\n| 참석 | 12 |\n", &p).unwrap();
        let doc = documents::extract(&p).unwrap();
        assert!(doc.text.contains("회의록 요약"));
        assert!(doc.text.contains("결정 사항"));
        assert!(doc.text.contains("교육과정 개편안 승인"));
        assert!(doc.text.contains("참석") && doc.text.contains("12"));
    }

    #[test]
    fn xlsx_export_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("s.csv");
        std::fs::write(&src, "반,점수\nA,10\nA,20\nB,30\n").unwrap();
        let t = tabular::load(&src, None).unwrap();
        let pr = tabular::profile(&t);
        let g = tabular::group_by(&t, "반", Some("점수"), tabular::Agg::Mean).unwrap();
        let out = d.path().join("결과.xlsx");
        analysis_to_xlsx(&t, &pr, Some(&g), &out).unwrap();
        let stats = tabular::load(&out, Some("기초 통계")).unwrap();
        assert_eq!(stats.sheets, vec!["기초 통계", "항목별 집계"]);
        let score = stats.rows.iter().find(|r| r[0] == "점수").unwrap();
        assert_eq!(score[5], "20"); // 평균
        let grp = tabular::load(&out, Some("항목별 집계")).unwrap();
        assert_eq!(grp.rows.iter().find(|r| r[0] == "A").unwrap()[2], "15");
    }
}

//! Document Processing Layer — PDF/DOCX/PPTX/TXT 텍스트 추출과 근거 검색.
//!
//! 모든 처리는 이 컴퓨터의 메모리 안에서 이루어진다. 임시 파일을 만들지 않고,
//! 원본 파일을 수정하지 않으며, 네트워크를 사용하지 않는다.
pub mod retrieve;

use crate::error::{AppError, AppResult};
use quick_xml::events::Event;
use serde::Serialize;
use std::io::Read;
use std::path::Path;

/// 한 문서에서 읽을 최대 크기 (악의적으로 큰 파일·압축 폭탄 방지)
const MAX_FILE_BYTES: u64 = 200 * 1024 * 1024;
const MAX_XML_PART_BYTES: u64 = 50 * 1024 * 1024;
const MAX_TEXT_CHARS: usize = 3_000_000;

#[derive(Debug, Clone, Serialize)]
pub struct ExtractedDocument {
    pub file_name: String,
    pub kind: String,
    pub text: String,
    pub char_count: usize,
    /// 쪽/슬라이드 수 (확인 가능한 경우)
    pub units: Option<usize>,
    pub warnings: Vec<String>,
}

pub fn supported_extensions() -> &'static [&'static str] {
    &["pdf", "docx", "pptx", "txt", "md"]
}

/// 파일에서 텍스트를 추출한다.
pub fn extract(path: &Path) -> AppResult<ExtractedDocument> {
    let file_name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let meta = std::fs::metadata(path).map_err(|_| AppError::File("파일을 열 수 없습니다".into()))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(AppError::File("파일이 너무 큽니다 (최대 200MB)".into()));
    }
    let mut warnings = Vec::new();
    let (text, units) = match ext.as_str() {
        "txt" | "md" => (decode_text(&std::fs::read(path)?), None),
        "pdf" => extract_pdf(path, &mut warnings)?,
        "docx" => extract_docx(path)?,
        "pptx" => extract_pptx(path)?,
        "hwp" | "hwpx" => return Err(AppError::Unsupported("한글(HWP) 문서는 아직 지원하지 않습니다. PDF로 저장한 뒤 불러와 주세요.".into())),
        "doc" | "ppt" => return Err(AppError::Unsupported("옛 형식(.doc/.ppt)은 지원하지 않습니다. 최신 형식(.docx/.pptx)으로 저장해 주세요.".into())),
        _ => return Err(AppError::Unsupported(format!(".{ext} 파일"))),
    };
    let mut text = normalize(&text);
    if text.chars().count() > MAX_TEXT_CHARS {
        text = text.chars().take(MAX_TEXT_CHARS).collect();
        warnings.push("문서가 매우 길어 앞부분만 불러왔습니다.".into());
    }
    if text.trim().is_empty() {
        warnings.push("문서에서 글자를 찾지 못했습니다. 스캔한 이미지 문서일 수 있습니다.".into());
    }
    Ok(ExtractedDocument { file_name, kind: ext, char_count: text.chars().count(), text, units, warnings })
}

/// UTF-8(BOM 포함) → UTF-16 → EUC-KR(CP949) 순으로 해석한다.
pub fn decode_text(bytes: &[u8]) -> String {
    if let Some(b) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(b).into_owned();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let (s, _, _) = if bytes[0] == 0xFF { encoding_rs::UTF_16LE.decode(bytes) } else { encoding_rs::UTF_16BE.decode(bytes) };
        return s.into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::EUC_KR.decode(bytes).0.into_owned(),
    }
}

fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank = 0;
    for line in s.replace('\r', "").replace('\u{0}', "").lines() {
        let t = line.trim_end();
        if t.trim().is_empty() {
            blank += 1;
            if blank <= 1 {
                out.push('\n');
            }
        } else {
            blank = 0;
            out.push_str(t);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

fn extract_pdf(path: &Path, warnings: &mut Vec<String>) -> AppResult<(String, Option<usize>)> {
    let bytes = std::fs::read(path)?;
    // pdf-extract는 손상된 PDF에서 패닉할 수 있으므로 격리한다.
    let res = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(&bytes));
    match res {
        Ok(Ok(text)) => {
            let pages = std::panic::catch_unwind(|| lopdf::Document::load_mem(&bytes).ok().map(|d| d.get_pages().len()))
                .ok()
                .flatten()
                .filter(|n| *n > 0);
            Ok((text.replace('\u{c}', "\n\n"), pages))
        }
        Ok(Err(_)) | Err(_) => {
            warnings.push("PDF를 읽는 중 문제가 있었습니다.".into());
            Err(AppError::File("이 PDF는 읽을 수 없습니다. 암호가 걸려 있거나 손상되었을 수 있습니다.".into()))
        }
    }
}

fn open_zip(path: &Path) -> AppResult<zip::ZipArchive<std::fs::File>> {
    zip::ZipArchive::new(std::fs::File::open(path)?).map_err(|_| AppError::File("문서 파일이 손상되었거나 형식이 올바르지 않습니다".into()))
}

fn read_zip_part(z: &mut zip::ZipArchive<std::fs::File>, name: &str) -> AppResult<Option<String>> {
    let Ok(entry) = z.by_name(name) else { return Ok(None) };
    let mut s = String::new();
    entry.take(MAX_XML_PART_BYTES).read_to_string(&mut s).map_err(|_| AppError::File("문서 내용을 읽지 못했습니다".into()))?;
    Ok(Some(s))
}

/// OOXML에서 텍스트 노드를 모은다. `para`가 끝나면 줄바꿈, `text` 요소의 내용만 수집.
/// 외부 엔티티·DTD는 해석하지 않는다 (quick-xml은 외부 엔티티를 불러오지 않음).
fn xml_text(xml: &str, text_tag: &[u8], para_tag: &[u8], tab_tag: Option<&[u8]>, cell_tag: Option<&[u8]>) -> String {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut out = String::new();
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == text_tag => in_text = true,
            Ok(Event::End(e)) if e.name().as_ref() == text_tag => in_text = false,
            Ok(Event::End(e)) if e.name().as_ref() == para_tag => out.push('\n'),
            Ok(Event::End(e)) if Some(e.name().as_ref()) == cell_tag => out.push_str(" | "),
            Ok(Event::Empty(e)) if Some(e.name().as_ref()) == tab_tag => out.push('\t'),
            Ok(Event::Text(t)) if in_text => out.push_str(&t.unescape().map(|c| c.into_owned()).unwrap_or_default()),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

fn extract_docx(path: &Path) -> AppResult<(String, Option<usize>)> {
    let mut z = open_zip(path)?;
    let xml = read_zip_part(&mut z, "word/document.xml")?.ok_or_else(|| AppError::File("Word 문서 본문을 찾을 수 없습니다".into()))?;
    Ok((xml_text(&xml, b"w:t", b"w:p", Some(b"w:tab"), Some(b"w:tc")), None))
}

fn extract_pptx(path: &Path) -> AppResult<(String, Option<usize>)> {
    let mut z = open_zip(path)?;
    let mut slides: Vec<(u32, String)> = z
        .file_names()
        .filter_map(|n| {
            let num = n.strip_prefix("ppt/slides/slide")?.strip_suffix(".xml")?.parse::<u32>().ok()?;
            Some((num, n.to_string()))
        })
        .collect();
    slides.sort();
    let mut out = String::new();
    for (num, name) in &slides {
        if let Some(xml) = read_zip_part(&mut z, name)? {
            out.push_str(&format!("[슬라이드 {num}]\n"));
            out.push_str(&xml_text(&xml, b"a:t", b"a:p", None, None));
            // 발표자 노트
            if let Some(notes) = read_zip_part(&mut z, &format!("ppt/notesSlides/notesSlide{num}.xml"))? {
                let t = xml_text(&notes, b"a:t", b"a:p", None, None);
                if !t.trim().is_empty() {
                    out.push_str("(발표자 노트) ");
                    out.push_str(t.trim());
                    out.push('\n');
                }
            }
            out.push('\n');
        }
    }
    Ok((out, Some(slides.len())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_zip(path: &Path, parts: &[(&str, &str)]) {
        let mut zw = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (n, c) in parts {
            zw.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            zw.write_all(c.as_bytes()).unwrap();
        }
        zw.finish().unwrap();
    }

    #[test]
    fn docx_text() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("강의.docx");
        write_zip(&p, &[("word/document.xml", r#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>인식론 개론</w:t></w:r></w:p><w:p><w:r><w:t xml:space="preserve">지식은 </w:t></w:r><w:r><w:t>정당화된 참된 믿음 &amp; 그 이상</w:t></w:r></w:p></w:body></w:document>"#)]);
        let doc = extract(&p).unwrap();
        assert_eq!(doc.text, "인식론 개론\n지식은 정당화된 참된 믿음 & 그 이상");
        assert_eq!(doc.kind, "docx");
    }

    #[test]
    fn pptx_slides_in_order_with_notes() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("수업.pptx");
        let slide = |t: &str| format!(r#"<p:sld xmlns:a="a" xmlns:p="p"><a:p><a:r><a:t>{t}</a:t></a:r></a:p></p:sld>"#);
        write_zip(&p, &[
            ("ppt/slides/slide10.xml", &slide("열 번째")),
            ("ppt/slides/slide2.xml", &slide("두 번째")),
            ("ppt/slides/slide1.xml", &slide("첫 번째")),
            ("ppt/notesSlides/notesSlide1.xml", &slide("노트 내용")),
        ]);
        let doc = extract(&p).unwrap();
        assert_eq!(doc.units, Some(3));
        let (a, b, c) = (doc.text.find("첫 번째").unwrap(), doc.text.find("두 번째").unwrap(), doc.text.find("열 번째").unwrap());
        assert!(a < b && b < c);
        assert!(doc.text.contains("(발표자 노트) 노트 내용"));
    }

    #[test]
    fn txt_euc_kr_and_utf8() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.txt");
        let (bytes, _, _) = encoding_rs::EUC_KR.encode("회의록: 교육과정 개편");
        std::fs::write(&p, &bytes).unwrap();
        assert_eq!(extract(&p).unwrap().text, "회의록: 교육과정 개편");
        std::fs::write(&p, "\u{feff}안녕하세요\r\n\r\n\r\n\r\n반갑습니다").unwrap();
        assert_eq!(extract(&p).unwrap().text, "안녕하세요\n\n반갑습니다");
    }

    #[test]
    fn rejects_unsupported_and_corrupt() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.hwp");
        std::fs::write(&p, b"x").unwrap();
        assert!(matches!(extract(&p), Err(AppError::Unsupported(_))));
        let p = d.path().join("bad.docx");
        std::fs::write(&p, b"not a zip").unwrap();
        assert!(matches!(extract(&p), Err(AppError::File(_))));
        let p = d.path().join("bad.pdf");
        std::fs::write(&p, b"%PDF-1.4 garbage").unwrap();
        assert!(extract(&p).is_err());
    }
}

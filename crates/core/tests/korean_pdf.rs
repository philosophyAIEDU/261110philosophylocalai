//! 실제 한국어 PDF(Chromium으로 생성, 글꼴 내장) 텍스트 추출 테스트.
use pla_core::documents;
use std::path::Path;

#[test]
fn extracts_korean_text_from_real_pdf() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/syllabus_ko.pdf");
    let doc = documents::extract(&p).expect("PDF 추출");
    let compact: String = doc.text.chars().filter(|c| !c.is_whitespace()).collect();
    eprintln!("units={:?} text={:?}", doc.units, doc.text);
    assert_eq!(doc.units, Some(2), "쪽 수");
    for needle in ["인식론개론강의계획서", "중간고사는8주차목요일에실시하며", "출석10%", "기말과제40%"] {
        assert!(compact.contains(needle), "추출 결과에 '{needle}' 이(가) 없음");
    }
}

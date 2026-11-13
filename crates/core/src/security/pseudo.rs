//! 가명 처리 — AI에게 학생 기록을 보내기 전에 이름을 "학생1, 학생2…"로 바꾸고
//! 주민번호·전화번호·이메일·학번 같은 패턴을 가린다.
//!
//! AI는 이 컴퓨터 안에서만 실행되지만, AI가 만든 결과는 Word 파일 등으로 저장·공유될 수 있다.
//! 가명 처리를 하면 결과물에 실명과 식별번호가 남지 않는다.
use super::redact::mask_pii;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Alias {
    pub alias: String,
    pub original: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Pseudonymized {
    pub text: String,
    /// 화면에만 보여 주는 대응표 (AI에는 보내지 않음)
    pub aliases: Vec<Alias>,
}

fn is_hangul_syllable(c: char) -> bool {
    ('\u{AC00}'..='\u{D7A3}').contains(&c)
}

/// 이름에서 함께 바꿀 표기들 (긴 것부터). 예: "이서연" → ["이서연", "서연"]
fn name_variants(name: &str) -> Vec<String> {
    let name = name.trim();
    let mut v = vec![name.to_string()];
    let chars: Vec<char> = name.chars().collect();
    if chars.len() == 3 && chars.iter().all(|c| is_hangul_syllable(*c)) {
        v.push(chars[1..].iter().collect());
    }
    v
}

/// `records`: (학생 이름, 본문) 목록. 같은 이름은 같은 가명을 받는다.
pub fn pseudonymize(records: &[(String, String)]) -> (Vec<(String, String)>, Vec<Alias>) {
    let mut aliases: Vec<Alias> = Vec::new();
    for (name, _) in records {
        let n = name.trim();
        if !n.is_empty() && !aliases.iter().any(|a| a.original == n) {
            aliases.push(Alias { alias: format!("학생{}", aliases.len() + 1), original: n.to_string() });
        }
    }
    // 긴 표기부터 바꿔야 "이서연"이 "이학생1"이 되는 일을 막는다.
    let mut replacements: Vec<(String, String)> = aliases
        .iter()
        .flat_map(|a| name_variants(&a.original).into_iter().map(move |v| (v, a.alias.clone())))
        .filter(|(v, _)| v.chars().count() >= 2)
        .collect();
    replacements.sort_by_key(|(v, _)| std::cmp::Reverse(v.chars().count()));
    let apply = |text: &str| {
        let mut t = mask_pii(text);
        for (from, to) in &replacements {
            t = t.replace(from.as_str(), to);
        }
        t
    };
    let out = records
        .iter()
        .map(|(name, body)| {
            let alias = aliases.iter().find(|a| a.original == name.trim()).map(|a| a.alias.clone()).unwrap_or_else(|| "학생".into());
            (alias, apply(body))
        })
        .collect();
    (out, aliases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_pii_are_replaced_consistently() {
        let recs = vec![
            ("이서연".to_string(), "이서연 학생은 진로 고민. 서연이는 010-1234-5678로 연락. 학번 20231234, hong@univ.ac.kr".to_string()),
            ("박준호".to_string(), "박준호, 이서연과 같은 조. 주민번호 990101-1234567".to_string()),
            ("이서연".to_string(), "두 번째 상담".to_string()),
        ];
        let (out, aliases) = pseudonymize(&recs);
        assert_eq!(aliases.len(), 2);
        assert_eq!(out[0].0, "학생1");
        assert_eq!(out[1].0, "학생2");
        assert_eq!(out[2].0, "학생1");
        let all: String = out.iter().map(|(a, b)| format!("{a} {b}")).collect();
        for leaked in ["이서연", "서연", "박준호", "010-1234-5678", "20231234", "hong@univ.ac.kr", "990101-1234567"] {
            assert!(!all.contains(leaked), "{leaked} 가 남아 있음: {all}");
        }
        assert!(out[1].1.contains("학생1과 같은 조"));
        assert!(out[0].1.starts_with("학생1 학생은"));
    }

    #[test]
    fn aliases_like_student_a_are_kept_simple() {
        let (out, aliases) = pseudonymize(&[("학생 A".into(), "학생 A의 고민".into())]);
        assert_eq!(aliases[0].original, "학생 A");
        assert_eq!(out[0].1, "학생1의 고민");
    }
}

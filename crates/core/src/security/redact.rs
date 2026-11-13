//! 로그·오류 메시지에서 개인정보로 보일 수 있는 내용을 제거한다.
//!
//! 원칙: 앱은 문서 원문/질문/응답을 로그에 남기지 않는다. 이 모듈은
//! 외부 라이브러리 오류 문자열 등 "통제하지 못하는 문자열"을 사용자 화면이나
//! 로그에 싣기 전에 거치는 마지막 방어선이다.
use regex::Regex;
use std::sync::OnceLock;

fn patterns() -> &'static Vec<(Regex, &'static str)> {
    static P: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    P.get_or_init(|| {
        vec![
            // 숫자 경계는 ASCII 기준(?-u:\b)으로 본다. 한국어 조사가 바로 붙어도("010-1234-5678로") 가려지도록.
            // 주민등록번호
            (Regex::new(r"(?-u:\b)\d{6}\s*-\s*[1-8]\d{6}(?-u:\b)").unwrap(), "[주민번호]"),
            // 이메일
            (Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap(), "[이메일]"),
            // 전화번호
            (Regex::new(r"(?-u:\b)0\d{1,2}[-.\s]?\d{3,4}[-.\s]?\d{4}(?-u:\b)").unwrap(), "[전화번호]"),
            // 학번 등 8~10자리 숫자
            (Regex::new(r"(?-u:\b)\d{8,10}(?-u:\b)").unwrap(), "[번호]"),
            // 사용자 홈 경로 (사용자 이름 노출 방지)
            (Regex::new(r"(?i)([A-Z]:\\Users\\|/Users/|/home/)[^\\/\s]+").unwrap(), "[사용자폴더]"),
        ]
    })
}

/// 개인정보 패턴(주민번호·이메일·전화번호·8~10자리 번호·사용자 폴더)을 가린다. 길이는 그대로 둔다.
pub fn mask_pii(input: &str) -> String {
    let mut s = input.to_string();
    for (re, rep) in patterns() {
        s = re.replace_all(&s, *rep).into_owned();
    }
    s
}

/// 로그·오류용: 개인정보 패턴을 가리고 길이도 제한한다.
pub fn redact(input: &str) -> String {
    let mut s = mask_pii(input);
    const MAX: usize = 300;
    if s.chars().count() > MAX {
        s = s.chars().take(MAX).collect::<String>() + "…";
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_korean_pii() {
        let s = redact("홍길동 900101-1234567 hong@univ.ac.kr 010-1234-5678 학번 20231234 /Users/hong/성적.xlsx");
        assert!(!s.contains("900101-1234567"));
        assert!(!s.contains("hong@univ.ac.kr"));
        assert!(!s.contains("010-1234-5678"));
        assert!(!s.contains("20231234"));
        assert!(!s.contains("/Users/hong"));
        assert!(s.contains("[주민번호]") && s.contains("[이메일]") && s.contains("[전화번호]"));
    }

    #[test]
    fn masks_numbers_followed_by_korean_particles() {
        let s = mask_pii("연락처는 010-1234-5678로, 학번은 20231234이고 주민번호 900101-1234567입니다");
        assert!(!s.contains("010-1234-5678") && !s.contains("20231234") && !s.contains("900101-1234567"), "{s}");
    }

    #[test]
    fn truncates_long_text() {
        let long = "가".repeat(1000);
        assert!(redact(&long).chars().count() <= 301);
    }
}

//! 근거 검색 — 문서를 조각으로 나누고 질문과 관련된 조각을 찾는다.
//!
//! 임베딩 모델 없이 동작하도록 글자 2-gram + 단어 겹침(IDF 가중) 점수를 쓴다.
//! 한국어는 조사 때문에 단어 단위 일치가 약하므로 글자 2-gram이 효과적이다.
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize)]
pub struct Chunk {
    /// 1부터 시작하는 조각 번호 — 답변의 [근거 n] 표기에 쓰인다
    pub index: usize,
    pub text: String,
}

/// 문단 경계를 살려 약 `target` 글자 단위로 나눈다.
pub fn chunk_text(text: &str, target: usize) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut cur = String::new();
    let push = |cur: &mut String, chunks: &mut Vec<Chunk>| {
        if !cur.trim().is_empty() {
            chunks.push(Chunk { index: chunks.len() + 1, text: cur.trim().to_string() });
        }
        cur.clear();
    };
    for para in text.split('\n') {
        let plen = para.chars().count();
        if plen > target {
            // 매우 긴 문단은 글자 수로 자른다
            push(&mut cur, &mut chunks);
            let chars: Vec<char> = para.chars().collect();
            for piece in chars.chunks(target) {
                cur = piece.iter().collect();
                push(&mut cur, &mut chunks);
            }
            continue;
        }
        if cur.chars().count() + plen + 1 > target {
            push(&mut cur, &mut chunks);
        }
        cur.push_str(para);
        cur.push('\n');
    }
    push(&mut cur, &mut chunks);
    chunks
}

fn features(s: &str) -> HashSet<String> {
    let lower = s.to_lowercase();
    let mut f = HashSet::new();
    for word in lower.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() >= 2 {
            f.insert(word.to_string());
            for w in chars.windows(2) {
                f.insert(w.iter().collect());
            }
        } else if chars[0].is_numeric() {
            f.insert(word.to_string());
        }
    }
    f
}

/// 질문과 가장 관련 있는 조각을 점수 순으로 최대 `k`개 돌려준다 (문서 순서로 재정렬).
pub fn top_chunks<'a>(chunks: &'a [Chunk], query: &str, k: usize) -> Vec<&'a Chunk> {
    if chunks.is_empty() {
        return vec![];
    }
    let chunk_feats: Vec<HashSet<String>> = chunks.iter().map(|c| features(&c.text)).collect();
    let mut df: HashMap<&str, usize> = HashMap::new();
    for cf in &chunk_feats {
        for f in cf {
            *df.entry(f.as_str()).or_default() += 1;
        }
    }
    let n = chunks.len() as f64;
    let q = features(query);
    let mut scored: Vec<(f64, usize)> = chunk_feats
        .iter()
        .enumerate()
        .map(|(i, cf)| {
            let score: f64 = q
                .iter()
                .filter(|f| cf.contains(*f))
                .map(|f| ((n + 1.0) / (*df.get(f.as_str()).unwrap_or(&1) as f64 + 0.5)).ln() * if f.chars().count() > 2 { 2.0 } else { 1.0 })
                .sum();
            (score, i)
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
    let mut picked: Vec<usize> = scored.iter().filter(|(s, _)| *s > 0.0).take(k).map(|(_, i)| *i).collect();
    if picked.is_empty() {
        // 일치하는 조각이 없으면 앞부분을 제공 (모델이 "문서에 없음"을 판단하도록)
        picked = (0..chunks.len().min(k.min(2))).collect();
    }
    picked.sort();
    picked.into_iter().map(|i| &chunks[i]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunking_respects_target_and_keeps_all_text() {
        let text = (0..50).map(|i| format!("문단 {i} 입니다. 내용이 이어집니다.")).collect::<Vec<_>>().join("\n");
        let chunks = chunk_text(&text, 200);
        assert!(chunks.len() > 3);
        assert!(chunks.iter().all(|c| c.text.chars().count() <= 200));
        assert!(chunks.iter().enumerate().all(|(i, c)| c.index == i + 1));
        for i in 0..50 {
            assert!(chunks.iter().any(|c| c.text.contains(&format!("문단 {i} "))));
        }
        let long = "가".repeat(1000);
        assert_eq!(chunk_text(&long, 300).len(), 4);
    }

    #[test]
    fn retrieval_finds_relevant_korean_chunk() {
        let chunks = vec![
            Chunk { index: 1, text: "1주차에는 인식론의 기본 개념과 지식의 정의를 다룬다.".into() },
            Chunk { index: 2, text: "중간고사는 8주차에 실시하며 서술형 3문항으로 구성된다.".into() },
            Chunk { index: 3, text: "기말 과제는 2,000자 분량의 에세이이며 표절 검사를 거친다.".into() },
        ];
        let r = top_chunks(&chunks, "중간고사는 언제인가요?", 1);
        assert_eq!(r[0].index, 2);
        let r = top_chunks(&chunks, "기말 과제의 분량", 1);
        assert_eq!(r[0].index, 3);
        // 관련 없는 질문도 빈 결과를 내지 않는다
        assert!(!top_chunks(&chunks, "xyzzy", 2).is_empty());
    }
}

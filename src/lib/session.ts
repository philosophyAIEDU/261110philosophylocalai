// 메뉴를 옮겨도 작업 중이던 화면(불러온 문서, 결과, 질문 내역)이 남도록 하는 메모리 저장소.
// 앱이 실행되는 동안 메모리에만 있으며, 앱이 잠기거나 종료되면 사라진다 (디스크에 쓰지 않음).
// 오래 보관할 기록은 records.tsx 의 '지난 기록'(암호화 저장)에 남는다.
import { Dispatch, SetStateAction, useCallback, useRef, useState } from "react";

const store = new Map<string, unknown>();

export function clearSession() {
  store.clear();
}

/** useState와 같지만, 같은 key의 값이 메뉴 이동 후에도 유지된다. */
export function useSessionState<T>(key: string, initial: T): [T, Dispatch<SetStateAction<T>>] {
  const initialRef = useRef(initial);
  const [value, setValue] = useState<T>(() => (store.has(key) ? (store.get(key) as T) : initial));
  // 저장소를 기준으로 값을 갱신한다. 화면을 떠난 뒤 도착한 AI 응답도 사라지지 않게 하기 위함.
  const set = useCallback<Dispatch<SetStateAction<T>>>(
    (next) => {
      const prev = (store.has(key) ? store.get(key) : initialRef.current) as T;
      const v = typeof next === "function" ? (next as (p: T) => T)(prev) : next;
      store.set(key, v);
      setValue(v);
    },
    [key],
  );
  return [value, set];
}

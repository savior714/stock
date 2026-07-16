# Legacy Fixture Format — `presets.json`

> 소스: [savior714/stock_vercel](https://github.com/savior714/stock_vercel) 의 `presets.json`
> 최종 확인일: 2026-07-16

---

## 1. 입력 JSON 스키마

`presets.json`은 **플랫 문자열 배열**이다. 중첩 객체, 메타데이터, 키-값 쌍이 없다.

```json
["VLO","PBR","BKR","CONY","RBLX","AAPL","TSLA","MSFT","AMZN","GOOGL"]
```

- **타입**: `string[]`
- **예상 최대 길이**: 현재 파일 기준 약 499개 심볼
- **형식**: JSON 배열 리터럴 (외부 래퍼 객체 없음)
- **인코딩**: UTF-8

### 스키마 정의 (JSON Schema)

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "array",
  "items": {
    "type": "string"
  }
}
```

---

## 2. 위시리스트 이름 생성 규칙

`presets.json`에는 위시리스트 이름이 포함되어 있지 않다. 따라서 가져오기 시 다음 기본 이름을 사용한다.

| 항목 | 값 |
|---|---|
| **기본 위시리스트 이름** | `Legacy Import` |
| **생성 시점** | `GithubSyncRepo.syncPresets()` 호출 시 |
| **이름 재사용** | 동일 이름의 위시리스트가 이미 존재하면 해당 리스트에 추가 |

> **권고**: 향후 `presets.json`에 `name` 필드를 추가하면 사용자 정의 이름을 지원할 수 있다.

---

## 3. 심볼 정규화

| 규칙 | 설명 |
|---|---|
| **대소문자** | 모든 심볼은 이미 대문자로 저장됨. 추가 변환 불필요 |
| **공백 제거** | 기존 데이터는 이미 클린 — 선행/후행 공백 제거 로직은 있지만 실제 공백은 확인되지 않음 |
| **유효성 검사** | import 시점에는 심볼 유효성 검사를 수행하지 않음. 잘못된 심볼(예: `""`, `ABC!@#`)도 그대로 DB에 전달됨 |
| **권고** | 실제 DB 삽입 전에 `^[A-Z]{1,5}$` 패턴으로 검증하여 유효하지 않은 심볼을 필터링하는 것이 안전함 |

---

## 4. 중복 처리

`presets.json` 배열에는 **동일한 심볼이 여러 번 등장할 수 있다**.

- **중복 제거 필수**: DB 삽입 전 `Set` 또는 `DISTINCT`로 중복을 제거해야 함
- **순서 보존**: 중복 제거 시 원래 순서(우선순위)를 유지하는 것이 바람직함
- **예시**:

```json
["AAPL", "TSLA", "AAPL", "MSFT", "TSLA"]
// → 중복 제거 후: ["AAPL", "TSLA", "MSFT"]
```

---

## 5. 500개 이상 심볼 처리

| 항목 | 현황 |
|---|---|
| **현재 최대 기록** | 약 499개 심볼 |
| **배치 처리 필요 여부** | 500개 미만이면 단일 트랜잭션으로 처리 가능 |
| **500개 초과 시** | 배치 처리(예: 100개씩 chunk)를 고려해야 함. SQLite는 단일 `INSERT`의 파라미터 제한이 32766이므로 500개는 문제없음 |
| **권고** | 향후 1000개 이상을 지원하려면 배치 삽입으로 확장 가능 |

---

## 6. 불량 항목 처리

JSON 배열 내에 문자열이 아닌 항목이 포함된 경우:

| 항목 유형 | 처리 방식 |
|---|---|
| **문자열 아님** (예: `null`, `123`, `{}`, `true`) | **스킵 + 경고 로깅** |
| **빈 문자열** (`""`) | **스킵 + 경고 로깅** |
| **null** | **스킵** (JSON 배열에서 `null`은 유효한 값이지만 심볼로 부적절) |
| **공백만 있는 문자열** (`"   "`) | **스킵 + 경고 로깅** |

### 처리 예시 (Rust pseudocode)

```rust
fn normalize_symbols(raw: Vec<serde_json::Value>) -> Vec<String> {
    raw.into_iter()
        .filter_map(|v| match v {
            serde_json::Value::String(s) if !s.trim().is_empty() => {
                Some(s.trim().to_uppercase())
            }
            serde_json::Value::String(_) => {
                eprintln!("WARN: skipped empty string in presets.json");
                None
            }
            _ => {
                eprintln!("WARN: skipped non-string entry in presets.json: {:?}", v);
                None
            }
        })
        .collect()
}
```

---

## 요약

| 구분 | 내용 |
|---|---|
| **형식** | 플랫 문자열 배열 |
| **최대 크기** | ~499 심볼 (현재) |
| **대소문자** | 대문자 고정 |
| **중복** | 존재 가능 → 삽입 전 dedup 필수 |
| **이름** | 없음 → 기본값 `Legacy Import` 사용 |
| **불량 항목** | 문자열 아님 / 빈 문자열 → 스킵 + 경고 |

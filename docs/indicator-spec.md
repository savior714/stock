# Indicator Specification

## 공통 규칙

- 입력은 날짜 오름차순의 유효한 일봉 시계열이다.
- 계산 함수는 입력을 변경하지 않는다.
- warm-up 구간은 `None`/`null`로 표현하고 0으로 대체하지 않는다.
- UI 표시를 위한 반올림은 계산 완료 후 presentation 계층에서 수행한다.
- signal 판정은 반올림 전 값을 사용한다.

## RSI

- 기본 기간: 14
- Wilder smoothing 사용
- 첫 평균 gain/loss는 최초 `period` 변화량의 단순 평균
- 이후 평균은 Wilder 방식으로 갱신
- 평균 loss가 0이고 gain이 양수이면 100
- gain과 loss가 모두 0이면 50으로 정의
- 첫 유효 값은 `period`개의 변화량이 확보되는 index `period`에 생성

### 조건

- 하단 current: `rsi <= lower_threshold`
- 상단 current: `rsi >= upper_threshold`
- 하단 cross: `previous_rsi > lower_threshold && current_rsi <= lower_threshold`
- 상단 cross: `previous_rsi < upper_threshold && current_rsi >= upper_threshold`

## MFI

- 기본 기간: 14
- Typical Price: `(high + low + close) / 3`
- Raw Money Flow: `typical_price * volume`
- Typical Price가 전일보다 높으면 positive, 낮으면 negative, 같으면 양쪽 모두 미포함
- 각 거래일의 MFI는 최근 `period`개의 분류된 money flow 합만 사용한다.
- window에서 벗어난 과거 money flow는 다음 계산에 남기지 않는다.
- negative flow 합이 0이고 positive flow 합이 양수이면 100
- positive와 negative flow 합이 모두 0이면 50
- 첫 유효 값은 `period`개의 변화량이 확보되는 index `period`에 생성

### 조건

RSI와 동일한 current/cross 비교 규칙을 사용한다.

## Bollinger Bands

- 기본 기간: 20
- 앱 전략 기본 표준편차 배수: 1.0 (legacy preset 호환)
- 기준 가격: Close
- 중앙선: SMA
- 표준편차: population standard deviation (`ddof = 0`)
- index `i`의 rolling window는 현재 값인 `close[i]`를 포함한다.
- 첫 유효 값은 `period`개의 close가 확보되는 index `period - 1`에 생성한다.

```text
window(i) = close[i + 1 - period ..= i]
middle = SMA(window, period)
upper = middle + multiplier * stddev
lower = middle - multiplier * stddev
```

### 감지 가격

MVP는 `close`로 고정한다.

- 하단 current: `close <= lower`
- 상단 current: `close >= upper`

`high_low`(low ≤ lower / high ≥ upper)는 post-MVP 후보로 기록한다.

### cross

하단:

- 이전 bar는 `close`가 lower를 터치하지 않았고 (`close > lower`)
- 현재 bar는 `close <= lower`

상단도 대칭적으로 정의한다. 이전 bar와 현재 bar의 band 값은 각 거래일의 rolling 결과를 사용한다.

## Signal Engine

각 활성 조건은 고유 ID를 가진다.

```text
bollinger.lower
bollinger.upper
rsi.lower
rsi.upper
mfi.lower
mfi.upper
```

- Single: 선택 ID의 match 여부
- AND: 활성 조건 수가 1개 이상이며 모든 조건이 match
- OR: 활성 조건 중 하나 이상 match

활성 조건이 0개일 때 AND/OR 결과는 false다.

## 가격 보정

동일 시계열 안에서 adjusted close만 raw high/low/volume과 혼합하지 않는다.

MVP에서는 Yahoo가 제공하는 raw OHLCV를 일관되게 사용하고 split 발생 시 전체 과거 구간 재동기화 전략을 우선 검토한다. 추후 adjusted OHLCV를 생성하려면 동일 adjustment factor를 open/high/low/close에 모두 적용하고 volume 정책을 명시해야 한다.

## Golden tests

- 상승만 존재하는 RSI
- 하락만 존재하는 RSI
- 가격 불변 RSI
- MFI positive-only / negative-only / flat
- MFI rolling window에서 오래된 flow 제거
- Bollinger constant series
- Bollinger가 현재 close를 포함하는지
- threshold와 정확히 같은 값
- current와 cross 차이
- upper/lower 동시 활성화
- warm-up 경계

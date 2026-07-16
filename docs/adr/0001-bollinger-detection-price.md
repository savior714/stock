# ADR-0001: Bollinger Bands 감지 가격 기준

**Status**: Accepted
**Date**: 2026-07-15

## Context

`docs/indicator-spec.md`에는 Bollinger Bands signal 감지를 위한 `high_low`와 `close` 두 가지 감지 가격 방식이 모두 기술되어 있었다. 하지만 현재 Scan Preset schema(`parameters_json`)와 UI에는 감지 가격 선택 필드가 없고 multiplier만 저장한다.

기존 앱은 마지막 adjusted close를 Bollinger lower/upper band와 비교하는 방식으로 동작했다. Signal Engine 구현 시 어떤 가격을 기준으로 할지 명확히 결정해야 한다.

## Decision

MVP는 **`close`** 판정으로 고정한다.

- 하단 current: `close <= lower`
- 상단 current: `close >= upper`
- cross: 전일 `close > lower` → 당일 `close <= lower`(하단), 상단은 대칭

## Consequences

- Signal Engine 구현자는 별도 판단 없이 `close` 기준만으로 구현 가능
- 기존 앱 동작과 일관됨
- `parameters_json` schema에 새로운 field 추가 없이 MVP 완료 가능
- `high_low`(low ≤ lower / high ≥ upper) 감지 방식은 post-MVP 기능 후보로 기록
- 추후 `high_low` 추가 시 `parameters_json`에 `detection_price` 필드 추가 및 UI 선택 필드 구현 필요

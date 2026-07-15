use super::*;
use crate::error::AppErrorCode;

fn condition(
    indicator: IndicatorKind,
    side: SignalSide,
    period: u32,
    threshold: Option<f64>,
    std_dev_multiplier: Option<f64>,
    trigger_mode: TriggerMode,
    enabled: bool,
) -> ScanConditionWrite {
    ScanConditionWrite {
        indicator,
        side,
        period,
        threshold,
        std_dev_multiplier,
        trigger_mode,
        enabled,
    }
}

fn six_conditions() -> Vec<ScanConditionWrite> {
    vec![
        condition(
            IndicatorKind::Rsi,
            SignalSide::Lower,
            14,
            Some(30.0),
            None,
            TriggerMode::Cross,
            true,
        ),
        condition(
            IndicatorKind::Rsi,
            SignalSide::Upper,
            14,
            Some(70.0),
            None,
            TriggerMode::Current,
            false,
        ),
        condition(
            IndicatorKind::Mfi,
            SignalSide::Lower,
            14,
            Some(30.0),
            None,
            TriggerMode::Current,
            true,
        ),
        condition(
            IndicatorKind::Mfi,
            SignalSide::Upper,
            14,
            Some(70.0),
            None,
            TriggerMode::Cross,
            false,
        ),
        condition(
            IndicatorKind::Bollinger,
            SignalSide::Lower,
            20,
            None,
            Some(1.0),
            TriggerMode::Current,
            true,
        ),
        condition(
            IndicatorKind::Bollinger,
            SignalSide::Upper,
            20,
            None,
            Some(1.0),
            TriggerMode::Cross,
            false,
        ),
    ]
}

#[test]
fn exposes_seeded_legacy_default_preset() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let repository = ScanPresetRepository::new(&mut database);

    let presets = repository.list().expect("presets must list");
    let default = presets
        .iter()
        .find(|preset| preset.id.0 == "default-legacy-triple")
        .expect("default preset must exist");
    let detail = repository
        .get(&default.id)
        .expect("default preset must load");

    assert_eq!(default.name, "기존 앱 기본값");
    assert_eq!(default.enabled_condition_count, 3);
    assert_eq!(detail.conditions.len(), 6);
    assert_eq!(detail.conditions[0].threshold, Some(30.0));
    assert_eq!(detail.conditions[2].threshold, Some(30.0));
    assert_eq!(detail.conditions[4].std_dev_multiplier, Some(1.0));
    assert!(detail.conditions[0].enabled);
    assert!(detail.conditions[2].enabled);
    assert!(detail.conditions[4].enabled);
}

#[test]
fn creates_preset_with_condition_specific_trigger_modes() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = ScanPresetRepository::new(&mut database);

    let created = repository
        .create(ScanPresetWrite {
            name: "Mixed triggers".to_string(),
            conditions: six_conditions(),
        })
        .expect("preset must create");

    assert_eq!(created.conditions.len(), 6);
    assert_eq!(created.conditions[0].trigger_mode, TriggerMode::Cross);
    assert_eq!(created.conditions[2].trigger_mode, TriggerMode::Current);
    assert_eq!(created.conditions[5].trigger_mode, TriggerMode::Cross);
    assert_eq!(created.conditions[4].std_dev_multiplier, Some(1.0));
}

#[test]
fn rejects_missing_or_duplicate_condition_slots() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = ScanPresetRepository::new(&mut database);
    let mut missing = six_conditions();
    missing.pop();

    let missing_error = repository
        .create(ScanPresetWrite {
            name: "Missing".to_string(),
            conditions: missing,
        })
        .expect_err("missing slot must fail");

    let mut duplicate = six_conditions();
    duplicate[5] = condition(
        IndicatorKind::Rsi,
        SignalSide::Lower,
        14,
        Some(25.0),
        None,
        TriggerMode::Current,
        false,
    );
    let duplicate_error = repository
        .create(ScanPresetWrite {
            name: "Duplicate".to_string(),
            conditions: duplicate,
        })
        .expect_err("duplicate slot must fail");

    assert_eq!(missing_error.code, AppErrorCode::Validation);
    assert_eq!(duplicate_error.code, AppErrorCode::Validation);
}

#[test]
fn rejects_duplicate_names_case_insensitively() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = ScanPresetRepository::new(&mut database);
    repository
        .create(ScanPresetWrite {
            name: "Momentum".to_string(),
            conditions: six_conditions(),
        })
        .expect("first preset must create");

    let error = repository
        .create(ScanPresetWrite {
            name: "momentum".to_string(),
            conditions: six_conditions(),
        })
        .expect_err("duplicate name must fail");

    assert_eq!(error.code, AppErrorCode::Conflict);
}

#[test]
fn updates_and_deletes_a_preset() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = ScanPresetRepository::new(&mut database);
    let created = repository
        .create(ScanPresetWrite {
            name: "Original".to_string(),
            conditions: six_conditions(),
        })
        .expect("preset must create");
    let mut updated_conditions = six_conditions();
    updated_conditions[0].threshold = Some(35.0);
    updated_conditions[0].trigger_mode = TriggerMode::Current;

    let updated = repository
        .update(
            &created.id,
            ScanPresetWrite {
                name: "Updated".to_string(),
                conditions: updated_conditions,
            },
        )
        .expect("preset must update");

    assert_eq!(updated.name, "Updated");
    assert_eq!(updated.conditions[0].threshold, Some(35.0));
    assert_eq!(updated.conditions[0].trigger_mode, TriggerMode::Current);

    repository.delete(&created.id).expect("preset must delete");
    let error = repository
        .get(&created.id)
        .expect_err("deleted preset must not load");
    assert_eq!(error.code, AppErrorCode::NotFound);
}

#[test]
fn rejects_invalid_indicator_parameters_and_all_disabled() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = ScanPresetRepository::new(&mut database);
    let mut invalid = six_conditions();
    invalid[0].std_dev_multiplier = Some(2.0);

    let parameter_error = repository
        .create(ScanPresetWrite {
            name: "Invalid parameters".to_string(),
            conditions: invalid,
        })
        .expect_err("invalid parameters must fail");

    let mut disabled = six_conditions();
    for condition in &mut disabled {
        condition.enabled = false;
    }
    let disabled_error = repository
        .create(ScanPresetWrite {
            name: "Disabled".to_string(),
            conditions: disabled,
        })
        .expect_err("all disabled must fail");

    assert_eq!(parameter_error.code, AppErrorCode::Validation);
    assert_eq!(disabled_error.code, AppErrorCode::Validation);
}

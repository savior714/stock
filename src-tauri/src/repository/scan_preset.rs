use crate::db::Database;
use crate::domain::{IndicatorKind, ScanPresetId, SignalSide, TriggerMode};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

const MAX_PRESET_NAME_LENGTH: usize = 80;
const MIN_PERIOD: u32 = 2;
const MAX_PERIOD: u32 = 500;
const MIN_STD_DEV_MULTIPLIER: f64 = 0.1;
const MAX_STD_DEV_MULTIPLIER: f64 = 10.0;
const CONDITION_SLOT_COUNT: usize = 6;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanPresetWrite {
    pub name: String,
    pub conditions: Vec<ScanConditionWrite>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanConditionWrite {
    pub indicator: IndicatorKind,
    pub side: SignalSide,
    pub period: u32,
    pub threshold: Option<f64>,
    pub std_dev_multiplier: Option<f64>,
    pub trigger_mode: TriggerMode,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanPresetSummary {
    pub id: ScanPresetId,
    pub name: String,
    pub enabled_condition_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanPresetDetail {
    pub id: ScanPresetId,
    pub name: String,
    pub conditions: Vec<ScanConditionDetail>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanConditionDetail {
    pub indicator: IndicatorKind,
    pub side: SignalSide,
    pub period: u32,
    pub threshold: Option<f64>,
    pub std_dev_multiplier: Option<f64>,
    pub trigger_mode: TriggerMode,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
struct NormalizedScanPresetWrite {
    name: String,
    conditions: Vec<NormalizedScanCondition>,
}

#[derive(Debug, Clone)]
struct NormalizedScanCondition {
    indicator: IndicatorKind,
    side: SignalSide,
    period: u32,
    threshold: Option<f64>,
    std_dev_multiplier: Option<f64>,
    trigger_mode: TriggerMode,
    enabled: bool,
    sort_order: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BollingerParameters {
    std_dev_multiplier: f64,
}

pub struct ScanPresetRepository<'connection> {
    connection: &'connection mut Connection,
}

impl<'connection> ScanPresetRepository<'connection> {
    pub fn new(database: &'connection mut Database) -> Self {
        Self {
            connection: database.connection_mut(),
        }
    }

    pub fn list(&self) -> AppResult<Vec<ScanPresetSummary>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT p.id, p.name,
                        SUM(CASE WHEN c.is_enabled = 1 THEN 1 ELSE 0 END)
                 FROM scan_presets p
                 LEFT JOIN scan_preset_conditions c ON c.preset_id = p.id
                 GROUP BY p.id, p.name
                 ORDER BY lower(p.name), p.id",
            )
            .map_err(|error| db_error("failed to prepare Scan preset list query", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|error| db_error("failed to list Scan presets", error))?;
        let raw_rows = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| db_error("failed to read Scan preset rows", error))?;

        raw_rows
            .into_iter()
            .map(|(id, name, enabled_condition_count)| {
                Ok(ScanPresetSummary {
                    id: ScanPresetId::new(id)?,
                    name,
                    enabled_condition_count: enabled_condition_count as u32,
                })
            })
            .collect()
    }

    pub fn get(&self, id: &ScanPresetId) -> AppResult<ScanPresetDetail> {
        let preset = self
            .connection
            .query_row(
                "SELECT id, name FROM scan_presets WHERE id = ?1",
                params![id.0.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| db_error("failed to load Scan preset", error))?
            .ok_or_else(|| AppError::not_found("Scan preset not found"))?;

        let mut statement = self
            .connection
            .prepare(
                "SELECT indicator, side, period, threshold, parameters_json,
                        trigger_mode, is_enabled
                 FROM scan_preset_conditions
                 WHERE preset_id = ?1
                 ORDER BY sort_order, indicator, side",
            )
            .map_err(|error| db_error("failed to prepare Scan condition query", error))?;
        let rows = statement
            .query_map(params![id.0.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<f64>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })
            .map_err(|error| db_error("failed to load Scan conditions", error))?;
        let raw_conditions = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| db_error("failed to read Scan condition rows", error))?;
        let conditions = raw_conditions
            .into_iter()
            .map(
                |(
                    indicator,
                    side,
                    period,
                    threshold,
                    parameters_json,
                    trigger_mode,
                    is_enabled,
                )| {
                    let indicator = parse_indicator(&indicator)?;
                    let std_dev_multiplier = if indicator == IndicatorKind::Bollinger {
                        let parameters: BollingerParameters =
                            serde_json::from_str(&parameters_json).map_err(|error| {
                                AppError::database(
                                    "failed to decode Bollinger parameters",
                                    error.to_string(),
                                )
                            })?;
                        Some(parameters.std_dev_multiplier)
                    } else {
                        None
                    };

                    Ok(ScanConditionDetail {
                        indicator,
                        side: parse_side(&side)?,
                        period: u32::try_from(period).map_err(|error| {
                            AppError::database("invalid Scan condition period", error.to_string())
                        })?,
                        threshold,
                        std_dev_multiplier,
                        trigger_mode: parse_trigger_mode(&trigger_mode)?,
                        enabled: is_enabled != 0,
                    })
                },
            )
            .collect::<AppResult<Vec<_>>>()?;

        Ok(ScanPresetDetail {
            id: ScanPresetId::new(preset.0)?,
            name: preset.1,
            conditions,
        })
    }

    pub fn create(&mut self, input: ScanPresetWrite) -> AppResult<ScanPresetDetail> {
        let normalized = normalize_write(input)?;
        if self.name_exists(&normalized.name, None)? {
            return Err(AppError::conflict(
                "a Scan preset with this name already exists",
            ));
        }

        let transaction = self
            .connection
            .transaction()
            .map_err(|error| db_error("failed to start Scan preset transaction", error))?;
        let raw_id: String = transaction
            .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
            .map_err(|error| db_error("failed to generate Scan preset id", error))?;
        let id = ScanPresetId::new(raw_id)?;

        transaction
            .execute(
                "INSERT INTO scan_presets (id, name, trigger_mode)
                 VALUES (?1, ?2, 'current')",
                params![id.0.as_str(), &normalized.name],
            )
            .map_err(|error| db_error("failed to create Scan preset", error))?;
        replace_conditions(&transaction, &id, &normalized.conditions)?;
        transaction
            .commit()
            .map_err(|error| db_error("failed to commit Scan preset creation", error))?;

        self.get(&id)
    }

    pub fn update(
        &mut self,
        id: &ScanPresetId,
        input: ScanPresetWrite,
    ) -> AppResult<ScanPresetDetail> {
        let normalized = normalize_write(input)?;
        if self.name_exists(&normalized.name, Some(id))? {
            return Err(AppError::conflict(
                "a Scan preset with this name already exists",
            ));
        }

        let transaction = self
            .connection
            .transaction()
            .map_err(|error| db_error("failed to start Scan preset transaction", error))?;
        let changed = transaction
            .execute(
                "UPDATE scan_presets
                 SET name = ?1, trigger_mode = 'current', updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?2",
                params![&normalized.name, id.0.as_str()],
            )
            .map_err(|error| db_error("failed to update Scan preset", error))?;
        if changed == 0 {
            return Err(AppError::not_found("Scan preset not found"));
        }

        replace_conditions(&transaction, id, &normalized.conditions)?;
        transaction
            .commit()
            .map_err(|error| db_error("failed to commit Scan preset update", error))?;

        self.get(id)
    }

    pub fn delete(&mut self, id: &ScanPresetId) -> AppResult<()> {
        let changed = self
            .connection
            .execute(
                "DELETE FROM scan_presets WHERE id = ?1",
                params![id.0.as_str()],
            )
            .map_err(|error| db_error("failed to delete Scan preset", error))?;

        if changed == 0 {
            return Err(AppError::not_found("Scan preset not found"));
        }
        Ok(())
    }

    fn name_exists(&self, name: &str, excluding_id: Option<&ScanPresetId>) -> AppResult<bool> {
        let count: i64 = match excluding_id {
            Some(id) => self.connection.query_row(
                "SELECT COUNT(*) FROM scan_presets
                 WHERE name = ?1 COLLATE NOCASE AND id <> ?2",
                params![name, id.0.as_str()],
                |row| row.get(0),
            ),
            None => self.connection.query_row(
                "SELECT COUNT(*) FROM scan_presets WHERE name = ?1 COLLATE NOCASE",
                params![name],
                |row| row.get(0),
            ),
        }
        .map_err(|error| db_error("failed to check Scan preset name", error))?;

        Ok(count > 0)
    }
}

fn normalize_write(input: ScanPresetWrite) -> AppResult<NormalizedScanPresetWrite> {
    let name = input.name.trim().to_string();
    if name.is_empty() || name.len() > MAX_PRESET_NAME_LENGTH {
        return Err(AppError::validation(format!(
            "Scan preset name must contain 1 to {MAX_PRESET_NAME_LENGTH} characters"
        )));
    }
    if input.conditions.len() != CONDITION_SLOT_COUNT {
        return Err(AppError::validation(
            "a Scan preset must contain exactly six fixed condition slots",
        ));
    }

    let mut occupied_slots = [false; CONDITION_SLOT_COUNT];
    let mut enabled_count = 0usize;
    let mut conditions = Vec::with_capacity(CONDITION_SLOT_COUNT);

    for condition in input.conditions {
        let slot = condition_slot(condition.indicator, condition.side);
        if occupied_slots[slot] {
            return Err(AppError::validation(format!(
                "duplicate condition slot: {} {}",
                indicator_db_value(condition.indicator),
                side_db_value(condition.side)
            )));
        }
        occupied_slots[slot] = true;

        if !(MIN_PERIOD..=MAX_PERIOD).contains(&condition.period) {
            return Err(AppError::validation(format!(
                "condition period must be between {MIN_PERIOD} and {MAX_PERIOD}"
            )));
        }

        let (threshold, std_dev_multiplier) = match condition.indicator {
            IndicatorKind::Rsi | IndicatorKind::Mfi => {
                let threshold = condition.threshold.ok_or_else(|| {
                    AppError::validation("RSI and MFI conditions require a threshold")
                })?;
                if !threshold.is_finite() || !(0.0..=100.0).contains(&threshold) {
                    return Err(AppError::validation(
                        "RSI and MFI thresholds must be between 0 and 100",
                    ));
                }
                if condition.std_dev_multiplier.is_some() {
                    return Err(AppError::validation(
                        "RSI and MFI conditions must not include a standard deviation multiplier",
                    ));
                }
                (Some(threshold), None)
            }
            IndicatorKind::Bollinger => {
                if condition.threshold.is_some() {
                    return Err(AppError::validation(
                        "Bollinger conditions must not include a threshold",
                    ));
                }
                let multiplier = condition.std_dev_multiplier.ok_or_else(|| {
                    AppError::validation(
                        "Bollinger conditions require a standard deviation multiplier",
                    )
                })?;
                if !multiplier.is_finite()
                    || !(MIN_STD_DEV_MULTIPLIER..=MAX_STD_DEV_MULTIPLIER)
                        .contains(&multiplier)
                {
                    return Err(AppError::validation(format!(
                        "Bollinger multiplier must be between {MIN_STD_DEV_MULTIPLIER} and {MAX_STD_DEV_MULTIPLIER}"
                    )));
                }
                (None, Some(multiplier))
            }
        };

        if condition.enabled {
            enabled_count += 1;
        }
        conditions.push(NormalizedScanCondition {
            indicator: condition.indicator,
            side: condition.side,
            period: condition.period,
            threshold,
            std_dev_multiplier,
            trigger_mode: condition.trigger_mode,
            enabled: condition.enabled,
            sort_order: slot as i64,
        });
    }

    if occupied_slots.iter().any(|occupied| !occupied) {
        return Err(AppError::validation(
            "a Scan preset must contain all six fixed condition slots",
        ));
    }
    if enabled_count == 0 {
        return Err(AppError::validation(
            "a Scan preset must enable at least one condition",
        ));
    }

    conditions.sort_by_key(|condition| condition.sort_order);
    Ok(NormalizedScanPresetWrite { name, conditions })
}

fn replace_conditions(
    transaction: &Transaction<'_>,
    id: &ScanPresetId,
    conditions: &[NormalizedScanCondition],
) -> AppResult<()> {
    transaction
        .execute(
            "DELETE FROM scan_preset_conditions WHERE preset_id = ?1",
            params![id.0.as_str()],
        )
        .map_err(|error| db_error("failed to clear Scan preset conditions", error))?;

    for condition in conditions {
        let condition_id: String = transaction
            .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
            .map_err(|error| db_error("failed to generate Scan condition id", error))?;
        let parameters_json = match condition.std_dev_multiplier {
            Some(std_dev_multiplier) => serde_json::to_string(&BollingerParameters {
                std_dev_multiplier,
            })
            .map_err(|error| {
                AppError::internal("failed to encode Bollinger parameters", error.to_string())
            })?,
            None => "{}".to_string(),
        };

        transaction
            .execute(
                "INSERT INTO scan_preset_conditions (
                    id, preset_id, indicator, side, period, threshold,
                    parameters_json, is_enabled, sort_order, trigger_mode
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    condition_id,
                    id.0.as_str(),
                    indicator_db_value(condition.indicator),
                    side_db_value(condition.side),
                    i64::from(condition.period),
                    condition.threshold,
                    parameters_json,
                    i64::from(condition.enabled),
                    condition.sort_order,
                    trigger_mode_db_value(condition.trigger_mode),
                ],
            )
            .map_err(|error| db_error("failed to save Scan condition", error))?;
    }
    Ok(())
}

fn condition_slot(indicator: IndicatorKind, side: SignalSide) -> usize {
    match (indicator, side) {
        (IndicatorKind::Rsi, SignalSide::Lower) => 0,
        (IndicatorKind::Rsi, SignalSide::Upper) => 1,
        (IndicatorKind::Mfi, SignalSide::Lower) => 2,
        (IndicatorKind::Mfi, SignalSide::Upper) => 3,
        (IndicatorKind::Bollinger, SignalSide::Lower) => 4,
        (IndicatorKind::Bollinger, SignalSide::Upper) => 5,
    }
}

fn indicator_db_value(indicator: IndicatorKind) -> &'static str {
    match indicator {
        IndicatorKind::Bollinger => "bollinger",
        IndicatorKind::Rsi => "rsi",
        IndicatorKind::Mfi => "mfi",
    }
}

fn side_db_value(side: SignalSide) -> &'static str {
    match side {
        SignalSide::Lower => "lower",
        SignalSide::Upper => "upper",
    }
}

fn trigger_mode_db_value(trigger_mode: TriggerMode) -> &'static str {
    match trigger_mode {
        TriggerMode::Current => "current",
        TriggerMode::Cross => "cross",
    }
}

fn parse_indicator(value: &str) -> AppResult<IndicatorKind> {
    match value {
        "bollinger" => Ok(IndicatorKind::Bollinger),
        "rsi" => Ok(IndicatorKind::Rsi),
        "mfi" => Ok(IndicatorKind::Mfi),
        _ => Err(AppError::database(
            "invalid indicator stored in database",
            value,
        )),
    }
}

fn parse_side(value: &str) -> AppResult<SignalSide> {
    match value {
        "lower" => Ok(SignalSide::Lower),
        "upper" => Ok(SignalSide::Upper),
        _ => Err(AppError::database(
            "invalid condition side stored in database",
            value,
        )),
    }
}

fn parse_trigger_mode(value: &str) -> AppResult<TriggerMode> {
    match value {
        "current" => Ok(TriggerMode::Current),
        "cross" => Ok(TriggerMode::Cross),
        _ => Err(AppError::database(
            "invalid trigger mode stored in database",
            value,
        )),
    }
}

fn db_error(message: &'static str, error: rusqlite::Error) -> AppError {
    AppError::database(message, error.to_string())
}

#[cfg(test)]
#[path = "scan_preset_tests.rs"]
mod tests;

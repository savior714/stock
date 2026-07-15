BEGIN IMMEDIATE;

ALTER TABLE scan_preset_conditions
ADD COLUMN trigger_mode TEXT NOT NULL DEFAULT 'current'
CHECK (trigger_mode IN ('current', 'cross'));

UPDATE scan_preset_conditions
SET trigger_mode = COALESCE(
    (
        SELECT scan_presets.trigger_mode
        FROM scan_presets
        WHERE scan_presets.id = scan_preset_conditions.preset_id
    ),
    'current'
);

INSERT INTO scan_presets (id, name, trigger_mode)
SELECT 'default-legacy-triple', '기존 앱 기본값', 'current'
WHERE NOT EXISTS (SELECT 1 FROM scan_presets);

INSERT INTO scan_preset_conditions (
    id,
    preset_id,
    indicator,
    side,
    period,
    threshold,
    parameters_json,
    is_enabled,
    sort_order,
    trigger_mode
)
SELECT 'default-rsi-lower', 'default-legacy-triple', 'rsi', 'lower', 14, 30.0, '{}', 1, 0, 'current'
WHERE EXISTS (SELECT 1 FROM scan_presets WHERE id = 'default-legacy-triple')
  AND NOT EXISTS (
      SELECT 1 FROM scan_preset_conditions
      WHERE preset_id = 'default-legacy-triple' AND indicator = 'rsi' AND side = 'lower'
  );

INSERT INTO scan_preset_conditions (
    id, preset_id, indicator, side, period, threshold, parameters_json,
    is_enabled, sort_order, trigger_mode
)
SELECT 'default-rsi-upper', 'default-legacy-triple', 'rsi', 'upper', 14, 70.0, '{}', 0, 1, 'current'
WHERE EXISTS (SELECT 1 FROM scan_presets WHERE id = 'default-legacy-triple')
  AND NOT EXISTS (
      SELECT 1 FROM scan_preset_conditions
      WHERE preset_id = 'default-legacy-triple' AND indicator = 'rsi' AND side = 'upper'
  );

INSERT INTO scan_preset_conditions (
    id, preset_id, indicator, side, period, threshold, parameters_json,
    is_enabled, sort_order, trigger_mode
)
SELECT 'default-mfi-lower', 'default-legacy-triple', 'mfi', 'lower', 14, 30.0, '{}', 1, 2, 'current'
WHERE EXISTS (SELECT 1 FROM scan_presets WHERE id = 'default-legacy-triple')
  AND NOT EXISTS (
      SELECT 1 FROM scan_preset_conditions
      WHERE preset_id = 'default-legacy-triple' AND indicator = 'mfi' AND side = 'lower'
  );

INSERT INTO scan_preset_conditions (
    id, preset_id, indicator, side, period, threshold, parameters_json,
    is_enabled, sort_order, trigger_mode
)
SELECT 'default-mfi-upper', 'default-legacy-triple', 'mfi', 'upper', 14, 70.0, '{}', 0, 3, 'current'
WHERE EXISTS (SELECT 1 FROM scan_presets WHERE id = 'default-legacy-triple')
  AND NOT EXISTS (
      SELECT 1 FROM scan_preset_conditions
      WHERE preset_id = 'default-legacy-triple' AND indicator = 'mfi' AND side = 'upper'
  );

INSERT INTO scan_preset_conditions (
    id, preset_id, indicator, side, period, threshold, parameters_json,
    is_enabled, sort_order, trigger_mode
)
SELECT 'default-bollinger-lower', 'default-legacy-triple', 'bollinger', 'lower', 20, NULL,
       '{"stdDevMultiplier":1.0}', 1, 4, 'current'
WHERE EXISTS (SELECT 1 FROM scan_presets WHERE id = 'default-legacy-triple')
  AND NOT EXISTS (
      SELECT 1 FROM scan_preset_conditions
      WHERE preset_id = 'default-legacy-triple' AND indicator = 'bollinger' AND side = 'lower'
  );

INSERT INTO scan_preset_conditions (
    id, preset_id, indicator, side, period, threshold, parameters_json,
    is_enabled, sort_order, trigger_mode
)
SELECT 'default-bollinger-upper', 'default-legacy-triple', 'bollinger', 'upper', 20, NULL,
       '{"stdDevMultiplier":1.0}', 0, 5, 'current'
WHERE EXISTS (SELECT 1 FROM scan_presets WHERE id = 'default-legacy-triple')
  AND NOT EXISTS (
      SELECT 1 FROM scan_preset_conditions
      WHERE preset_id = 'default-legacy-triple' AND indicator = 'bollinger' AND side = 'upper'
  );

PRAGMA user_version = 2;
COMMIT;

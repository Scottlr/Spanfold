use serde::{Deserialize, Deserializer};
use spanfold::PrimitiveValue;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EventImportMap {
    input: Option<String>,
    source: FieldSelector,
    key: Option<FieldSelector>,
    position: FieldSelector,
    partition: Option<FieldSelector>,
    windows: Vec<EventWindowMap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventWindowMap {
    name: String,
    key: Option<FieldSelector>,
    active: EventPredicate,
    #[serde(default, deserialize_with = "deserialize_segments")]
    segments: Vec<NamedFieldSelector>,
    #[serde(default, deserialize_with = "deserialize_tags")]
    tags: Vec<NamedFieldSelector>,
}

impl EventImportMap {
    fn compile(self, path: &str) -> Result<CompiledImportMap, String> {
        self.validate(path)?;
        let EventImportMap {
            input,
            source,
            key,
            position,
            partition,
            windows,
        } = self;
        let uses_default_key = windows.iter().any(|window| window.key.is_none());
        let key = if uses_default_key {
            key.map(|selector| selector.compile(path)).transpose()?
        } else {
            None
        };
        Ok(CompiledImportMap {
            input,
            source: source.compile(path)?,
            position: position.compile(path)?,
            partition: partition
                .map(|selector| selector.compile(path))
                .transpose()?,
            windows: windows
                .into_iter()
                .map(|window| window.compile(key.as_ref(), path))
                .collect::<Result<_, _>>()?,
        })
    }

    fn validate(&self, path: &str) -> Result<(), String> {
        if self.windows.is_empty() {
            return Err(format!(
                "{path}: $.windows must contain at least one window"
            ));
        }
        if self.source.field().trim().is_empty() || self.position.field().trim().is_empty() {
            return Err(format!("{path}: source and position fields are required"));
        }
        let mut names = std::collections::BTreeSet::new();
        for window in &self.windows {
            if window.name.trim().is_empty() || !names.insert(window.name.as_str()) {
                return Err(format!("{path}: window names must be non-empty and unique"));
            }
            window.active.validate(path)?;
            for selector in window.segments.iter().chain(window.tags.iter()) {
                if selector.name.trim().is_empty() || selector.selector.field().trim().is_empty() {
                    return Err(format!(
                        "{path}: named selectors require non-empty names and fields"
                    ));
                }
            }
        }
        Ok(())
    }
}

impl EventWindowMap {
    fn compile(
        self,
        default_key: Option<&CompiledFieldSelector>,
        path: &str,
    ) -> Result<CompiledWindowMap, String> {
        Ok(CompiledWindowMap {
            name: self.name,
            key: self
                .key
                .map(|selector| selector.compile(path))
                .transpose()?
                .or_else(|| default_key.cloned()),
            active: self.active.compile(path)?,
            segments: self
                .segments
                .into_iter()
                .map(|selector| selector.compile(path))
                .collect::<Result<_, _>>()?,
            tags: self
                .tags
                .into_iter()
                .map(|selector| selector.compile(path))
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventPredicate {
    field: String,
    equals: Option<PrimitiveValue>,
    #[serde(rename = "notEquals")]
    not_equals: Option<PrimitiveValue>,
    #[serde(rename = "greaterThan")]
    greater_than: Option<PrimitiveValue>,
    #[serde(rename = "greaterThanOrEqual")]
    greater_than_or_equal: Option<PrimitiveValue>,
    #[serde(rename = "lessThan")]
    less_than: Option<PrimitiveValue>,
    #[serde(rename = "lessThanOrEqual")]
    less_than_or_equal: Option<PrimitiveValue>,
    #[serde(rename = "isTrue")]
    is_true: Option<bool>,
    #[serde(rename = "isFalse")]
    is_false: Option<bool>,
}

impl EventPredicate {
    fn validate(&self, path: &str) -> Result<(), String> {
        if self.field.trim().is_empty() {
            return Err(format!("{path}: predicate field cannot be empty"));
        }
        let operators = [
            self.equals.is_some(),
            self.not_equals.is_some(),
            self.greater_than.is_some(),
            self.greater_than_or_equal.is_some(),
            self.less_than.is_some(),
            self.less_than_or_equal.is_some(),
            self.is_true.is_some(),
            self.is_false.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();
        if operators != 1 {
            return Err(format!(
                "{path}: each predicate must declare exactly one operator"
            ));
        }
        Ok(())
    }

    fn compile(self, path: &str) -> Result<CompiledPredicate, String> {
        let field = CompiledFieldSelector::compile(self.field, path)?;
        let operator = if let Some(expected) = self.equals {
            PredicateOperator::Equals(expected)
        } else if let Some(expected) = self.not_equals {
            PredicateOperator::NotEquals(expected)
        } else if let Some(expected) = self.greater_than {
            PredicateOperator::GreaterThan(expected)
        } else if let Some(expected) = self.greater_than_or_equal {
            PredicateOperator::GreaterThanOrEqual(expected)
        } else if let Some(expected) = self.less_than {
            PredicateOperator::LessThan(expected)
        } else if let Some(expected) = self.less_than_or_equal {
            PredicateOperator::LessThanOrEqual(expected)
        } else if let Some(expected) = self.is_true {
            PredicateOperator::IsTrue(expected)
        } else if let Some(expected) = self.is_false {
            PredicateOperator::IsFalse(expected)
        } else {
            unreachable!("validated predicates have exactly one operator")
        };
        Ok(CompiledPredicate { field, operator })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum FieldSelector {
    FieldName(String),
    Field { field: String },
}

impl FieldSelector {
    fn field(&self) -> &str {
        match self {
            Self::FieldName(field) | Self::Field { field } => field,
        }
    }

    fn compile(self, path: &str) -> Result<CompiledFieldSelector, String> {
        let field = match self {
            Self::FieldName(field) | Self::Field { field } => field,
        };
        CompiledFieldSelector::compile(field, path)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNamedFieldSelector {
    name: String,
    field: String,
    #[serde(rename = "parentName")]
    parent_name: Option<String>,
}

#[derive(Clone, Debug)]
struct NamedFieldSelector {
    name: String,
    selector: FieldSelector,
    parent_name: Option<String>,
    kind: &'static str,
}

impl NamedFieldSelector {
    fn compile(self, path: &str) -> Result<CompiledNamedFieldSelector, String> {
        Ok(CompiledNamedFieldSelector {
            name: self.name,
            selector: self.selector.compile(path)?,
            parent_name: self.parent_name,
            kind: self.kind,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledImportMap {
    pub(crate) input: Option<String>,
    pub(crate) source: CompiledFieldSelector,
    pub(crate) position: CompiledFieldSelector,
    pub(crate) partition: Option<CompiledFieldSelector>,
    pub(crate) windows: Vec<CompiledWindowMap>,
}

impl CompiledImportMap {
    pub(crate) fn input(&self) -> &str {
        self.input.as_deref().unwrap_or("jsonl")
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledWindowMap {
    pub(crate) name: String,
    pub(crate) key: Option<CompiledFieldSelector>,
    pub(crate) active: CompiledPredicate,
    pub(crate) segments: Vec<CompiledNamedFieldSelector>,
    pub(crate) tags: Vec<CompiledNamedFieldSelector>,
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledNamedFieldSelector {
    pub(crate) name: String,
    pub(crate) selector: CompiledFieldSelector,
    pub(crate) parent_name: Option<String>,
    pub(crate) kind: &'static str,
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledPredicate {
    field: CompiledFieldSelector,
    operator: PredicateOperator,
}

#[derive(Clone, Debug)]
enum PredicateOperator {
    Equals(PrimitiveValue),
    NotEquals(PrimitiveValue),
    GreaterThan(PrimitiveValue),
    GreaterThanOrEqual(PrimitiveValue),
    LessThan(PrimitiveValue),
    LessThanOrEqual(PrimitiveValue),
    IsTrue(bool),
    IsFalse(bool),
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledFieldSelector {
    field: String,
    parts: Vec<FieldPathPart>,
}

impl CompiledFieldSelector {
    fn compile(field: String, path: &str) -> Result<Self, String> {
        let parts = parse_field_path(&field)
            .map_err(|error| format!("{path}: invalid field '{field}': {error}"))?;
        Ok(Self { field, parts })
    }

    pub(crate) fn field(&self) -> &str {
        &self.field
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FieldPathPart {
    Name(String),
    Index(usize),
}

fn deserialize_segments<'de, D>(deserializer: D) -> Result<Vec<NamedFieldSelector>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_named_selectors(deserializer, "segment")
}

fn deserialize_tags<'de, D>(deserializer: D) -> Result<Vec<NamedFieldSelector>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_named_selectors(deserializer, "tag")
}

fn deserialize_named_selectors<'de, D>(
    deserializer: D,
    kind: &'static str,
) -> Result<Vec<NamedFieldSelector>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<RawNamedFieldSelector>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .map(|selector| NamedFieldSelector {
            name: selector.name,
            selector: FieldSelector::Field {
                field: selector.field,
            },
            parent_name: selector.parent_name,
            kind,
        })
        .collect())
}

fn parse_field_path(field_path: &str) -> Result<Vec<FieldPathPart>, &'static str> {
    if let Some(pointer) = field_path.strip_prefix('/') {
        if pointer.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(pointer
            .split('/')
            .map(|field| FieldPathPart::Name(field.replace("~1", "/").replace("~0", "~")))
            .collect());
    }

    let mut parts = Vec::new();
    let mut name = String::new();
    let mut chars = field_path.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '.' => {
                if name.is_empty() {
                    return Err("empty path segment");
                }
                parts.push(FieldPathPart::Name(std::mem::take(&mut name)));
            }
            '[' => {
                if !name.is_empty() {
                    parts.push(FieldPathPart::Name(std::mem::take(&mut name)));
                }
                let mut index = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == ']' {
                        closed = true;
                        break;
                    }
                    index.push(next);
                }
                if !closed {
                    return Err("unmatched opening bracket");
                }
                if !index.chars().all(|digit| digit.is_ascii_digit()) {
                    return Err("array indexes must be non-negative integers");
                }
                let index = index
                    .parse::<usize>()
                    .map_err(|_| "array index is too large")?;
                parts.push(FieldPathPart::Index(index));
                if chars.peek() == Some(&'.') {
                    chars.next();
                }
            }
            ']' => return Err("unmatched closing bracket"),
            _ => name.push(character),
        }
    }
    if field_path.ends_with('.') {
        return Err("empty path segment");
    }
    if !name.is_empty() {
        parts.push(FieldPathPart::Name(name));
    }
    if parts.is_empty() {
        return Err("field path is empty");
    }
    Ok(parts)
}

pub(crate) fn deserialize_import_map(json: &str) -> Result<EventImportMap, serde_json::Error> {
    serde_json::from_str(json)
}

pub(crate) fn compile_import_map(
    import_map: EventImportMap,
    path: &str,
) -> Result<CompiledImportMap, String> {
    import_map.compile(path)
}

pub(crate) fn select_compiled_field<'a>(
    event: &'a serde_json::Value,
    selector: &CompiledFieldSelector,
    path: &str,
    line_number: usize,
) -> Result<&'a serde_json::Value, String> {
    let mut current = event;
    for part in &selector.parts {
        let next = match part {
            FieldPathPart::Name(field) => current.get(field).or_else(|| {
                current
                    .as_array()
                    .and_then(|_| field.parse::<usize>().ok())
                    .and_then(|index| current.get(index))
            }),
            FieldPathPart::Index(index) => current.get(*index),
        };
        let Some(next) = next else {
            return Err(format!(
                "{path}:{line_number}: missing event field '{}'",
                selector.field
            ));
        };
        current = next;
    }
    Ok(current)
}

#[cfg(test)]
pub(crate) fn select_field<'a>(
    event: &'a serde_json::Value,
    field_path: &str,
    path: &str,
    line_number: usize,
) -> Result<&'a serde_json::Value, String> {
    let parts = parse_field_path(field_path)
        .map_err(|error| format!("{path}:{line_number}: invalid field '{field_path}': {error}"))?;
    let selector = CompiledFieldSelector {
        field: field_path.to_owned(),
        parts,
    };
    select_compiled_field(event, &selector, path, line_number)
}

pub(crate) fn evaluate_predicate(
    event: &serde_json::Value,
    predicate: &CompiledPredicate,
    path: &str,
    line_number: usize,
) -> Result<bool, String> {
    let value = select_compiled_field(event, &predicate.field, path, line_number)?;
    let primitive = primitive_from_json(value).map_err(|error| {
        format!(
            "{path}:{line_number}: predicate field '{}' {error}",
            predicate.field.field
        )
    })?;

    match &predicate.operator {
        PredicateOperator::Equals(expected) => Ok(primitive == *expected),
        PredicateOperator::NotEquals(expected) => Ok(primitive != *expected),
        PredicateOperator::GreaterThan(expected) => {
            compare_numbers(&primitive, expected, |ordering| {
                ordering == std::cmp::Ordering::Greater
            })
            .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))
        }
        PredicateOperator::GreaterThanOrEqual(expected) => {
            compare_numbers(&primitive, expected, |ordering| {
                ordering != std::cmp::Ordering::Less
            })
            .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))
        }
        PredicateOperator::LessThan(expected) => {
            compare_numbers(&primitive, expected, |ordering| {
                ordering == std::cmp::Ordering::Less
            })
            .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))
        }
        PredicateOperator::LessThanOrEqual(expected) => {
            compare_numbers(&primitive, expected, |ordering| {
                ordering != std::cmp::Ordering::Greater
            })
            .ok_or_else(|| numeric_predicate_error(path, line_number, predicate))
        }
        PredicateOperator::IsTrue(true) => Ok(primitive == PrimitiveValue::Bool(true)
            || primitive == PrimitiveValue::String("true".to_owned())),
        PredicateOperator::IsFalse(true) => Ok(primitive == PrimitiveValue::Bool(false)
            || primitive == PrimitiveValue::String("false".to_owned())),
        PredicateOperator::IsTrue(false) | PredicateOperator::IsFalse(false) => Err(format!(
            "{path}:{line_number}: predicate for field '{}' has no condition",
            predicate.field.field
        )),
    }
}

fn numeric_predicate_error(
    path: &str,
    line_number: usize,
    predicate: &CompiledPredicate,
) -> String {
    format!(
        "{path}:{line_number}: predicate field '{}' and threshold must be numeric",
        predicate.field.field
    )
}

fn compare_numbers(
    left: &PrimitiveValue,
    right: &PrimitiveValue,
    compare: impl FnOnce(std::cmp::Ordering) -> bool,
) -> Option<bool> {
    let left = csv_numeric(left)?;
    let right = csv_numeric(right)?;
    let ordering = match (&left, &right) {
        (PrimitiveValue::Integer(left), PrimitiveValue::Integer(right)) => left.cmp(right),
        (PrimitiveValue::Float(left), PrimitiveValue::Float(right)) => {
            left.as_f64().partial_cmp(&right.as_f64())?
        }
        (PrimitiveValue::Integer(left), PrimitiveValue::Float(right)) => {
            if left.unsigned_abs() > (1_u64 << 53) {
                return None;
            }
            (*left as f64).partial_cmp(&right.as_f64())?
        }
        (PrimitiveValue::Float(left), PrimitiveValue::Integer(right)) => {
            if right.unsigned_abs() > (1_u64 << 53) {
                return None;
            }
            left.as_f64().partial_cmp(&(*right as f64))?
        }
        _ => return None,
    };
    Some(compare(ordering))
}

fn csv_numeric(value: &PrimitiveValue) -> Option<PrimitiveValue> {
    match value {
        PrimitiveValue::String(value) => value
            .parse::<i64>()
            .ok()
            .map(PrimitiveValue::Integer)
            .or_else(|| {
                value
                    .parse::<f64>()
                    .ok()
                    .and_then(|number| PrimitiveValue::try_float(number).ok())
            }),
        other => Some(other.clone()),
    }
}

pub(crate) fn primitive_from_json(
    value: &serde_json::Value,
) -> Result<PrimitiveValue, &'static str> {
    match value {
        serde_json::Value::Null => Ok(PrimitiveValue::Null),
        serde_json::Value::Bool(value) => Ok(PrimitiveValue::Bool(*value)),
        serde_json::Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                Ok(PrimitiveValue::Integer(integer))
            } else if let Some(float) = value.as_f64() {
                PrimitiveValue::try_float(float).map_err(|_| "must be a finite JSON number")
            } else {
                Err("must be a finite JSON number")
            }
        }
        serde_json::Value::String(value) => Ok(PrimitiveValue::String(value.clone())),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err("must be a scalar JSON value")
        }
    }
}

pub(crate) fn primitive_to_string(value: &serde_json::Value) -> Option<String> {
    match primitive_from_json(value).ok()? {
        PrimitiveValue::String(value) => Some(value),
        PrimitiveValue::Integer(value) => Some(value.to_string()),
        PrimitiveValue::Float(value) => Some(value.as_f64().to_string()),
        PrimitiveValue::Bool(value) => Some(value.to_string()),
        PrimitiveValue::Null => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::{
        ImportError, ImportRecorder, close_remaining_imported_windows, process_import_event,
    };

    fn compile_map(json: serde_json::Value) -> Result<CompiledImportMap, String> {
        serde_json::from_value::<EventImportMap>(json)
            .expect("import map schema")
            .compile("map.json")
    }

    fn compile_predicate(json: serde_json::Value) -> Result<CompiledPredicate, String> {
        let predicate = serde_json::from_value::<EventPredicate>(json).expect("predicate schema");
        predicate.validate("map.json")?;
        predicate.compile("map.json")
    }

    #[test]
    fn compiled_import_map_selects_nested_bracket_pointer_and_numeric_name_paths() {
        let import_map = compile_map(serde_json::json!({
            "source": { "field": "/metadata/source" },
            "key": "identity.0",
            "position": "rows[0].position",
            "partition": "metadata.partition",
            "windows": [{
                "name": "Online",
                "active": { "field": "states[0].active", "isTrue": true },
                "segments": [{ "name": "escaped", "field": "/a~1b/~0key", "parentName": "root" }],
                "tags": [{ "name": "region", "field": "metadata.region" }]
            }]
        }))
        .expect("compiled map");
        let event = serde_json::json!({
            "metadata": { "source": "provider-a", "partition": "p1", "region": "eu" },
            "identity": ["device-1"],
            "rows": [{ "position": 7 }],
            "states": [{ "active": true }],
            "a/b": { "~key": 42 }
        });
        let mut lifecycle = ImportRecorder::new();
        let mut windows = Vec::new();

        assert!(
            process_import_event(
                &event,
                &import_map,
                "events.jsonl",
                1,
                &mut lifecycle,
                &mut windows,
            )
            .is_ok()
        );
        assert!(close_remaining_imported_windows(lifecycle, &mut windows).is_ok());

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].key, "device-1");
        assert_eq!(windows[0].source, "provider-a");
        assert_eq!(windows[0].partition.as_deref(), Some("p1"));
        assert_eq!(windows[0].segments[0].value, PrimitiveValue::Integer(42));
        assert_eq!(windows[0].segments[0].parent_name.as_deref(), Some("root"));
        assert_eq!(
            windows[0].tags[0].value,
            PrimitiveValue::String("eu".to_owned())
        );
    }

    #[test]
    fn recorder_import_rejects_backwards_positions() {
        let import_map = compile_map(serde_json::json!({
            "source": "source",
            "key": "key",
            "position": "position",
            "windows": [{
                "name": "Online",
                "active": { "field": "active", "isTrue": true }
            }]
        }))
        .expect("compiled map");
        let mut lifecycle = ImportRecorder::new();
        let mut windows = Vec::new();
        let first = serde_json::json!({
            "source": "provider-a",
            "key": "device-1",
            "position": 2,
            "active": true
        });
        process_import_event(
            &first,
            &import_map,
            "events.jsonl",
            1,
            &mut lifecycle,
            &mut windows,
        )
        .expect("first position");

        let second = serde_json::json!({
            "source": "provider-a",
            "key": "device-1",
            "position": 1,
            "active": false
        });
        let error = process_import_event(
            &second,
            &import_map,
            "events.jsonl",
            2,
            &mut lifecycle,
            &mut windows,
        )
        .expect_err("backwards position");
        assert!(matches!(
            error,
            ImportError::Input(message)
                if message == "import-events: events.jsonl:2: event position cannot move backwards"
        ));
    }

    #[test]
    fn recorder_import_drops_metadata_after_closing_a_window() {
        let import_map = compile_map(serde_json::json!({
            "source": "source",
            "key": "key",
            "position": "position",
            "windows": [{
                "name": "Online",
                "active": { "field": "active", "isTrue": true }
            }]
        }))
        .expect("compiled map");
        let mut lifecycle = ImportRecorder::new();
        let mut windows = Vec::new();
        let active = serde_json::json!({
            "source": "provider-a",
            "key": "device-1",
            "position": 1,
            "active": true
        });
        process_import_event(
            &active,
            &import_map,
            "events.jsonl",
            1,
            &mut lifecycle,
            &mut windows,
        )
        .expect("active position");
        assert_eq!(lifecycle.metadata.len(), 1);

        let inactive = serde_json::json!({
            "source": "provider-a",
            "key": "device-1",
            "position": 2,
            "active": false
        });
        process_import_event(
            &inactive,
            &import_map,
            "events.jsonl",
            2,
            &mut lifecycle,
            &mut windows,
        )
        .expect("inactive position");

        assert!(lifecycle.metadata.is_empty());
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].end_position, Some(2));
    }

    #[test]
    fn compiled_predicates_preserve_operator_semantics() {
        let cases = [
            (
                serde_json::json!({ "field": "x", "equals": 4 }),
                serde_json::json!(4),
            ),
            (
                serde_json::json!({ "field": "x", "notEquals": 3 }),
                serde_json::json!(4),
            ),
            (
                serde_json::json!({ "field": "x", "greaterThan": 3 }),
                serde_json::json!(4),
            ),
            (
                serde_json::json!({ "field": "x", "greaterThanOrEqual": 4 }),
                serde_json::json!(4),
            ),
            (
                serde_json::json!({ "field": "x", "lessThan": 5 }),
                serde_json::json!(4),
            ),
            (
                serde_json::json!({ "field": "x", "lessThanOrEqual": 4 }),
                serde_json::json!(4),
            ),
            (
                serde_json::json!({ "field": "x", "isTrue": true }),
                serde_json::json!(true),
            ),
            (
                serde_json::json!({ "field": "x", "isFalse": true }),
                serde_json::json!(false),
            ),
        ];

        for (predicate, value) in cases {
            let predicate = compile_predicate(predicate).expect("compiled predicate");
            assert_eq!(
                evaluate_predicate(
                    &serde_json::json!({ "x": value }),
                    &predicate,
                    "events.jsonl",
                    2,
                ),
                Ok(true)
            );
        }
    }

    #[test]
    fn compiled_numeric_predicate_preserves_precision_failure() {
        let predicate =
            compile_predicate(serde_json::json!({ "field": "value", "greaterThan": 1.5 }))
                .expect("compiled predicate");

        assert_eq!(
            evaluate_predicate(
                &serde_json::json!({ "value": 9_007_199_254_740_993_i64 }),
                &predicate,
                "events.jsonl",
                3,
            ),
            Err("events.jsonl:3: predicate field 'value' and threshold must be numeric".to_owned())
        );
    }

    #[test]
    fn map_compilation_rejects_invalid_field_path_with_map_and_original_field() {
        let error = compile_map(serde_json::json!({
            "source": "metadata[source",
            "position": "position",
            "windows": [{
                "name": "Online",
                "key": "key",
                "active": { "field": "active", "isTrue": true }
            }]
        }))
        .expect_err("invalid selector");

        assert_eq!(
            error,
            "map.json: invalid field 'metadata[source': unmatched opening bracket"
        );
    }

    #[test]
    fn map_compilation_ignores_invalid_global_key_shadowed_by_window_keys() {
        let import_map = compile_map(serde_json::json!({
            "source": "source",
            "key": "invalid[global",
            "position": "position",
            "windows": [{
                "name": "Online",
                "key": "windowKey",
                "active": { "field": "active", "isTrue": true }
            }]
        }))
        .expect("shadowed global key is unused");

        assert_eq!(
            import_map.windows[0]
                .key
                .as_ref()
                .map(CompiledFieldSelector::field),
            Some("windowKey")
        );
    }

    #[test]
    fn compiled_boolean_false_preserves_has_no_condition_error() {
        let predicate =
            compile_predicate(serde_json::json!({ "field": "active", "isTrue": false }))
                .expect("present operator validates");

        assert_eq!(
            evaluate_predicate(
                &serde_json::json!({ "active": true }),
                &predicate,
                "events.jsonl",
                4,
            ),
            Err("events.jsonl:4: predicate for field 'active' has no condition".to_owned())
        );
    }

    #[test]
    fn predicate_compilation_still_requires_exactly_one_operator() {
        let error = compile_predicate(serde_json::json!({
            "field": "value",
            "equals": 1,
            "notEquals": 2
        }))
        .expect_err("two operators");

        assert_eq!(
            error,
            "map.json: each predicate must declare exactly one operator"
        );
    }
}

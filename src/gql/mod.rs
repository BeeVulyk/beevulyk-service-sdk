use crate::external::async_graphql::{
    InputValueError, InputValueResult, Scalar, ScalarType, Value,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct GraphQlDate(pub i64);

impl GraphQlDate {
    #[inline]
    pub fn as_millis(&self) -> i64 {
        self.0
    }
}

fn parse_to_millis(value: &Value) -> Result<i64, InputValueError<GraphQlDate>> {
    match value {
        Value::String(s) => {
            if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                return Ok(dt.with_timezone(&Utc).timestamp_millis());
            }
            if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.3fZ")
            {
                let dt = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
                return Ok(dt.timestamp_millis());
            }
            if let Ok(naive) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                let dt = naive.and_hms_opt(0, 0, 0).unwrap();
                let dt_utc = DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc);
                return Ok(dt_utc.timestamp_millis());
            }
            Err(InputValueError::<GraphQlDate>::custom(format!(
                "invalid date string: expected ISO 8601 or YYYY-MM-DD, got {:?}",
                s
            )))
        }
        Value::Number(n) => {
            let ms = if let Some(i) = n.as_i64() {
                if i < 1_000_000_000_000 && i > -1_000_000_000_000 {
                    i * 1000
                } else {
                    i
                }
            } else if let Some(f) = n.as_f64() {
                let truncated = f.trunc();
                if truncated < 1_000_000_000_000.0 && truncated > -1_000_000_000_000.0 {
                    (truncated as i64) * 1000
                } else {
                    truncated as i64
                }
            } else {
                return Err(InputValueError::<GraphQlDate>::expected_type(value.clone()));
            };
            Ok(ms)
        }
        _ => Err(InputValueError::<GraphQlDate>::expected_type(value.clone())),
    }
}

impl<'de> Deserialize<'de> for GraphQlDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};
        use std::fmt;

        struct GraphQlDateVisitor;

        impl<'de> Visitor<'de> for GraphQlDateVisitor {
            type Value = i64;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a date as ISO 8601 string or number (seconds or milliseconds)")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                if let Ok(dt) = DateTime::parse_from_rfc3339(v) {
                    return Ok(dt.with_timezone(&Utc).timestamp_millis());
                }
                if let Ok(naive) =
                    chrono::NaiveDateTime::parse_from_str(v, "%Y-%m-%dT%H:%M:%S%.3fZ")
                {
                    let dt = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
                    return Ok(dt.timestamp_millis());
                }
                if let Ok(naive) = chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d") {
                    let dt = naive.and_hms_opt(0, 0, 0).unwrap();
                    let dt_utc = DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc);
                    return Ok(dt_utc.timestamp_millis());
                }
                Err(de::Error::custom(format!(
                    "invalid date string: expected ISO 8601 or YYYY-MM-DD, got {:?}",
                    v
                )))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(if v < 1_000_000_000_000 && v > -1_000_000_000_000 {
                    v * 1000
                } else {
                    v
                })
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                if v < 1_000_000_000_000 {
                    Ok((v as i64) * 1000)
                } else if v > i64::MAX as u64 {
                    Err(de::Error::custom("timestamp out of range: value exceeds i64::MAX"))
                } else {
                    Ok(v as i64)
                }
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                let truncated = v.trunc() as i64;
                Ok(if truncated < 1_000_000_000_000 && truncated > -1_000_000_000_000 {
                    truncated * 1000
                } else {
                    truncated
                })
            }
        }

        let ms = deserializer.deserialize_any(GraphQlDateVisitor)?;
        Ok(GraphQlDate(ms))
    }
}

#[Scalar]
impl ScalarType for GraphQlDate {
    fn parse(value: Value) -> InputValueResult<Self> {
        parse_to_millis(&value).map(GraphQlDate)
    }

    fn to_value(&self) -> Value {
        Value::Number(
            crate::external::async_graphql::Number::from_i128(self.0 as i128)
                .expect("i64 fits in Number"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn deserialize_from_rfc3339_string() {
        let json = r#""2025-02-18T12:00:00Z""#;
        let d: GraphQlDate = serde_json::from_str(json).unwrap();
        // 13-digit milliseconds (exact value depends on chrono/UTC handling)
        assert!(d.as_millis() >= 1739_000_000_000 && d.as_millis() <= 1740_000_000_000);
    }

    #[test]
    fn deserialize_from_date_only_string() {
        let json = r#""2025-02-18""#;
        let d: GraphQlDate = serde_json::from_str(json).unwrap();
        // Midnight on that day in some interpretation; 13-digit ms in 2025
        assert!(d.as_millis() >= 1739_000_000_000 && d.as_millis() <= 1740_000_000_000);
    }

    #[test]
    fn deserialize_from_seconds_number() {
        let json = "1739872800";
        let d: GraphQlDate = serde_json::from_str(json).unwrap();
        assert_eq!(d.as_millis(), 1739872800000);
    }

    #[test]
    fn deserialize_from_milliseconds_number() {
        let json = "1739872800000";
        let d: GraphQlDate = serde_json::from_str(json).unwrap();
        assert_eq!(d.as_millis(), 1739872800000);
    }

    #[test]
    fn as_millis_returns_inner_value() {
        let d = GraphQlDate(1234567890123);
        assert_eq!(d.as_millis(), 1234567890123);
    }

    #[test]
    fn serialize_round_trip() {
        let d = GraphQlDate(1739872800000);
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, "1739872800000");
        let d2: GraphQlDate = serde_json::from_str(&json).unwrap();
        assert_eq!(d.as_millis(), d2.as_millis());
    }

    #[test]
    fn option_some_deserialize() {
        #[derive(Deserialize)]
        struct Wrapper {
            at: Option<GraphQlDate>,
        }
        let json = r#"{"at": "2025-02-18T12:00:00Z"}"#;
        let w: Wrapper = serde_json::from_str(json).unwrap();
        let ms = w.at.unwrap().as_millis();
        assert!(ms >= 1739_000_000_000 && ms <= 1740_000_000_000);
    }

    #[test]
    fn option_none_deserialize() {
        #[derive(Deserialize)]
        struct Wrapper {
            at: Option<GraphQlDate>,
        }
        let json = r#"{"at": null}"#;
        let w: Wrapper = serde_json::from_str(json).unwrap();
        assert!(w.at.is_none());
    }

    #[test]
    fn option_missing_field_deserialize() {
        #[derive(Deserialize)]
        struct Wrapper {
            at: Option<GraphQlDate>,
        }
        let json = "{}";
        let w: Wrapper = serde_json::from_str(json).unwrap();
        assert!(w.at.is_none());
    }

    #[test]
    fn deserialize_invalid_string_fails() {
        let json = r#""not-a-date""#;
        let res: Result<GraphQlDate, _> = serde_json::from_str(json);
        assert!(res.is_err());
    }

    #[test]
    fn deserialize_u64_above_i64_max_fails() {
        // i64::MAX + 1; casting to i64 would wrap to negative
        let json = "9223372036854775808";
        let res: Result<GraphQlDate, _> = serde_json::from_str(json);
        assert!(res.is_err());
    }
}

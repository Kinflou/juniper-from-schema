#[allow(unused_imports)]
use crate::{FromDefaultScalarValue, FromLookAheadValue};
#[allow(unused_imports)]
use juniper::{ID, DefaultScalarValue, LookAheadValue};


impl<'a, 'b> FromDefaultScalarValue<i32> for &'a &'b DefaultScalarValue {
    fn from(self) -> i32 {
        let warn = |kind| {
            panic!("Failed converting scalar value. Expected `#to` got `{}`", kind);
        };
        match self {
            DefaultScalarValue::Int(x) => x.to_owned(),
            other => {
                match other {
                    DefaultScalarValue::Int(_) => warn("Int"),
                    DefaultScalarValue::String(_) => warn("String"),
                    DefaultScalarValue::Float(_) => warn("Float"),
                    DefaultScalarValue::Boolean(_) => warn("Boolean"),
                }
            }
        }
    }
}

impl<'a, 'b> FromDefaultScalarValue<String> for &'a &'b DefaultScalarValue {
    fn from(self) -> String {
        let warn = |kind| {
            panic!("Failed converting scalar value. Expected `#to` got `{}`", kind);
        };
        match self {
            DefaultScalarValue::String(x) => x.to_owned(),
            other => {
                match other {
                    DefaultScalarValue::Int(_) => warn("Int"),
                    DefaultScalarValue::String(_) => warn("String"),
                    DefaultScalarValue::Float(_) => warn("Float"),
                    DefaultScalarValue::Boolean(_) => warn("Boolean"),
                }
            }
        }
    }
}

impl<'a, 'b> FromDefaultScalarValue<f64> for &'a &'b DefaultScalarValue {
    fn from(self) -> f64 {
        let warn = |kind| {
            panic!("Failed converting scalar value. Expected `#to` got `{}`", kind);
        };
        match self {
            DefaultScalarValue::Float(x) => x.to_owned(),
            other => {
                match other {
                    DefaultScalarValue::Int(_) => warn("Int"),
                    DefaultScalarValue::String(_) => warn("String"),
                    DefaultScalarValue::Float(_) => warn("Float"),
                    DefaultScalarValue::Boolean(_) => warn("Boolean"),
                }
            }
        }
    }
}

impl<'a, 'b> FromDefaultScalarValue<bool> for &'a &'b DefaultScalarValue {
    fn from(self) -> bool {
        let warn = |kind| {
            panic!("Failed converting scalar value. Expected `#to` got `{}`", kind);
        };
        match self {
            DefaultScalarValue::Boolean(x) => x.to_owned(),
            other => {
                match other {
                    DefaultScalarValue::Int(_) => warn("Int"),
                    DefaultScalarValue::String(_) => warn("String"),
                    DefaultScalarValue::Float(_) => warn("Float"),
                    DefaultScalarValue::Boolean(_) => warn("Boolean"),
                }
            }
        }
    }
}

impl<'a, 'b, T> FromDefaultScalarValue<Option<T>> for &'a &'b DefaultScalarValue
    where &'a &'b DefaultScalarValue: FromDefaultScalarValue<T>,
{
    fn from(self) -> Option<T> {
        Some(self.from())
    }
}

impl<'a, 'b> FromLookAheadValue<i32> for &'a LookAheadValue<'b, DefaultScalarValue> {
    fn from(self) -> i32 {
        let warn = |kind| {
            panic!(
                "Failed converting look ahead value. Expected scalar type got `{kind}`"
            )
        };
        match self {
            LookAheadValue::Scalar(scalar) => FromDefaultScalarValue::from(scalar),
            LookAheadValue::Null => warn("null"),
            LookAheadValue::Enum(_) => warn("enum"),
            LookAheadValue::List(_) => warn("list"),
            LookAheadValue::Object(_) => warn("object"),
        }
    }
}

impl<'a, 'b> FromLookAheadValue<String> for &'a LookAheadValue<'b, DefaultScalarValue> {
    fn from(self) -> String {
        let warn = |kind| {
            panic!(
                "Failed converting look ahead value. Expected scalar type got `{kind}`"
            )
        };
        match self {
            LookAheadValue::Scalar(scalar) => FromDefaultScalarValue::from(scalar),
            LookAheadValue::Null => warn("null"),
            LookAheadValue::Enum(_) => warn("enum"),
            LookAheadValue::List(_) => warn("list"),
            LookAheadValue::Object(_) => warn("object"),
        }
    }
}

impl<'a, 'b> FromLookAheadValue<f64> for &'a LookAheadValue<'b, DefaultScalarValue> {
    fn from(self) -> f64 {
        let warn = |kind| {
            panic!(
                "Failed converting look ahead value. Expected scalar type got `{kind}`"
            )
        };
        match self {
            LookAheadValue::Scalar(scalar) => FromDefaultScalarValue::from(scalar),
            LookAheadValue::Null => warn("null"),
            LookAheadValue::Enum(_) => warn("enum"),
            LookAheadValue::List(_) => warn("list"),
            LookAheadValue::Object(_) => warn("object"),
        }
    }
}

impl<'a, 'b> FromLookAheadValue<bool> for &'a LookAheadValue<'b, DefaultScalarValue> {
    fn from(self) -> bool {
        let warn = |kind| {
            panic!(
                "Failed converting look ahead value. Expected scalar type got `{kind}`"
            )
        };
        match self {
            LookAheadValue::Scalar(scalar) => FromDefaultScalarValue::from(scalar),
            LookAheadValue::Null => warn("null"),
            LookAheadValue::Enum(_) => warn("enum"),
            LookAheadValue::List(_) => warn("list"),
            LookAheadValue::Object(_) => warn("object"),
        }
    }
}

impl<'a, 'b, T> FromLookAheadValue<Option<T>> for &'a LookAheadValue<'b, DefaultScalarValue>
    where &'a LookAheadValue<'b, DefaultScalarValue>: FromLookAheadValue<T>,
{
    fn from(self) -> Option<T> {
        match self {
            LookAheadValue::Null => None,
            other => Some(other.from()),
        }
    }
}

impl<'a, 'b, T> FromLookAheadValue<Vec<T>> for &'a LookAheadValue<'b, DefaultScalarValue>
    where &'a LookAheadValue<'b, DefaultScalarValue>: FromLookAheadValue<T>,
{
    fn from(self) -> Vec<T> {
        let warn = |kind| {
            panic!(
                "Failed converting look ahead value. Expected scalar type got `{kind}`"
            )
        };
        match self {
            LookAheadValue::List(values) => {
                values.iter().map(|value| value.item.from()).collect::<Vec<_>>()
            }
            LookAheadValue::Scalar(_) => warn("scalar"),
            LookAheadValue::Null => warn("null"),
            LookAheadValue::Enum(_) => warn("enum"),
            LookAheadValue::Object(_) => warn("object"),
        }
    }
}
impl<'a, 'b> FromLookAheadValue<ID> for &'a LookAheadValue<'b, DefaultScalarValue> {
    fn from(self) -> ID {
        let s = FromLookAheadValue::<String>::from(self);
        ID::new(s)
    }
}

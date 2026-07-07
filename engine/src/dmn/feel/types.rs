//! FEEL type lattice nodes and carrier structs — skeleton declarations.
//!
//! Plan 30-01 lands the `FeelType` enum, `FeelRange`, and `FeelFunction`
//! placeholder carriers. The lattice relations (`conforms`, `equivalent`,
//! `implicit_convert`, `resolve_type_ref`) land in Plan 04.
//!
//! The lattice is modelled as a closed enum + free functions (per
//! 30-RESEARCH.md Pattern 2), not a trait graph: FEEL's type space is small
//! and closed, so a table/function is clearer and property-testable.

use crate::data_types::Value;
use serde::{Deserialize, Serialize};

/// The FEEL type lattice nodes (DMN 1.5 §10.3.2.9).
///
/// `Null` and `Any` are the lattice bounds: `Null <: T <: Any` for every `T`.
/// `List`/`Context`/`Range` are recursive (carry inner `FeelType`s); `Function`
/// carries parameter and return types. The closure *body* of a FEEL function
/// value is a Phase 31 concern — here we carry only the type shape, enough for
/// `conforms`/`equivalent` (research Open Question 3 resolution).
///
/// `Serialize`/`Deserialize` are derived because [`FeelFunction`] (which holds
/// `Vec<FeelType>` / `Box<FeelType>`) derives serde and the derive propagates
/// the bound to the inner type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeelType {
    /// The null type — bottom of the lattice. `Null <: T` for every `T`.
    Null,
    /// The top of the lattice. `T <: Any` for every `T`.
    Any,
    /// FEEL `number` (Decimal128). Backed by `rust_decimal::Decimal`.
    Number,
    /// FEEL `string`.
    String,
    /// FEEL `boolean`.
    Boolean,
    /// FEEL `date`.
    Date,
    /// FEEL `time`.
    Time,
    /// FEEL `date and time`.
    DateTime,
    /// FEEL `days and time duration` (ISO 8601 `PnYnMnDTnHnMnS` day-time part).
    DaysTimeDuration,
    /// FEEL `years and months duration` (ISO 8601 `PnYnM` year-month part).
    YearsMonthsDuration,
    /// FEEL `list<T>`.
    List(Box<FeelType>),
    /// FEEL `context<k1:T1..kn:Tn>` — an ordered list of `(key, type)` pairs.
    Context(Vec<(String, FeelType)>),
    /// FEEL `range<T>`.
    Range(Box<FeelType>),
    /// FEEL `function (T1..Tn) -> U`. Contravariant params, covariant return.
    Function {
        /// Parameter types, positional.
        params: Vec<FeelType>,
        /// Return type.
        ret: Box<FeelType>,
    },
}

/// FEEL range value carrier — the payload of [`Value::Range`].
///
/// `start`/`end` are `Option<Value>` so an open endpoint (`None`) means
/// "unbounded" on that side (FEEL `..]` / `[..`). `start_incl`/`end_incl`
/// carry the bracket sense: `true` = inclusive (`[`/`]`), `false` = exclusive
/// `(`/`)`.
///
/// `Value::Range(Box<FeelRange>)` boxes the range because `FeelRange` holds
/// `Option<Value>` and `Value` holds `Box<FeelRange>` — the `Box` breaks the
/// infinite size cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeelRange {
    /// Start endpoint, or `None` if unbounded below.
    pub start: Option<Value>,
    /// End endpoint, or `None` if unbounded above.
    pub end: Option<Value>,
    /// Whether the start endpoint is inclusive.
    pub start_incl: bool,
    /// Whether the end endpoint is inclusive.
    pub end_incl: bool,
}

/// FEEL function value carrier — the payload of [`Value::Function`].
///
/// Plan 30-01 ships a *placeholder* carrying only arity + parameter/return
/// `FeelType`s — enough for the lattice `conforms`/`equivalent` relations.
/// The closure body (the `FeelExpression` AST + environment capture) is
/// Phase 31 (research Open Question 3 resolution).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeelFunction {
    /// Number of parameters the function accepts.
    pub arity: usize,
    /// Parameter types, positional.
    pub param_types: Vec<FeelType>,
    /// Return type.
    pub ret_type: Box<FeelType>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_types::Value;

    #[test]
    fn feeltype_number_variant_constructs() {
        let _t = FeelType::Number;
    }

    #[test]
    fn feeltype_list_and_function_construct() {
        let list = FeelType::List(Box::new(FeelType::Number));
        let f = FeelType::Function {
            params: vec![FeelType::Number],
            ret: Box::new(FeelType::Boolean),
        };
        // Exercise Debug/Clone/PartialEq derives.
        assert_eq!(list.clone(), FeelType::List(Box::new(FeelType::Number)));
        // Access the enum variant's fields via a pattern match.
        let FeelType::Function { params, ret } = &f else {
            panic!("expected FeelType::Function");
        };
        assert_eq!(params.len(), 1);
        assert_eq!(**ret, FeelType::Boolean);
    }

    #[test]
    fn feel_range_carrier_round_trips() {
        let r = FeelRange {
            start: Some(Value::Null),
            end: None,
            start_incl: true,
            end_incl: false,
        };
        assert!(r.start_incl);
        assert!(r.end.is_none());
        let cloned = r.clone();
        assert_eq!(r, cloned);
    }

    #[test]
    fn feel_function_carrier_constructs() {
        let f = FeelFunction {
            arity: 1,
            param_types: vec![FeelType::Number],
            ret_type: Box::new(FeelType::Boolean),
        };
        assert_eq!(f.arity, 1);
        assert_eq!(f, f.clone());
    }

    #[test]
    fn feeltype_has_all_lattice_nodes() {
        // Touch every variant so a future removal/rename surfaces here.
        let _ = FeelType::Null;
        let _ = FeelType::Any;
        let _ = FeelType::Number;
        let _ = FeelType::String;
        let _ = FeelType::Boolean;
        let _ = FeelType::Date;
        let _ = FeelType::Time;
        let _ = FeelType::DateTime;
        let _ = FeelType::DaysTimeDuration;
        let _ = FeelType::YearsMonthsDuration;
        let _ = FeelType::List(Box::new(FeelType::Any));
        let _ = FeelType::Context(vec![("k".to_string(), FeelType::Any)]);
        let _ = FeelType::Range(Box::new(FeelType::Any));
        let _ = FeelType::Function {
            params: vec![],
            ret: Box::new(FeelType::Any),
        };
    }
}
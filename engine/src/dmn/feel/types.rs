//! FEEL type lattice nodes and carrier structs — skeleton declarations.
//!
//! Plan 30-01 lands the `FeelType` enum, `FeelRange`, and `FeelFunction`
//! placeholder carriers. The lattice relations (`conforms`, `equivalent`,
//! `implicit_convert`, `resolve_type_ref`) land in Plan 04.

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
        assert_eq!(f.params.len(), 1);
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
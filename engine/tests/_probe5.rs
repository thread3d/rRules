use std::collections::HashMap;
use engine::data_types::Value;
use engine::dmn::feel::cache::FeelAstCache;
use engine::dmn::parser;
use engine::dmn::evaluator::{DecisionEvaluator, EvaluateRequest, SharedDmnModel};
#[test]
fn probe() {
    let dmn = std::fs::read_to_string("tests/fixtures/tck/0006-simpletable-P1/0006-simpletable-P1.dmn").unwrap();
    let model = parser::parse_dmn(&dmn).expect("parse");
    let cache = FeelAstCache::new();
    let shared = std::sync::Arc::new(std::sync::RwLock::new(SharedDmnModel::new(model, cache)));
    let ev = DecisionEvaluator::new(shared);
    let mut inputs = HashMap::new();
    inputs.insert("Age".to_string(), Value::Integer(20));
    inputs.insert("RiskCategory".to_string(), Value::String("Medium".into()));
    inputs.insert("isAffordable".to_string(), Value::Bool(true));
    println!("{:?}", ev.evaluate("_3b2953a3-745f-4d2e-b55d-75c8c5ae653c", EvaluateRequest { inputs, trace: false }));
}

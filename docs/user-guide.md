# rRules User Guide - Writing Rules

This guide shows you how to write rules for the rRules Business Rules Management System, from simple patterns to complex scenarios.

## Table of Contents

1. [Basic Rule Structure](#basic-rule-structure)
2. [Simple Pattern Matching](#simple-pattern-matching)
3. [Conditional Logic](#conditional-logic)
4. [Actions and Effects](#actions-and-effects)
5. [Rule Priority (Salience)](#rule-priority-salience)
6. [Complex Scenarios](#complex-scenarios)
7. [DMN decisions](#dmn-decisions)
8. [Governance: approval before a rule goes live](#governance-approval-before-a-rule-goes-live)
9. [Testing Your Rules](#testing-your-rules)
10. [Best Practices](#best-practices)

---

## What a rule *is*, before any syntax

Two things in this system are called "a rule", and only one of them is executed:

| | Rule **document** | Rule the engine **executes** |
|---|---|---|
| Created by | the interface, `POST /v1/rules` | written into `RULE_STORAGE_PATH` |
| Carries | name, description, category, tags, status, body text | a condition over facts and the actions to take |
| Runs | **no**, unless the create request carried a structured `rule` | yes, via `POST /v1/execute` |

The **New Rule** form in the interface sends the structured form: you choose a fact type, a field, an
operator and a value for the condition, and a log message for the action, and the API writes the engine's
own rule into the rules directory — so the rule is executable after the next reload. The body text shown
afterwards is a rendering of that same rule, not a separate definition.

Two consequences worth knowing:

- a rule written by hand into the rules directory is executed directly; that is the format the sections
  below describe;
- a rule created **before** the form sent structured rules has no executable form — the API reports it
  `executable: false`. Re-create it through the form to make it runnable.

`specs/rule-sources.md` states the whole picture, including what is still missing.

---

## Basic Rule Structure

A rule has two parts:
- **Condition (LHS)**: When this is true, the rule fires
- **Actions (RHS)**: What happens when the rule fires

### Syntax

```rust
use engine::{Action, ComparisonOp, Condition, LogLevel, Rule, Value};

// What the sketch below stands for: one condition, one action.
let condition = Condition::Pattern {
    fact_type: "customer".to_string(),
    field: "status".to_string(),
    value: Value::String("premium".to_string()),
    op: ComparisonOp::Eq,
};
let action = Action::LogMessage {
    level: LogLevel::Info,
    message: "rule fired".to_string(),
};

Rule::builder("rule-id", "Human Readable Name")
    .salience(10)           // Optional: priority (-1000 to 1000)
    .no_loop(false)         // Optional: prevent re-firing
    .lock_on_active(false)  // Optional: block other rules in same group
    .condition(condition)   // The condition that must be met
    .action(action)         // Action(s) to execute
    .build();
```

---

## Simple Pattern Matching

### Example 1: Basic Fact Detection

**Scenario**: Detect when a customer has high status.

**Facts**:
```json
{
  "type": "customer",
  "fields": {
    "status": "premium"
  }
}
```

**Rule**:
```rust
use engine::{Action, ComparisonOp, Condition, LogLevel, Rule, Value};

let rule = Rule::builder("high-status-customer", "High Status Customer")
    .salience(10)
    .condition(Condition::Pattern {
        fact_type: "customer".to_string(),
        field: "status".to_string(),
        value: Value::String("premium".to_string()),
        op: ComparisonOp::Eq,
    })
    .action(Action::LogMessage {
        level: LogLevel::Info,
        message: "High status customer detected".to_string(),
    })
    .build();
```

### Example 2: Numeric Comparison

**Scenario**: Flag transactions over $10,000 for review.

```rust
use engine::{Action, ComparisonOp, Condition, LogLevel, Rule, Value};

let rule = Rule::builder("large-transaction", "Large Transaction")
    .salience(20)
    .condition(Condition::Compare {
        left: Box::new(Condition::Pattern {
            fact_type: "transaction".to_string(),
            field: "amount".to_string(),
            value: Value::Integer(10000),
            op: ComparisonOp::Eq,
        }),
        op: ComparisonOp::Gt,
        right: Box::new(Condition::True),  // Compare against literal
    })
    .action(Action::LogMessage {
        level: LogLevel::Warning,
        message: "Transaction over $10,000".to_string(),
    })
    .build();
```

---

## Conditional Logic

### AND Conditions

Multiple conditions that must ALL be true:

```rust
use engine::{Action, ComparisonOp, Condition, LogLevel, Rule, Value};

let rule = Rule::builder("vip-discount", "VIP Discount")
    .salience(15)
    .condition(Condition::And(vec![
        // Customer has premium status
        Condition::Pattern {
            fact_type: "customer".to_string(),
            field: "status".to_string(),
            value: Value::String("premium".to_string()),
            op: ComparisonOp::Eq,
        },
        // Order amount is over $100
        Condition::Compare {
            left: Box::new(Condition::Pattern {
                fact_type: "order".to_string(),
                field: "amount".to_string(),
                value: Value::Integer(100),
                op: ComparisonOp::Eq,
            }),
            op: ComparisonOp::Gt,
            right: Box::new(Condition::True),
        },
    ]))
    .action(Action::LogMessage {
        level: LogLevel::Info,
        message: "VIP customer eligible for discount".to_string(),
    })
    .build();
```

### OR Conditions

At least ONE condition must be true:

```rust
use engine::{Action, ComparisonOp, Condition, LogLevel, Rule, Value};

let rule = Rule::builder("rush-hour-fee", "Rush Hour Surcharge")
    .salience(5)
    .condition(Condition::Or(vec![
        // Hour is 7-9 AM
        Condition::Pattern {
            fact_type: "time".to_string(),
            field: "hour".to_string(),
            value: Value::Integer(8),  // Simplified
            op: ComparisonOp::Eq,
        },
        // Hour is 5-7 PM
        Condition::Pattern {
            fact_type: "time".to_string(),
            field: "hour".to_string(),
            value: Value::Integer(18),  // Simplified
            op: ComparisonOp::Eq,
        },
    ]))
    .action(Action::LogMessage {
        level: LogLevel::Info,
        message: "Rush hour surcharge applies".to_string(),
    })
    .build();
```

### NOT Conditions

Negate a condition:

```rust
use engine::{Action, ComparisonOp, Condition, LogLevel, Rule, Value};

let rule = Rule::builder("non-member-price", "Non-Member Price")
    .salience(8)
    .condition(Condition::Not(Box::new(
        Condition::Pattern {
            fact_type: "customer".to_string(),
            field: "member".to_string(),
            value: Value::Bool(true),
            op: ComparisonOp::Eq,
        },
    )))
    .action(Action::LogMessage {
        level: LogLevel::Info,
        message: "Non-member pricing applied".to_string(),
    })
    .build();
```

---

## Actions and Effects

### Inserting Facts

Create new facts during rule execution:

```rust
use engine::{Action, ComparisonOp, Condition, Rule, Value};

let rule = Rule::builder("detect-high-risk", "Detect High Risk")
    .salience(30)
    .condition(Condition::Pattern {
        fact_type: "transaction".to_string(),
        field: "amount".to_string(),
        value: Value::Integer(50000),  // $50,000
        op: ComparisonOp::Eq,
    })
    .action(Action::InsertFact {
        fact_type: "risk_alert".to_string(),
        fields: [
            ("severity".to_string(), Value::String("high".to_string())),
            ("reason".to_string(), Value::String("Amount exceeds threshold".to_string())),
        ].into_iter().collect(),
    })
    .build();
```

### Updating Facts

Modify existing facts:

```rust
use engine::{Action, ComparisonOp, Condition, Expression, Rule, Value};
use uuid::Uuid;

let fact_id = uuid::Uuid::new_v4().to_string();  // the fact this rule updates

let rule = Rule::builder("update-score", "Update Customer Score")
    .salience(12)
    .condition(Condition::Pattern {
        fact_type: "customer".to_string(),
        field: "status".to_string(),
        value: Value::String("premium".to_string()),
        op: ComparisonOp::Eq,
    })
    // `UpdateFact` takes expressions, so a field can be computed from working memory
    // (`score := score + 10`) as well as set to a literal.
    .action(Action::UpdateFact {
        id: fact_id.clone(),  // the fact to update
        fields: [
            ("score".to_string(), Expression::Literal(Value::Integer(100))),
        ].into_iter().collect(),
    })
    .build();
```

### Retracting Facts

Remove facts from working memory:

```rust
use engine::{Action, ComparisonOp, Condition, Rule, Value};

let rule = Rule::builder("cancel-invalid", "Cancel Invalid Order")
    .salience(25)
    .condition(Condition::Pattern {
        fact_type: "order".to_string(),
        field: "status".to_string(),
        value: Value::String("invalid".to_string()),
        op: ComparisonOp::Eq,
    })
    .action(Action::RetractFact {
        id: "order-to-retract".to_string(),  // the fact to remove
    })
    .build();
```

### Calling Functions

Execute registered functions:

```rust
use engine::{Action, ComparisonOp, Condition, Rule, Value};

let rule = Rule::builder("send-notification", "Send Notification")
    .salience(5)
    .condition(Condition::Pattern {
        fact_type: "order".to_string(),
        field: "status".to_string(),
        value: Value::String("shipped".to_string()),
        op: ComparisonOp::Eq,
    })
    .action(Action::CallFunction {
        name: "send_email_notification".to_string(),
        args: vec![
            Value::String("customer@example.com".to_string()),
            Value::String("Your order has shipped!".to_string()),
        ],
    })
    .build();
```

---

## Rule Priority (Salience)

Rules with higher salience fire first. Default is 0.

### Example: Layered Processing

```rust
use engine::{Action, ComparisonOp, Condition, LogLevel, Rule, Value};

// High priority: Safety checks (fire first)
let safety_rule = Rule::builder("safety-check", "Safety Check")
    .salience(100)  // Highest
    .condition(Condition::Pattern {
        fact_type: "transaction".to_string(),
        field: "amount".to_string(),
        value: Value::Integer(100000),  // $100,000
        op: ComparisonOp::Eq,
    })
    .action(Action::LogMessage {
        level: LogLevel::Warning,
        message: "CRITICAL: Transaction over $100k - manual review required".to_string(),
    })
    .build();

// Medium priority: Compliance checks
let compliance_rule = Rule::builder("compliance-check", "Compliance Check")
    .salience(50)
    .condition(Condition::Pattern {
        fact_type: "transaction".to_string(),
        field: "amount".to_string(),
        value: Value::Integer(10000),  // $10,000
        op: ComparisonOp::Eq,
    })
    .action(Action::LogMessage {
        level: LogLevel::Info,
        message: "Compliance check triggered for large transaction".to_string(),
    })
    .build();

// Low priority: Customer experience optimizations
let offer_rule = Rule::builder("customer-offer", "Customer Offer")
    .salience(10)
    .condition(Condition::Pattern {
        fact_type: "order".to_string(),
        field: "amount".to_string(),
        value: Value::Integer(50),  // $50
        op: ComparisonOp::Eq,
    })
    .action(Action::LogMessage {
        level: LogLevel::Info,
        message: "Offer applicable for customer".to_string(),
    })
    .build();
```

**Execution order**: Safety Check → Compliance Check → Customer Offer

---

## Complex Scenarios

### Scenario 1: Loan Approval System

```rust
use engine::{Action, ComparisonOp, Condition, Rule, Value};

// Rule 1: Basic eligibility check
let basic_eligibility = Rule::builder("loan-basic-eligibility", "Basic Eligibility")
    .salience(100)
    .condition(Condition::And(vec![
        Condition::Pattern {
            fact_type: "applicant".to_string(),
            field: "age".to_string(),
            value: Value::Integer(18),
            op: ComparisonOp::Eq,
        },
        Condition::Compare {
            left: Box::new(Condition::Pattern {
                fact_type: "applicant".to_string(),
                field: "credit_score".to_string(),
                value: Value::Integer(600),
                op: ComparisonOp::Eq,
            }),
            op: ComparisonOp::Ge,
            right: Box::new(Condition::True),
        },
    ]))
    .action(Action::InsertFact {
        fact_type: "eligibility_status".to_string(),
        fields: [
            ("status".to_string(), Value::String("basic_eligible".to_string())),
        ].into_iter().collect(),
    })
    .build();

// Rule 2: Income verification
let income_check = Rule::builder("loan-income-check", "Income Verification")
    .salience(90)
    .condition(Condition::And(vec![
        Condition::Pattern {
            fact_type: "eligibility_status".to_string(),
            field: "status".to_string(),
            value: Value::String("basic_eligible".to_string()),
            op: ComparisonOp::Eq,
        },
        Condition::Compare {
            left: Box::new(Condition::Pattern {
                fact_type: "applicant".to_string(),
                field: "annual_income".to_string(),
                value: Value::Integer(30000),  // $30k
                op: ComparisonOp::Eq,
            }),
            op: ComparisonOp::Ge,
            right: Box::new(Condition::True),
        },
    ]))
    .action(Action::InsertFact {
        fact_type: "eligibility_status".to_string(),
        fields: [
            ("status".to_string(), Value::String("income_verified".to_string())),
        ].into_iter().collect(),
    })
    .build();

// Rule 3: Reject if debt too high
let debt_check = Rule::builder("loan-debt-check", "Debt Verification")
    .salience(95)  // Higher than income check to run first
    .condition(Condition::Compare {
        left: Box::new(Condition::Pattern {
            fact_type: "applicant".to_string(),
            field: "debt_to_income_ratio".to_string(),
            value: Value::Float(0.43),  // 43%
            op: ComparisonOp::Eq,
        }),
        op: ComparisonOp::Gt,
        right: Box::new(Condition::True),
    })
    .action(Action::InsertFact {
        fact_type: "eligibility_status".to_string(),
        fields: [
            ("status".to_string(), Value::String("rejected".to_string())),
            ("reason".to_string(), Value::String("Debt-to-income ratio too high".to_string())),
        ].into_iter().collect(),
    })
    .build();

// Rule 4: Final approval
let final_approval = Rule::builder("loan-approval", "Loan Approval")
    .salience(80)
    .condition(Condition::And(vec![
        Condition::Pattern {
            fact_type: "eligibility_status".to_string(),
            field: "status".to_string(),
            value: Value::String("income_verified".to_string()),
            op: ComparisonOp::Eq,
        },
        Condition::Not(Box::new(
            Condition::Pattern {
                fact_type: "eligibility_status".to_string(),
                field: "status".to_string(),
                value: Value::String("rejected".to_string()),
                op: ComparisonOp::Eq,
            },
        )),
    ]))
    .action(Action::InsertFact {
        fact_type: "loan_decision".to_string(),
        fields: [
            ("decision".to_string(), Value::String("approved".to_string())),
        ].into_iter().collect(),
    })
    .build();
```

### Scenario 2: E-commerce Discount System

```rust
use engine::{Action, ComparisonOp, Condition, Rule, Value};

// Rule 1: VIP customers get automatic discount
let vip_discount = Rule::builder("vip-auto-discount", "VIP Auto Discount")
    .salience(100)
    .condition(Condition::And(vec![
        Condition::Pattern {
            fact_type: "customer".to_string(),
            field: "tier".to_string(),
            value: Value::String("gold".to_string()),
            op: ComparisonOp::Eq,
        },
        Condition::Compare {
            left: Box::new(Condition::Pattern {
                fact_type: "cart".to_string(),
                field: "total".to_string(),
                value: Value::Integer(50),  // $50 minimum
                op: ComparisonOp::Eq,
            }),
            op: ComparisonOp::Ge,
            right: Box::new(Condition::True),
        },
    ]))
    .action(Action::InsertFact {
        fact_type: "discount".to_string(),
        fields: [
            ("type".to_string(), Value::String("vip_10_percent".to_string())),
            ("percentage".to_string(), Value::Integer(10)),
            ("applied".to_string(), Value::Bool(true)),
        ].into_iter().collect(),
    })
    .build();

// Rule 2: Holiday promotions
let holiday_promo = Rule::builder("holiday-promo", "Holiday Promotion")
    .salience(95)
    .condition(Condition::And(vec![
        Condition::Pattern {
            fact_type: "season".to_string(),
            field: "event".to_string(),
            value: Value::String("holiday_sale".to_string()),
            op: ComparisonOp::Eq,
        },
        Condition::Compare {
            left: Box::new(Condition::Pattern {
                fact_type: "cart".to_string(),
                field: "total".to_string(),
                value: Value::Integer(100),  // $100 minimum
                op: ComparisonOp::Eq,
            }),
            op: ComparisonOp::Ge,
            right: Box::new(Condition::True),
        },
    ]))
    .action(Action::InsertFact {
        fact_type: "discount".to_string(),
        fields: [
            ("type".to_string(), Value::String("holiday_15_percent".to_string())),
            ("percentage".to_string(), Value::Integer(15)),
            ("applied".to_string(), Value::Bool(true)),
        ].into_iter().collect(),
    })
    .build();

// Rule 3: Clear cart if no items
let clear_empty_cart = Rule::builder("clear-empty-cart", "Clear Empty Cart")
    .salience(200)  // Very high priority - safety rule
    .condition(Condition::And(vec![
        Condition::Pattern {
            fact_type: "cart".to_string(),
            field: "item_count".to_string(),
            value: Value::Integer(0),
            op: ComparisonOp::Eq,
        },
    ]))
    .action(Action::RetractFact {
        id: "cart-to-retract".to_string(),  // the empty cart to remove
    })
    .build();
```

### Scenario 3: Fraud Detection System

```rust
use engine::{Action, ComparisonOp, Condition, Rule, Value};

// Rule 1: Detect unusual amount
let unusual_amount = Rule::builder("fraud-unusual-amount", "Unusual Amount")
    .salience(100)
    .condition(Condition::Compare {
        left: Box::new(Condition::Pattern {
            fact_type: "transaction".to_string(),
            field: "amount".to_string(),
            value: Value::Integer(5000),  // $5,000
            op: ComparisonOp::Eq,
        }),
        op: ComparisonOp::Gt,
        right: Box::new(Condition::True),
    })
    .action(Action::InsertFact {
        fact_type: "fraud_flag".to_string(),
        fields: [
            ("flag".to_string(), Value::String("unusual_amount".to_string())),
            ("risk_score".to_string(), Value::Integer(30)),
        ].into_iter().collect(),
    })
    .build();

// Rule 2: Detect unusual location
let unusual_location = Rule::builder("fraud-unusual-location", "Unusual Location")
    .salience(100)
    .condition(Condition::Pattern {
        fact_type: "transaction".to_string(),
        field: "is_foreign".to_string(),
        value: Value::Bool(true),
        op: ComparisonOp::Eq,
    })
    .action(Action::InsertFact {
        fact_type: "fraud_flag".to_string(),
        fields: [
            ("flag".to_string(), Value::String("foreign_transaction".to_string())),
            ("risk_score".to_string(), Value::Integer(20)),
        ].into_iter().collect(),
    })
    .build();

// Rule 3: Combine flags for high risk
let high_risk = Rule::builder("fraud-high-risk", "High Risk Fraud")
    .salience(95)
    .condition(Condition::And(vec![
        Condition::Pattern {
            fact_type: "fraud_flag".to_string(),
            field: "flag".to_string(),
            value: Value::String("unusual_amount".to_string()),
            op: ComparisonOp::Eq,
        },
        // Check if another fraud flag exists
        Condition::Pattern {
            fact_type: "fraud_flag".to_string(),
            field: "risk_score".to_string(),
            value: Value::Integer(20),  // Some other flag
            op: ComparisonOp::Eq,
        },
    ]))
    .action(Action::InsertFact {
        fact_type: "fraud_decision".to_string(),
        fields: [
            ("decision".to_string(), Value::String("block".to_string())),
            ("risk_score".to_string(), Value::Integer(80)),
        ].into_iter().collect(),
    })
    .build();

// Rule 4: Low risk - just flag for review
let low_risk = Rule::builder("fraud-low-risk", "Low Risk Fraud")
    .salience(90)
    .condition(Condition::And(vec![
        Condition::Pattern {
            fact_type: "fraud_flag".to_string(),
            field: "risk_score".to_string(),
            value: Value::Integer(30),  // Only one flag
            op: ComparisonOp::Eq,
        },
    ]))
    .action(Action::InsertFact {
        fact_type: "fraud_decision".to_string(),
        fields: [
            ("decision".to_string(), Value::String("review".to_string())),
            ("risk_score".to_string(), Value::Integer(50)),
        ].into_iter().collect(),
    })
    .build();
```

---

## DMN decisions

Rules and DMN are two ways to express the same thing. A decision model is a `.dmn` file — the OMG
standard XML that any DMN editor produces — and rRules evaluates it without translating it.

### Evaluating a decision in Rust

The engine parses the document, evaluates a decision by id, and returns its outputs. The decisions a
model declares are the ids in its `<decision>` elements.

```rust
use engine::dmn::evaluator::{DecisionEvaluator, EvaluateRequest};
use engine::dmn::parser::parse_dmn_with_cache;
use std::sync::{Arc, RwLock};

let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20180913/MODEL/" id="demo" name="Demo"
             namespace="http://example.com/demo">
  <decision id="discount" name="Discount">
    <decisionTable id="discount_table" hitPolicy="UNIQUE">
      <input id="in_customer"><inputExpression><text>customer</text></inputExpression></input>
      <output id="out_rate" name="rate" typeRef="number"/>
      <rule id="r1">
        <inputEntry><text>"premium"</text></inputEntry>
        <outputEntry><text>20</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#;

let (model, cache) = parse_dmn_with_cache(xml).expect("a valid DMN document");
let shared = engine::dmn::evaluator::SharedDmnModel::new(model, cache);
let evaluator = DecisionEvaluator::new(Arc::new(RwLock::new(shared)));

let mut inputs = std::collections::HashMap::new();
inputs.insert(
    "customer".to_string(),
    engine::data_types::Value::String("premium".to_string()),
);
let result = evaluator
    .evaluate("discount", EvaluateRequest { inputs, trace: false })
    .expect("the decision exists and its table has a matching rule");

// The output names come from the `<output>` elements of the decision table.
assert_eq!(
    result.outputs.get("rate"),
    Some(&engine::data_types::Value::Number(engine::Decimal::from(20))),
);
```

Two things are worth knowing before writing a model:

- **Hit policy decides what multiple matching rules mean.** `UNIQUE` refuses to evaluate when two
  rules match, and the API answers `400` with `DMN_HIT_POLICY_VIOLATION`; `FIRST`, `PRIORITY`,
  `COLLECT` and the rest each define their own answer. The full set is in
  [`specs/dmn-feel.md`](../specs/dmn-feel.md).
- **A decision that depends on another is evaluated first.** The order comes from the decision
  requirements graph, not from the order of the elements in the file, and independent branches are
  evaluated in parallel.

### Managing decisions over HTTP

`POST /v1/decisions/import` stores a model, `GET /v1/decisions` lists what is stored, and
`POST /v1/decisions/{id}/evaluate` runs one:

```bash
# Store a model. The body is the DMN XML itself.
curl -X POST "$HOST/v1/decisions/import" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/xml' \
  --data-binary @discount.dmn

# Evaluate it with inputs named after your input expressions.
curl -X POST "$HOST/v1/decisions/discount/evaluate" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"inputs": {"customer": "premium"}, "trace": false}'
```

The response carries `outputs`, `rules_fired` and `duration_ms`, and with `"trace": true` an
evaluation trace as well. Every endpoint is documented in [`API.md`](API.md).

## Governance: approval before a rule goes live

A rule is not published by the person who writes it. Submitting a rule creates an approval request,
and another user approves or rejects it; both actions are recorded in the audit trail with the acting
user.

```bash
# The maker submits their change.
curl -X POST "$HOST/v1/rules/$RULE_ID/submit" \
  -H "Authorization: Bearer $MAKER_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"maker_id": "alice"}'

# The checker sees what is waiting, then decides.
curl "$HOST/v1/approvals/pending" -H "Authorization: Bearer $CHECKER_TOKEN"
curl -X POST "$HOST/v1/approvals/$APPROVAL_ID/approve" \
  -H "Authorization: Bearer $CHECKER_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"approver_id": "bob"}'
```

The same flow is in the interface under **Governance → Approval Queue**, and the trail it leaves is
under **Governance → Audit Log**. `DELETE /v1/rules/{id}` **archives** a rule rather than erasing it —
an audit trail would mean little if the thing it names could vanish — and archived rules keep their
row with an `Archived` badge.

---

## Testing Your Rules

A rule set is testable like any other value: build the rules, run them against facts, and assert on
what fired. This example is compiled *and executed* by `make docs-snippets`, so it cannot drift from
the engine it documents.

```rust
use engine::data_types::Value;
use engine::engine::registry::{EngineRegistry, ExecutionEngine};
use engine::fact::Fact;
use engine::rules::{Action, ComparisonOp, Condition, LogLevel, Rule, RuleSet};
use std::collections::HashMap;

#[test]
fn test_loan_approval() {
    // The rule: an applicant at least 18 is eligible.
    let rule = Rule::builder("eligible", "Old enough to borrow")
        .condition(Condition::Pattern {
            fact_type: "applicant".to_string(),
            field: "age".to_string(),
            value: Value::Integer(18),
            op: ComparisonOp::Ge,
        })
        .action(Action::LogMessage {
            level: LogLevel::Info,
            message: "applicant is eligible".to_string(),
        })
        .build();
    let mut rules = RuleSet::new();
    rules.add_rules(vec![rule]).expect("valid rule set");

    // Any engine the registry reports runs the same rules; `rete` is the default.
    let mut engine = EngineRegistry::new()
        .create_engine_by_name("rete")
        .expect("rete is a known engine")
        .expect("rete constructs");
    engine.load_ruleset(&rules).expect("the rule set loads");

    let mut payload = HashMap::new();
    payload.insert("age".to_string(), Value::Integer(30));
    engine
        .insert_fact(Fact::new("applicant", payload))
        .expect("the fact is inserted");

    let result = engine.execute();
    assert!(result.success, "{}", result.message);
    assert_eq!(result.rules_fired, 1, "the eligibility rule fires");
    assert_eq!(result.fired_rule_ids, vec!["eligible".to_string()]);
}
```

Two habits worth keeping:

- **Test the rule set, not just the rule.** A condition that never matches a fact type is invisible
  until something puts a fact in front of it, and `rules_fired == 0` is the symptom.
- **Assert on `fired_rule_ids`.** The count tells you something fired; the ids tell you *which*, and
  that is what catches a rule that fires for the wrong reason.

`engine/tests/cross_engine_suite.rs` is a worked example of the same idea taken further: it runs one
scenario against every engine the registry reports, so the four implementations are held to the same
behaviour rather than to four copied fixtures.

---

## Best Practices

1. **Start with high salience for safety rules.** Validation and security checks should be considered
   before the rules they guard; salience orders the passes within an execution.
2. **A rule fires at most once per execution, so you do not need `no_loop`.** The evaluator records
   every rule it fires and skips it for the rest of that `execute()` call, which is the idempotence
   `no_loop` was meant to provide. The flag still serialises (it is part of the rule schema) but
   nothing read it; treat the guarantee as unconditional.
3. **Use `lock_on_active` to sequence within an agenda group.** A rule with it set blocks the rest of
   its `agenda_group` for the pass it fires — that one *is* honoured.
4. **Organize by business domains.** Group related rules logically; the agenda group is the mechanism
   the engine understands.
5. **Document rule purpose.** Descriptive names and comments, because a rule set is read far more
   often than it is written.
6. **Test incrementally.** Start with simple rules, add complexity gradually, and assert on
   `fired_rule_ids` rather than only on a count.
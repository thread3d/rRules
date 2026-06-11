# rRules User Guide - Writing Rules

This guide shows you how to write rules for the rRules Business Rules Management System, from simple patterns to complex scenarios.

## Table of Contents

1. [Basic Rule Structure](#basic-rule-structure)
2. [Simple Pattern Matching](#simple-pattern-matching)
3. [Conditional Logic](#conditional-logic)
4. [Actions and Effects](#actions-and-effects)
5. [Rule Priority (Salience)](#rule-priority-salience)
6. [Complex Scenarios](#complex-scenarios)

---

## Basic Rule Structure

A rule has two parts:
- **Condition (LHS)**: When this is true, the rule fires
- **Actions (RHS)**: What happens when the rule fires

### Syntax

```rust
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
use engine::{Rule, Condition};

let rule = Rule::builder("high-status-customer", "High Status Customer")
    .salience(10)
    .condition(Condition::Pattern {
        fact_type: "customer".to_string(),
        field: "status".to_string(),
        value: Value::String("premium".to_string()),
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
use engine::{Rule, Condition, Value, ComparisonOp};

let rule = Rule::builder("large-transaction", "Large Transaction")
    .salience(20)
    .condition(Condition::Compare {
        left: Box::new(Condition::Pattern {
            fact_type: "transaction".to_string(),
            field: "amount".to_string(),
            value: Value::Integer(10000),
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
use engine::{Rule, Condition};

let rule = Rule::builder("vip-discount", "VIP Discount")
    .salience(15)
    .condition(Condition::And(vec![
        // Customer has premium status
        Condition::Pattern {
            fact_type: "customer".to_string(),
            field: "status".to_string(),
            value: Value::String("premium".to_string()),
        },
        // Order amount is over $100
        Condition::Compare {
            left: Box::new(Condition::Pattern {
                fact_type: "order".to_string(),
                field: "amount".to_string(),
                value: Value::Integer(100),
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
use engine::{Rule, Condition};

let rule = Rule::builder("rush-hour-fee", "Rush Hour Surcharge")
    .salience(5)
    .condition(Condition::Or(vec![
        // Hour is 7-9 AM
        Condition::Pattern {
            fact_type: "time".to_string(),
            field: "hour".to_string(),
            value: Value::Integer(8),  // Simplified
        },
        // Hour is 5-7 PM
        Condition::Pattern {
            fact_type: "time".to_string(),
            field: "hour".to_string(),
            value: Value::Integer(18),  // Simplified
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
use engine::{Rule, Condition};

let rule = Rule::builder("non-member-price", "Non-Member Price")
    .salience(8)
    .condition(Condition::Not(Box::new(
        Condition::Pattern {
            fact_type: "customer".to_string(),
            field: "member".to_string(),
            value: Value::Bool(true),
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
use engine::{Rule, Condition, Action};

let rule = Rule::builder("detect-high-risk", "Detect High Risk")
    .salience(30)
    .condition(Condition::Pattern {
        fact_type: "transaction".to_string(),
        field: "amount".to_string(),
        value: Value::Integer(50000),  // $50,000
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
use engine::{Rule, Condition, Action};
use uuid::Uuid;

let rule = Rule::builder("update-score", "Update Customer Score")
    .salience(12)
    .condition(Condition::Pattern {
        fact_type: "customer".to_string(),
        field: "status".to_string(),
        value: Value::String("premium".to_string()),
    })
    .action(Action::UpdateFact {
        id: some_uuid,  // The fact to update
        fields: [
            ("score".to_string(), Value::Integer(100)),
        ].into_iter().collect(),
    })
    .build();
```

### Retracting Facts

Remove facts from working memory:

```rust
use engine::{Rule, Condition, Action};

let rule = Rule::builder("cancel-invalid", "Cancel Invalid Order")
    .salience(25)
    .condition(Condition::Pattern {
        fact_type: "order".to_string(),
        field: "status".to_string(),
        value: Value::String("invalid".to_string()),
    })
    .action(Action::RetractFact {
        id: some_uuid,  // The fact to remove
    })
    .build();
```

### Calling Functions

Execute registered functions:

```rust
use engine::{Rule, Condition, Action};

let rule = Rule::builder("send-notification", "Send Notification")
    .salience(5)
    .condition(Condition::Pattern {
        fact_type: "order".to_string(),
        field: "status".to_string(),
        value: Value::String("shipped".to_string()),
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
use engine::{Rule, Condition};

// High priority: Safety checks (fire first)
let safety_rule = Rule::builder("safety-check", "Safety Check")
    .salience(100)  // Highest
    .condition(Condition::Pattern {
        fact_type: "transaction".to_string(),
        field: "amount".to_string(),
        value: Value::Integer(100000),  // $100,000
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
    })
    .action(Action::LogMessage {
        level: LogLevel::Info,
        message: "Compliance check triggered for large transaction".to_string(),
    })
    .build();

// Low priority: Customer experience optimizations
let优惠_rule = Rule::builder("customer-offer", "Customer Offer")
    .salience(10)
    .condition(Condition::Pattern {
        fact_type: "order".to_string(),
        field: "amount".to_string(),
        value: Value::Integer(50),  // $50
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
use engine::{Rule, Condition, Value, ComparisonOp};

// Rule 1: Basic eligibility check
let basic_eligibility = Rule::builder("loan-basic-eligibility", "Basic Eligibility")
    .salience(100)
    .condition(Condition::And(vec![
        Condition::Pattern {
            fact_type: "applicant".to_string(),
            field: "age".to_string(),
            value: Value::Integer(18),
        },
        Condition::Compare {
            left: Box::new(Condition::Pattern {
                fact_type: "applicant".to_string(),
                field: "credit_score".to_string(),
                value: Value::Integer(600),
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
        },
        Condition::Compare {
            left: Box::new(Condition::Pattern {
                fact_type: "applicant".to_string(),
                field: "annual_income".to_string(),
                value: Value::Integer(30000),  // $30k
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
        },
        Condition::Not(Box::new(
            Condition::Pattern {
                fact_type: "eligibility_status".to_string(),
                field: "status".to_string(),
                value: Value::String("rejected".to_string()),
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
use engine::{Rule, Condition, Action};

// Rule 1: VIP customers get automatic discount
let vip_discount = Rule::builder("vip-auto-discount", "VIP Auto Discount")
    .salience(100)
    .condition(Condition::And(vec![
        Condition::Pattern {
            fact_type: "customer".to_string(),
            field: "tier".to_string(),
            value: Value::String("gold".to_string()),
        },
        Condition::Compare {
            left: Box::new(Condition::Pattern {
                fact_type: "cart".to_string(),
                field: "total".to_string(),
                value: Value::Integer(50),  // $50 minimum
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
        },
        Condition::Compare {
            left: Box::new(Condition::Pattern {
                fact_type: "cart".to_string(),
                field: "total".to_string(),
                value: Value::Integer(100),  // $100 minimum
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
        },
    ]))
    .action(Action::RetractFact {
        id: cart_uuid,  // The empty cart to remove
    })
    .build();
```

### Scenario 3: Fraud Detection System

```rust
use engine::{Rule, Condition, Action};

// Rule 1: Detect unusual amount
let unusual_amount = Rule::builder("fraud-unusual-amount", "Unusual Amount")
    .salience(100)
    .condition(Condition::Compare {
        left: Box::new(Condition::Pattern {
            fact_type: "transaction".to_string(),
            field: "amount".to_string(),
            value: Value::Integer(5000),  // $5,000
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
        },
        // Check if another fraud flag exists
        Condition::Pattern {
            fact_type: "fraud_flag".to_string(),
            field: "risk_score".to_string(),
            value: Value::Integer(20),  // Some other flag
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

## Testing Your Rules

The rRules engine provides testing utilities:

```rust
use engine::{Rule, Condition, EngineBuilder, Fact};
use std::collections::HashMap;

#[test]
fn test_loan_approval() {
    // Create engine with test rules
    let mut engine = EngineBuilder::new().build();
    
    // Insert facts
    let mut applicant_payload = HashMap::new();
    applicant_payload.insert("age".to_string(), Value::Integer(30));
    applicant_payload.insert("credit_score".to_string(), Value::Integer(750));
    applicant_payload.insert("annual_income".to_string(), Value::Integer(50000));
    
    let applicant_fact = Fact::new("applicant", applicant_payload);
    engine.working_memory_mut().insert_fact(applicant_fact).unwrap();
    
    // Execute rules
    let result = engine.execute_forward();
    
    // Verify results
    assert!(result.success);
    assert_eq!(result.rules_fired, 2);  // Eligibility + income check
}
```

---

## Best Practices

1. **Start with high salience for safety rules**: Validation and security checks should fire first
2. **Use no_loop for idempotent rules**: Prevent rules from firing repeatedly on their own results
3. **Organize by business domains**: Group related rules together logically
4. **Document rule purpose**: Use descriptive names and comments
5. **Test incrementally**: Start with simple rules, add complexity gradually

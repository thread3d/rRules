# Testing Framework Stack

**Project:** rRules - Business Rules Management System (BRMS)
**Researched:** 2026-06-03
**Mode:** Ecosystem research for BDD/Gherkin and rule engine testing patterns

## Recommended Stack

### Core Testing Frameworks

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `criterion` | 0.5.x - 0.8.x | Micro-benchmarking | Industry standard for Rust benchmarks; integrates with cargo test |
| `mockall` | 0.12.x - 0.13.x | Mocking/duck-typing | Essential for isolating rule engine components during testing |
| `serial_test` | 2.x - 3.x | Parallel test isolation | Critical for integration tests that share state |

### BDD/Gherkin Testing

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `gherkin` | 15.x | Gherkin parser | Most actively maintained; used by cucumber-rs; pure Rust |
| `cucumber-rs` | 0.2.x - 0.3.x | BDD test runner | Official Cucumber implementation for Rust; Gherkin v15 support |
| `spec-rs` | 0.6.x - 0.7.x | Spec-style testing | Built-in Given/When/Then syntax; good for rule execution scenarios |

### Integration Testing Libraries

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `assert_cmd` | 2.x | CLI integration tests | Perfect for testing CLI commands like `rulerun` |
| `reqwest` + `wiremock` | 0.11.x / 0.7.x | HTTP API testing | Mock REST endpoints for rule execution integration tests |
| `tempfile` | 3.10.x - 3.12.x | Test file cleanup | Safe temporary directory creation for Git-backed storage tests |

### Coverage and Reporting

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `cargo-tarpaulin` | 0.26.x - 0.27.x | Code coverage | Best Rust coverage tool; works with Criterion benchmarks |
| `cargo-llvm-cov` | 0.4.x - 0.6.x | LLVM coverage | More accurate for complex codegen; integrates with GitHub Actions |

## BDD Testing Pattern: Rule Execution Scenarios

### Gherkin Feature File Example

```gherkin
Feature: Credit Approval Rule Engine

  Scenario: Approve standard credit application
    Given a customer with credit score > 700
    And annual income >= $50,000
    And no recent delinquencies
    When the customer applies for $10,000 personal loan
    Then the application should be approved
    And interest rate should be <= 8.5%
    And a credit check should NOT be triggered

  Scenario: Reject high-risk application
    Given a customer with credit score < 600
    And income history < 2 years
    When applying for $5,000
    Then the application should be rejected
    And reason should include "insufficient_credit_history"
```

### Rust Test Implementation Pattern

```rust
use cucumber::{given, when, then, World};

#[derive(World)]
struct RuleEngineWorld {
    engine: RuleEngine,
    customer: Option<Customer>,
    result: Option<ApprovalResult>,
}

#[given("a customer with credit score > 700")]
async fn high_credit_score(world: &mut RuleEngineWorld) {
    world.customer = Some(Customer::high_score());
}

#[when("the customer applies for loan")]
async fn apply_for_loan(world: &mut RuleEngineWorld) {
    let result = world.engine.execute(&world.customer.unwrap()).await;
    world.result = Some(result);
}

#[then("should be approved")]
async fn assert_approved(world: &mut RuleEngineWorld) {
    assert!(world.result.as_ref().unwrap().is_approved());
}
```

### Integration Pattern: Testing Forward/Backward Chaining

```rust
#[cfg(test)]
mod forward_chaining_tests {
    use serial_test::serial;
    use mockall::predicate::*;

    #[tokio::test]
    #[serial] // Prevent race conditions with shared KieSession
    async fn test_chained_rule_firings() {
        let mut engine = RuleEngine::new();

        // Insert facts
        engine.insert(Customer { score: 750, income: 60000 });

        // Assert chain reaction: score triggers eligibility, then rate calculation
        engine.run_forward_chaining().await.unwrap();

        let facts = engine.get_facts();
        assert!(facts.contains(&Fact::Eligible));
        assert!(facts.contains(&Fact::RateCalculated { rate: le(8.5) }));
    }
}

#[cfg(test)]
mod backward_chaining_tests {
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn test_goal_directed_inference() {
        let mut engine = RuleEngine::new();

        // Goal: Determine approval decision
        engine.add_goal(Goal::ApprovalDecision);

        // Backward chain from goal to facts needed
        engine.run_backward_chaining().await.unwrap();

        assert!(engine.get_missing_facts().contains(&Fact::CreditScore));
    }
}
```

## Alternative Approaches

### Approach 1: Custom BDD Framework (Recommended for v2.0)

Build lightweight BDD infrastructure on top of `criterion`:

```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn rule_execution_bdd(c: &mut Criterion) {
    let mut group = c.benchmark_group("BDD Scenarios");
    
    // Given-When-Then pattern within Criterion
    group.bench_function("credit approval - standard", |b| {
        b.iter(|| {
            let engine = RuleEngine::new();
            engine.insert(Customer { score: 750, income: 60000 });
            let result = engine.execute(&Customer { score: 750, income: 60000 });
            assert!(result.is_approved());
        })
    });
}
```

### Approach 2: Property-Based Testing with proptest

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn rule_engine_respects_constraints(
        score in 300..850,
        income in 1..1000000,
        loan_amount in 1..100000
    ) {
        let mut engine = RuleEngine::new();
        engine.insert(Customer { score, income });
        
        let result = engine.execute(&Customer { score, income });
        
        // Property: approved customers have score > 650
        if result.is_approved() {
            assert!(score > 650);
        }
    }
}
```

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| BDD runner | cucumber-rs | `behave-rs` | behave-rs appears unmaintained or inactive |
| Mocking | mockall | `test-mock` | mockall has better async support |
| Coverage | tarpaulin | `kcov` | kcov requires external system dependencies |

## Installation

```bash
# Core testing stack
cargo add --dev criterion mockall serial_test tokio --features full

# BDD/Gherkin options
cargo add --dev gherkin cucumber-rs spec-rs

# Integration testing
cargo add --dev assert_cmd reqwest wiremock tempfile

# Coverage tools (install separately)
cargo install cargo-tarpaulin
cargo install cargo-llvm-cov
```

## Sources

- [criterion.rs - Statistics-driven micro-benchmarking](https://bheisler.github.io/criterion.rs/book/)
- [Mockall Documentation](https://docs.rs/mockall/latest/mockall/)
- [Gherkin crate docs](https://docs.rs/gherkin/)
- [Cucumber-rs GitHub](https://github.com/cucumber-rs/cucumber-rs)
- [cargo-tarpaulin GitHub](https://github.com/greshka/cargo-tarpaulin)
- [cargo-llvm-cov GitHub](https://github.com/taiki-e/cargo-llvm-cov)

## Confidence Assessment

| Area | Level | Reason |
|------|-------|--------|
| Core testing stack | HIGH | criterion, mockall are mature and well-documented with active maintenance |
| BDD/Gherkin integration | MEDIUM | cucumber-rs is the official Cucumber implementation but less commonly used than JS alternatives; gherkin v15 is standard |
| Integration patterns | MEDIUM | Pattern based on common BRMS testing needs; rule-specific variations may need tuning during implementation |

## Recommendations for rRules

1. **Start with criterion + mockall** for Phase 2 - these are essential for all testing
2. **Add cucumber-rs** for BDD feature file support when implementing the v2.0 milestone
3. **Use serial_test** for integration tests that share in-memory state (e.g., KieSession)
4. **Consider property-based testing** with `proptest` to verify chaining rules handle edge cases
5. **Integrate cargo-tarpaulin** into CI for coverage reporting; target >80% for core engine

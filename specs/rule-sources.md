# Rule sources — the two universes, and the bridge between them

**Status:** current behaviour. Written in 9.0.73, when nothing converted one store into the other, and
corrected in 9.0.87: 9.0.74-9.0.82 built the bridge this document's opening section assumed would never
exist, and left three claims below describing the code as it had been rather than as it is. A gap that
is closed and still described as open costs a reader the same as a bug that is still there.

## The two universes, and the bridge

| | Rule **documents** | Rules the engine **executes** |
|---|---|---|
| Stored where | in memory, `RuleStore` = `HashMap<(tenant_id, Uuid), StoredRule>` (`api/src/v1/rules.rs`) | `RULE_STORAGE_PATH/[<tenant>/]*.rules` on disk, parsed into `engine::rules::Rule` |
| Written by | `POST/PUT /v1/rules` (`create_rule`, `update_rule`) | an operator, **and** those two handlers when the request carries a `rule` or a `dsl_body` that parses |
| Carries | `id`, `tenant_id`, `name`, `category`, `tags`, `dsl_body`, `status`, `version`, timestamps | `id` (a string), `name`, `salience`, `no_loop`, `lock_on_active`, `agenda_group`, `condition`, `actions` |
| Read by | `/v1/rules` CRUD, `/v1/approvals`, the IDE | `build_tenant_ruleset_from_dir` → `reload_and_swap` → `POST /v1/execute` |
| Survives a restart | **no** — the map is created empty in `ApiState` | yes, it is the file system |

**What converts one into the other: the create/update request itself.** `POST /v1/rules` (and `PUT`) takes
an optional `rule`, the engine's own `Rule` JSON. When it is present the API imposes the document's id and
name on it, validates it by loading it, and writes `<rules dir>/<tenant>/<id>.rules`, so the id the caller
receives is the id `rule_filter` matches and the id `POST /v1/execute` reports (9.0.74, tenant directory
since 9.0.77). When `rule` is absent, `dsl_body` is parsed by `engine::rule_syntax::parse_rule_text`
(`api/src/v1/rules.rs:483`) and, if it parses, written the same way (9.0.82). A body that does not parse is
**not** an error: it was prose for several releases, so it stays a draft and the response says so with
`executable: false`.

**What still does not convert:** a document whose body is prose, or a body written in a syntax the parser
declines (`specs/rule-syntax.md` lists what the text form cannot express). Those remain documents the
engine never sees, and `executable: false` is the field that says so.

`api/tests/execute_integration.rs::a_created_rule_is_not_executable_and_its_id_matches_no_executed_rule`
is not a counter-example: its body is `rule "api-created" when true then log("hi")`, and the text grammar
has no bare-literal condition (`specs/rule-syntax.md`: `expr := ref op literal`), so the body is rejected
and the test exercises the *draft* path. Its sibling,
`a_rule_created_with_a_structured_rule_becomes_executable`, is the bridge.

## What the remaining gap costs a user

1. **The IDE authors executable rules only for shapes the form or the text grammar can express.** A
   document whose body is prose is stored, approved and listed, and never runs.
2. **"Test this rule" tests nothing for a draft** — `rules_matched: 0` is the honest signal for a document
   with no rule behind it, and `executable: false` is what distinguishes that from a rule that ran and
   matched nothing.
3. **Categories, tags, owners, status and approvals describe documents; salience, conditions and actions
   describe rules.** The two sets are bridged per document, not unified: editing `rule` JSON does not
   change `dsl_body` and vice versa — the form renders one from the other when *it* is the author.
4. **A restart loses every document** and keeps every executed rule — still true, and the reason the
   rule file is the durable artefact.

## Options, and what each would take

**A. Documents become executable (recommended — first slice shipped in 9.0.74).** Compile `dsl_body` into `engine::rules::Rule` — or, more
honestly, store the rule **as JSON** in the document instead of a DSL string, since the engine's format is
what executes. Add a `.rules` writer on create/update/status-change, so the directory and the documents
agree, and make `/v1/rules/reload` a no-op that reports "already current" rather than the only refresh.
Verification: an integration test that creates a rule through the API and executes it (id and category
both matching), and a reload that changes nothing because nothing was stale. Also needs a migration story
for existing directories and a decision about `dsl_body`.

**B. Execution reads documents.** Drop the directory and build the `RuleSet` from the rule store. Cheaper
for the API, but it makes execution depend on API memory, so a restart empties the rules an engine runs —
unacceptable for the deployments this ships.

**C. Keep them separate and say so.** Rename them (`/v1/rule-documents`), state in `README.md` and
`docs/API.md` that documents are drafts and the directory is what runs, and add the missing signal: a
document that is not on disk is reported as such, and "test this rule" refuses rather than answering
`rules_matched: 0`. Least machinery, but leaves the core value ("define decisions as reusable rules,
deploy them without a rebuild") delivered by a directory rather than by the product.

**D. Status quo.** Rejected: the gap is invisible from the outside, which is how it survived this long.

A and C are both defensible; A is what the product claims to be. Neither is a patch to one field, which is
why 9.0.72 refused instead of inventing a third category vocabulary.

## What shipped: the first slice of A (9.0.74)

`POST /v1/rules` accepts an optional **`rule`** — the engine's own `Rule` JSON. When it is present:

1. the API **imposes the document's identity** on it (id and name), so the id returned to the caller is the
   id the engine reports and the id `rule_filter.name_pattern` matches;
2. the engine validates it by loading it — a rule that would not execute is a `400`, and nothing is stored;
3. it is written to `RULE_STORAGE_PATH/<id>.rules`, and the ordinary `POST /v1/rules/reload` makes it part of
   the executing set.

`RuleResponse.executable` says which documents have a rule behind them. A document created with only a
`dsl_body` is stored and reported `executable: false` — there is no parser from that text to a rule, and the
response now says so rather than leaving a caller to infer it from a zero.

**A drift this slice exposed and closed:** `build_ruleset_from_dir` had its own file parser accepting only a
bare rule or an array, while `Engine::load_rules` accepted those *and* the `{"rules": [ … ]}` envelope. The
API writes the envelope, so its own directory loader skipped its own file — silently, with a `tracing::warn!`
nobody reads in a test. `engine::environment::parse_rules_document` is now the one parser, used by both.

### The tenant boundary (9.0.75)

Execution has **no tenant context**: `SharedRuleSet` is one set for the whole deployment and
`POST /v1/execute` carries no tenant, while `X-Tenant-ID` scopes only the document store. So a rule written
on behalf of tenant `acme` would run for every tenant — a cross-tenant leak introduced the moment the API
began writing the executing set.

`write_executable_rule` therefore refuses a non-default tenant with a 400 that says the set is
deployment-wide, and nothing is written. A *document* without `rule` is still accepted for any tenant, so
multi-tenant deployments keep their CRUD and approvals; they simply cannot add an executing rule until
execution is tenant-scoped. That is the gap `execution has no tenant` names, and it is now a boundary
rather than a silence.

### Tenant identity now comes from the token (9.0.76)

Tenant-scoped execution was blocked by something more basic: **there was no trustworthy tenant identity.**
`X-Tenant-ID` was the only mechanism, and any authenticated caller could set it to any value, so scoping
execution by that header would have looked like isolation and been none.

A JWT now carries the user's `tenant_id` (`auth::Claims`, from `UserRecord::tenant_id`), and
`extract_tenant_id` resolves: an **admin**'s header (the "manages several tenants" flow), else the **token's
tenant** whatever the header says, else the header, else `default`. A non-admin therefore cannot reach
another tenant's rules by forging `X-Tenant-ID`. `api/tests/auth_protection.rs` proves both halves —
including the admin isolation flow that CodeWitness's `API-003` scenario exercises, which the first version
of this change broke (`201|1` where `201|0` was required) and which is now a test of its own.

**The write boundary of 9.0.75 still stands**, unchanged: execution remains deployment-wide, so only the
default tenant may add an executing rule. What changed is that the question "which tenant is asking?" finally
has an answer that a caller cannot forge — which is the prerequisite for lifting that boundary rather than a
lift in itself.

### Execution is tenant-scoped (9.0.77)

`SharedRuleSet` now carries ownership: `tenants: rule id → tenant`, built from the directory layout —
`RULE_STORAGE_PATH/<tenant>/<file>.rules` gives a rule to `<tenant>`, and files in the root belong to
`default`, so an existing directory keeps running exactly as it did. `POST /v1/execute` and
`POST /v1/rules/batch-execute` resolve the caller's tenant (`extract_tenant_id`) and load **only that
tenant's** rules; `rule_filter.name_pattern` selects within them, and `rules_not_fired` reports only the
caller's rules on the sessionful path too.

`POST /v1/rules` writes an executable rule under its tenant's directory, so **9.0.75's refusal of every
non-default tenant is gone** — it was standing in for exactly this scoping. A rule created for `acme` now
runs for `acme` and for nobody else, which is what the test asserts.

`api/tests/tenant_scoped_execution.rs` covers the three cases that matter: each tenant runs only its own
rules, a tenant with none matches nothing (and no other tenant's rule ids appear in `rules_not_fired`), and a
`name_pattern` naming another tenant's rule selects nothing.

### The interface creates executable rules (9.0.78)

The New Rule form used to collect a free-text body in a pseudo-DSL (`rule "x" when customer.status =
"premium" then log(…)`) that **nothing parsed**. Every rule created through the interface was therefore a
draft that could never fire, while the interface presented rule creation as the core action.

The form now collects the structured fields the engine needs — fact type, field, operator, value and a log
message — and `frontend/src/api/rule-builder.ts` emits the engine's own rule JSON. `dsl_body` is **rendered
from the same draft**, so the text shown on the edit page describes the rule that runs rather than defining a
second one. `frontend/src/api/__tests__/rule-builder.test.ts` pins the shape and the rendering, including
that `1.5` stays a string rather than becoming a silently truncated integer.

### The form speaks the engine's value types (9.0.79)

The form's value control was untyped: every non-integer became a string, so `1.5` — plainly a number to the
person who typed it — was stored as `"1.5"` and compared against nothing. It now asks for the kind
(**Text**, **Number**, **True / false**) and `encodeValue` encodes that kind or reports that the text is not
one: an integer becomes `{"Integer": n}`, a fraction `{"Float": f}`, text `{"String": s}`, `true`/`false`
`{"Bool": b}`. A value of the wrong kind is an error in the form, not a reinterpretation, and the operator
list narrows to `=`/`!=` for booleans because ordering one is meaningless.

`Number('')` being `0` produced one real bug on the way — an empty box became a comparison against zero,
which looks like a deliberate answer — caught by the test written for the empty case.

### A text-only document can be migrated (9.0.80)

`PUT /v1/rules/{id}` accepts the same `rule` the create request does. That closes the gap left by 9.0.74:
documents created before it hold a `dsl_body` and nothing an engine can run, and the only repair was to
delete and re-create — losing the id, the history and any approvals.

The engine validates the rule, the API writes it under the caller's tenant, and the document reports
`executable: true` from then on, **keeping its identity**. A rule the engine rejects is a `400` before the
store is touched, so a bad migration changes nothing.

**And a defect the edit page had all along:** the frontend's view model dropped `dsl_body`, so the edit form's
textarea started empty and saving wrote that empty string back over the stored body — opening a rule and
pressing Save destroyed its text. `Rule.body` is mapped now, and `rules.mapping.test.ts` fails without it.

**Still open, in the order that matters:** a body in a grammar the parser declines stays a draft (see
`specs/rule-syntax.md` for what the text form deliberately cannot express); and documents written before
9.0.80 have no rule file until someone sends a `rule` or a parseable body, so they remain
`executable: false`.

Struck in 9.0.87: "`dsl_body` is not compiled" (9.0.82 compiles it through `engine::rule_syntax`) and "the
executing set is per deployment rather than per tenant" (9.0.77 made it tenant-scoped, and
`api/tests/tenant_scoped_execution.rs` holds it there).

## Evidence

- `api/src/v1/rules.rs` — `RuleStore`, `StoredRule`, `create_rule` (writes a rule file when the request
  carries `rule` or a parseable `dsl_body`), `write_executable_rule` (the tenant directory, identity
  imposed, validated by loading).
- `engine/src/rule_syntax.rs` — `parse_rule_text` / `render_rule_text`, the text form `dsl_body` is in.
- `api/src/v1/execution.rs` — `build_tenant_ruleset_from_dir`, `reload_and_swap`, `filter_ruleset`
  (per-tenant).
- `api/src/app.rs` — `rule_store: v1::rules::init_store()` (empty at startup), `rules_dir` from
  `RULE_STORAGE_PATH`.
- `api/tests/execute_integration.rs` — the draft path (`…is_not_executable…`) and the bridge
  (`…_with_a_structured_rule_becomes_executable`).
- `api/tests/tenant_scoped_execution.rs` — execution sees only the caller's tenant's rules.

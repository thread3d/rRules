# Rule sources — the two universes, and what reconciling them would take

**Status:** current behaviour, stated so it cannot be discovered by surprise. Written in 9.0.73 after
`rule_filter.categories` (9.0.72) turned out to be unimplementable: it assumed a link between two rule
sets that has never existed.

## The two universes

| | Rule **documents** | Rules the engine **executes** |
|---|---|---|
| Stored where | in memory, `RuleStore` = `HashMap<(tenant_id, Uuid), StoredRule>` (`api/src/v1/rules.rs`) | `RULE_STORAGE_PATH/*.rules` on disk, parsed into `engine::rules::Rule` |
| Written by | `POST/PUT /v1/rules` (`create_rule`, `update_rule`) | an operator, or whatever writes the rules directory |
| Carries | `id`, `tenant_id`, `name`, `category`, `tags`, `dsl_body`, `status`, `version`, timestamps | `id` (a string), `name`, `salience`, `no_loop`, `lock_on_active`, `agenda_group`, `condition`, `actions` |
| Read by | `/v1/rules` CRUD, `/v1/approvals`, the IDE | `build_ruleset_from_dir` → `reload_and_swap` → `POST /v1/execute` |
| Survives a restart | **no** — the map is created empty in `ApiState` | yes, it is the file system |

**Nothing converts one into the other.** No code parses `dsl_body`; no code writes `.rules` files. A rule
created through the API cannot be executed, and a rule that executes was never created through the API.
`/v1/rules/reload` reloads the disk set, which the API never writes.

`api/tests/execute_integration.rs::a_created_rule_is_not_executable_and_its_id_matches_no_executed_rule`
pins this: after creating a rule, `POST /v1/execute` matches only the disk rule, and filtering by the
created rule's own id — which is what the IDE's "test this rule" sends (`TestRunner.tsx:130`) — matches
nothing.

## What that costs a user

1. **The rule-authoring IDE does not author executable rules.** Documents and execution share a product,
   not a data path.
2. **"Test this rule" tests nothing** unless the document's UUID happens to equal a disk rule's id, which
   it never does. `rules_matched: 0` is the honest signal, but nothing surfaces it as an error.
3. **Categories, tags, owners, status and approvals describe documents that cannot run**; salience,
   conditions and actions describe rules that cannot be edited through the API.
4. **A restart loses every document** and keeps every executed rule — the opposite of what "the rule store
   is the source of truth" would suggest.

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

**Still open, in the order that matters:** `dsl_body` is not compiled (there is no rule-syntax parser, and
`dsl_body` is a FEEL expression inside a rule); the executing set is per deployment rather than per tenant;
and documents written before this slice have no rule file, so they remain `executable: false`.

## Evidence

- `api/src/v1/rules.rs` — `RuleStore`, `StoredRule`, `create_rule` (no compile step, no file write).
- `api/src/v1/execution.rs` — `build_ruleset_from_dir`, `reload_and_swap`, `filter_ruleset`.
- `api/src/app.rs` — `rule_store: v1::rules::init_store()` (empty at startup), `rules_dir` from
  `RULE_STORAGE_PATH`.
- `specs/http-api.md` — the `/v1/execute` row states the separation.
- `api/tests/execute_integration.rs` — the two tests named above.

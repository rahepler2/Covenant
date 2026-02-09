//! Verification engine tests — IVE checker, capability/IFC, contract verifier
//!
//! Covers:
//!   E001-E005, W001-W008, I001-I002 (IVE checker)
//!   F001-F006                        (Capability / IFC)
//!   V001-V005                        (Contract verification)

use covenant_lang::lexer::Lexer;
use covenant_lang::parser::Parser;
use covenant_lang::ast::*;
use covenant_lang::verify::checker::{verify_program, VerificationResult, Severity};
use covenant_lang::verify::capability::verify_capabilities;
use covenant_lang::verify::contract_verify::verify_contracts;

// ── Helpers ──────────────────────────────────────────────────────

/// Parse Covenant source into a Program AST.
fn parse(source: &str) -> Program {
    let tokens = Lexer::new(source, "test.cov").tokenize().unwrap();
    Parser::new(tokens, "test.cov").parse().unwrap()
}

/// Run full verification pipeline (IVE + capability + contract) and return all results.
fn verify_all(source: &str) -> Vec<VerificationResult> {
    let program = parse(source);
    verify_program(&program, "test.cov")
}

/// Run only capability / IFC verification.
fn verify_caps(source: &str) -> Vec<VerificationResult> {
    let program = parse(source);
    verify_capabilities(&program, "test.cov")
}

/// Run only contract verification (V-codes).
fn verify_cv(source: &str) -> Vec<VerificationResult> {
    let program = parse(source);
    verify_contracts(&program, "test.cov")
}

/// Collect diagnostic codes from results.
fn codes(results: &[VerificationResult]) -> Vec<String> {
    results.iter().map(|r| r.code.clone()).collect()
}

/// True if any result carries the given code.
fn has_code(results: &[VerificationResult], code: &str) -> bool {
    results.iter().any(|r| r.code == code)
}

/// Count how many results carry the given code.
fn count_code(results: &[VerificationResult], code: &str) -> usize {
    results.iter().filter(|r| r.code == code).count()
}

/// Return the severity of the first result with the given code.
fn severity_of(results: &[VerificationResult], code: &str) -> Option<Severity> {
    results.iter().find(|r| r.code == code).map(|r| r.severity)
}

// ══════════════════════════════════════════════════════════════════
//  IVE CHECKER TESTS  (E001-E005, W001-W008, I001-I002)
// ══════════════════════════════════════════════════════════════════

// ── 1. Clean contract produces no errors ─────────────────────────

#[test]
fn ive_01_clean_contract_no_errors() {
    let results = verify_all("\
intent: \"Test clean contract\"
scope: test.clean
risk: low

contract add(a: Int, b: Int) -> Int
  precondition:
    a >= 0
    b >= 0
  postcondition:
    result >= 0
  effects:
    reads [a, b]
    touches_nothing_else
  body:
    return a + b
");
    let errors: Vec<_> = results.iter()
        .filter(|r| matches!(r.severity, Severity::Error | Severity::Critical))
        .collect();
    assert!(
        errors.is_empty(),
        "Expected no errors, got: {:?}",
        errors.iter().map(|r| format!("{}: {}", r.code, r.message)).collect::<Vec<_>>()
    );
}

// ── 2. E001: undeclared mutation (no touches_nothing_else) ──────

#[test]
fn ive_02_e001_undeclared_mutation() {
    let results = verify_all("\
intent: \"Test undeclared mutation\"
scope: test.mutation
risk: low

contract mutate(obj: Record)
  effects:
    modifies [obj.x]
  body:
    obj.x = 1
    obj.y = 2
");
    assert!(has_code(&results, "E001"),
            "Expected E001 for undeclared mutation of obj.y, got codes: {:?}", codes(&results));
}

// ── 3. E003: undeclared function call (touches_nothing_else) ────

#[test]
fn ive_03_e003_undeclared_function_call() {
    let results = verify_all("\
intent: \"Test undeclared calls\"
scope: test.calls
risk: low

contract do_work(x: Int) -> Int
  effects:
    reads [x]
    touches_nothing_else
  body:
    result = external_svc.process(x)
    return result
");
    assert!(has_code(&results, "E003"),
            "Expected E003 for undeclared call to external_svc.process, got codes: {:?}", codes(&results));
}

// ── 4. E004: missing body ───────────────────────────────────────

#[test]
fn ive_04_e004_missing_body() {
    // The parser requires at least one section, so we construct the AST
    // manually with body = None and call verify_contract directly.
    use covenant_lang::verify::checker::verify_contract;

    let contract = ContractDef {
        name: "no_body".to_string(),
        params: vec![Param {
            name: "x".to_string(),
            type_expr: TypeExpr::Simple {
                name: "Int".to_string(),
                loc: SourceLocation::new("test.cov", 1, 1),
            },
            loc: SourceLocation::new("test.cov", 1, 1),
        }],
        return_type: None,
        is_async: false,
        precondition: None,
        postcondition: None,
        effects: None,
        permissions: None,
        body: None,
        on_failure: None,
        loc: SourceLocation::new("test.cov", 1, 1),
    };

    let results = verify_contract(&contract, None, "test.cov", None, RiskLevel::Low, None);
    assert!(has_code(&results, "E004"),
            "Expected E004 for contract without body, got codes: {:?}", codes(&results));
}

// ── 5. W003: missing precondition at HIGH risk ──────────────────

#[test]
fn ive_05_w003_missing_precondition_high_risk() {
    let results = verify_all("\
intent: \"Test high risk precondition\"
scope: test.risk
risk: high

contract risky(x: Int) -> Int
  postcondition:
    result >= 0
  effects:
    reads [x]
    touches_nothing_else
  body:
    return x * x
");
    assert!(has_code(&results, "W003"),
            "Expected W003 for missing precondition at HIGH risk, got codes: {:?}", codes(&results));
    assert_eq!(severity_of(&results, "W003"), Some(Severity::Error),
               "W003 at HIGH risk should be Error severity");
}

// ── 6. W004: missing postcondition at HIGH risk ─────────────────

#[test]
fn ive_06_w004_missing_postcondition_high_risk() {
    let results = verify_all("\
intent: \"Test high risk postcondition\"
scope: test.risk
risk: high

contract risky(x: Int) -> Int
  precondition:
    x >= 0
  effects:
    reads [x]
    touches_nothing_else
  body:
    return x * x
");
    assert!(has_code(&results, "W004"),
            "Expected W004 for missing postcondition at HIGH risk, got codes: {:?}", codes(&results));
    assert_eq!(severity_of(&results, "W004"), Some(Severity::Error));
}

// ── 7. W005: auto-escalation for external side effects ──────────

#[test]
fn ive_07_w005_auto_escalation_external_effects() {
    // Low risk but body mutates a dotted path without declaring effects.
    let results = verify_all("\
intent: \"Test auto escalation\"
scope: test.escalation
risk: low

contract mutator(obj: Record)
  body:
    obj.field = 42
");
    assert!(has_code(&results, "W005"),
            "Expected W005 auto-escalation for external mutation without effects block, got codes: {:?}",
            codes(&results));
    assert_eq!(severity_of(&results, "W005"), Some(Severity::Error));
}

// ── 8. W006: precondition references non-parameter ──────────────

#[test]
fn ive_08_w006_precondition_references_non_parameter() {
    let results = verify_all("\
intent: \"Test precondition relevance\"
scope: test.relevance
risk: low

contract check(a: Int) -> Int
  precondition:
    phantom > 0
  effects:
    reads [a]
    touches_nothing_else
  body:
    return a + 1
");
    assert!(has_code(&results, "W006"),
            "Expected W006 for precondition referencing non-parameter 'phantom', got codes: {:?}",
            codes(&results));
}

// ── 9. W007: postcondition old() reference not modified ─────────

#[test]
fn ive_09_w007_postcondition_old_not_modified() {
    let results = verify_all("\
intent: \"Test achievability with old\"
scope: test.achievability
risk: low

contract check_old(obj: Record) -> Int
  postcondition:
    obj.value == old(obj.value) + 1
  effects:
    reads [obj]
    touches_nothing_else
  body:
    return obj.value
");
    assert!(has_code(&results, "W007"),
            "Expected W007 for old(obj.value) when body does not modify obj.value, got codes: {:?}",
            codes(&results));
}

// ── 10. Pure contract passes clean ──────────────────────────────

#[test]
fn ive_10_pure_contract_passes_clean() {
    let results = verify_all("\
intent: \"Test pure contract\"
scope: test.pure
risk: low

contract add(a: Int, b: Int) -> Int
  pure
  body:
    return a + b
");
    let errors: Vec<_> = results.iter()
        .filter(|r| matches!(r.severity, Severity::Error | Severity::Critical))
        .collect();
    assert!(
        errors.is_empty(),
        "Pure contract should have no errors, got: {:?}",
        errors.iter().map(|r| format!("{}: {}", r.code, r.message)).collect::<Vec<_>>()
    );
}

// ── 11. Matching modifies passes ────────────────────────────────

#[test]
fn ive_11_modifies_with_matching_mutation_passes() {
    let results = verify_all("\
intent: \"Test matching modifies\"
scope: test.modifies
risk: low

contract update(obj: Record)
  effects:
    modifies [obj.value]
  body:
    obj.value = 42
");
    assert!(!has_code(&results, "E001"), "Should not have E001 when mutation matches modifies");
    assert!(!has_code(&results, "E002"), "Should not have E002 when mutation matches modifies");
}

// ── 12. Matching emits passes ───────────────────────────────────

#[test]
fn ive_12_emits_with_matching_emit_passes() {
    let results = verify_all("\
intent: \"Test matching emits\"
scope: test.emits
risk: low

contract notify(msg: String)
  effects:
    emits AlertEvent
  body:
    emit AlertEvent(msg)
");
    assert!(!has_code(&results, "E005"), "Should not have E005 when emit matches declared emits");
}

// ── 13. I001: recursion detected ────────────────────────────────

#[test]
fn ive_13_i001_recursion_detected() {
    // Use effects without touches_nothing_else so E003 does not fire for the self-call.
    let results = verify_all("\
intent: \"Test recursion detection\"
scope: test.recursion
risk: low

contract factorial(n: Int) -> Int
  effects:
    reads [n]
  body:
    if n <= 1:
      return 1
    return n * factorial(n - 1)
");
    assert!(has_code(&results, "I001"),
            "Expected I001 for recursive self-call, got codes: {:?}", codes(&results));
}

// ── 14. I002: deep nesting detected ─────────────────────────────

#[test]
fn ive_14_i002_deep_nesting_detected() {
    let results = verify_all("\
intent: \"Test deep nesting detection\"
scope: test.nesting
risk: low

contract deep(x: Int) -> Int
  effects:
    reads [x]
    touches_nothing_else
  body:
    if x > 0:
      if x > 1:
        if x > 2:
          if x > 3:
            return x
    return 0
");
    assert!(has_code(&results, "I002"),
            "Expected I002 for nesting depth >= 4, got codes: {:?}", codes(&results));
}

// ── 15. W005 becomes Error at HIGH risk ─────────────────────────

#[test]
fn ive_15_w005_error_at_high_risk() {
    let results = verify_all("\
intent: \"Test high risk effects requirement\"
scope: test.risk
risk: high

contract risky(x: Int) -> Int
  precondition:
    x >= 0
  postcondition:
    result >= 0
  body:
    return x * x
");
    assert!(has_code(&results, "W005"),
            "Expected W005 at HIGH risk without effects");
    assert_eq!(severity_of(&results, "W005"), Some(Severity::Error),
               "W005 at HIGH risk should be Error severity");
}

// ── 16. E005: undeclared emit ───────────────────────────────────

#[test]
fn ive_16_e005_undeclared_emit() {
    let results = verify_all("\
intent: \"Test undeclared emit\"
scope: test.events
risk: low

contract emitter(msg: String)
  effects:
    emits DeclaredEvent
  body:
    emit DeclaredEvent(msg)
    emit UndeclaredEvent(msg)
");
    assert!(has_code(&results, "E005"),
            "Expected E005 for undeclared emit of UndeclaredEvent, got codes: {:?}", codes(&results));
}

// ── 17. W001: declared modifies not observed in body ────────────

#[test]
fn ive_17_w001_declared_modifies_not_observed() {
    let results = verify_all("\
intent: \"Test modifies soundness\"
scope: test.soundness
risk: low

contract phantom_modify(obj: Record)
  effects:
    modifies [obj.value, obj.phantom]
  body:
    obj.value = 42
");
    assert!(has_code(&results, "W001"),
            "Expected W001 for declared modifies target 'obj.phantom' not observed in body, got codes: {:?}",
            codes(&results));
}

// ── 18. E002: touches_nothing_else violated by mutation ─────────

#[test]
fn ive_18_e002_touches_nothing_else_violated_by_mutation() {
    let results = verify_all("\
intent: \"Test touches nothing else mutation\"
scope: test.touches
risk: low

contract strict(obj: Record)
  effects:
    modifies [obj.x]
    touches_nothing_else
  body:
    obj.x = 1
    obj.y = 2
");
    assert!(has_code(&results, "E002"),
            "Expected E002 for touches_nothing_else violation via obj.y, got codes: {:?}", codes(&results));
    assert_eq!(severity_of(&results, "E002"), Some(Severity::Error),
               "E002 (touches_nothing_else violation) should be Error severity");
}

// ── 19. W002: declared emits not emitted ────────────────────────

#[test]
fn ive_19_w002_declared_emits_not_emitted() {
    let results = verify_all("\
intent: \"Test emits soundness\"
scope: test.soundness
risk: low

contract phantom_emit(msg: String)
  effects:
    emits PhantomEvent
  body:
    x = msg
");
    assert!(has_code(&results, "W002"),
            "Expected W002 for declared emits 'PhantomEvent' not emitted, got codes: {:?}", codes(&results));
}

// ── 20. W008: has-check references capability not in scope ──────

#[test]
fn ive_20_w008_capability_check_scope_mismatch() {
    let results = verify_all("\
intent: \"Test capability check scope\"
scope: test.scope
risk: low
requires: [auth.verified]

contract check_perms(user: User)
  effects:
    reads [user]
    touches_nothing_else
  body:
    if user has billing.admin:
      return true
    return false
");
    assert!(has_code(&results, "W008"),
            "Expected W008 for has-check of billing.admin not matching declared capabilities, got codes: {:?}",
            codes(&results));
}

// ══════════════════════════════════════════════════════════════════
//  CAPABILITY / IFC TESTS  (F001-F006)
// ══════════════════════════════════════════════════════════════════

// ── 1. No capability violations in simple contract ──────────────

#[test]
fn cap_01_no_violations_simple_contract() {
    let results = verify_caps("\
intent: \"Test capability clean\"
scope: test.capability
risk: low

contract add(a: Int, b: Int) -> Int
  body:
    return a + b
");
    let f_codes: Vec<_> = results.iter().filter(|r| r.code.starts_with('F')).collect();
    assert!(
        f_codes.is_empty(),
        "Simple contract should have no F-codes, got: {:?}",
        f_codes.iter().map(|r| &r.code).collect::<Vec<_>>()
    );
}

// ── 2. F001: information flow violation ─────────────────────────

#[test]
fn cap_02_f001_information_flow_violation() {
    let results = verify_caps("\
intent: \"Test information flow control\"
scope: test.ifc
risk: low

type SecretData = Record
  fields:
    secret: String [classified, no_log]
    public_info: String

  flow_constraints:
    never_flows_to: [log_sink, public_api]

contract leak_secret(data: SecretData) -> String
  effects:
    reads [data]
    touches_nothing_else
  body:
    log_sink.write(data.secret)
    return data.public_info
");
    assert!(has_code(&results, "F001"),
            "Expected F001 for labeled data flowing to restricted sink, got codes: {:?}", codes(&results));
}

// ── 3. F002: permission denied ──────────────────────────────────

#[test]
fn cap_03_f002_permission_denied() {
    let results = verify_caps("\
intent: \"Test permission denied\"
scope: test.permissions
risk: low

contract read_denied(record: Record)
  permissions:
    denies: [read(record.name)]
  effects:
    reads [record]
    touches_nothing_else
  body:
    name = record.name
    return name
");
    assert!(has_code(&results, "F002"),
            "Expected F002 for reading denied field record.name, got codes: {:?}", codes(&results));
}

// ── 4. F003: access not granted ─────────────────────────────────

#[test]
fn cap_04_f003_access_not_granted() {
    let results = verify_caps("\
intent: \"Test access not granted\"
scope: test.grants
risk: low

contract limited(record: Record)
  permissions:
    grants: [read(record.allowed)]
  body:
    x = record.secret
    return x
");
    assert!(has_code(&results, "F003"),
            "Expected F003 for reading record.secret not covered by grants, got codes: {:?}", codes(&results));
}

// ── 5. F004: context required ───────────────────────────────────

#[test]
fn cap_05_f004_context_required() {
    let results = verify_caps("\
intent: \"Test context required type\"
scope: test.general
risk: low

type ContextualData = Record
  fields:
    value: String

  flow_constraints:
    requires_context: medical_session

contract use_data(data: ContextualData) -> String
  body:
    return data.value
");
    assert!(has_code(&results, "F004"),
            "Expected F004 for type requiring medical_session context in non-medical scope, got codes: {:?}",
            codes(&results));
}

// ── 6. F005: capability check not declared ──────────────────────

#[test]
fn cap_06_f005_capability_check_not_declared() {
    let results = verify_caps("\
intent: \"Test capability check\"
scope: test.capability
risk: low
requires: [auth.verified]

contract check_admin(user: User)
  body:
    if user has admin.superuser:
      return true
    return false
");
    assert!(has_code(&results, "F005"),
            "Expected F005 for has-check referencing undeclared capability admin.superuser, got codes: {:?}",
            codes(&results));
}

// ── 7. F006: grant-deny conflict ────────────────────────────────

#[test]
fn cap_07_f006_grant_deny_conflict() {
    let results = verify_caps("\
intent: \"Test grant deny conflict\"
scope: test.conflict
risk: low

contract conflicting(record: Record)
  permissions:
    grants: [read(record.name)]
    denies: [read(record.name)]
  body:
    return record.name
");
    assert!(has_code(&results, "F006"),
            "Expected F006 for grant-deny conflict on read(record.name), got codes: {:?}", codes(&results));
}

// ── 8. Clean flow — type with constraints but no violations ─────

#[test]
fn cap_08_clean_flow_no_violation() {
    let results = verify_caps("\
intent: \"Test clean data flow\"
scope: test.flow
risk: low

type PublicData = Record
  fields:
    name: String
    value: Int

contract read_public(data: PublicData) -> String
  body:
    return data.name
");
    let f_codes: Vec<_> = results.iter().filter(|r| r.code.starts_with('F')).collect();
    assert!(
        f_codes.is_empty(),
        "Contract with unlabeled type should have no F-codes, got: {:?}",
        f_codes.iter().map(|r| &r.code).collect::<Vec<_>>()
    );
}

// ── 9. Permissions within grants pass ───────────────────────────

#[test]
fn cap_09_permissions_within_grants_pass() {
    let results = verify_caps("\
intent: \"Test permissions within grants\"
scope: test.permissions
risk: low

contract allowed(record: Record)
  permissions:
    grants: [read(record.name), read(record.value)]
  body:
    x = record.name
    return x
");
    assert!(!has_code(&results, "F003"),
            "Should not have F003 when read is covered by grants");
}

// ── 10. Context satisfied — scope matches requires_context ──────

#[test]
fn cap_10_context_satisfied_passes() {
    let results = verify_caps("\
intent: \"Test context satisfied\"
scope: medical.records
risk: low

type MedicalData = Record
  fields:
    diagnosis: String

  flow_constraints:
    requires_context: medical_session

contract read_diagnosis(data: MedicalData) -> String
  body:
    return data.diagnosis
");
    assert!(!has_code(&results, "F004"),
            "Should not have F004 when scope satisfies context requirement (medical in scope)");
}

// ══════════════════════════════════════════════════════════════════
//  CONTRACT VERIFICATION TESTS  (V001-V005)
// ══════════════════════════════════════════════════════════════════

// ── 1. All paths return — passes V001 ───────────────────────────

#[test]
fn cv_01_all_paths_return_passes_v001() {
    let results = verify_cv("\
intent: \"Test all paths return\"
scope: test.returns
risk: low

contract compute(n: Int) -> Int
  body:
    if n > 0:
      return n
    return 0
");
    assert!(!has_code(&results, "V001"),
            "Should not have V001 when all paths return");
}

// ── 2. V001: missing return on some path ────────────────────────

#[test]
fn cv_02_v001_missing_return_on_some_path() {
    let results = verify_cv("\
intent: \"Test missing return\"
scope: test.returns
risk: low

contract partial(n: Int) -> Int
  body:
    if n > 0:
      return n
    x = n + 1
");
    assert!(has_code(&results, "V001"),
            "Expected V001 for missing return path, got codes: {:?}", codes(&results));
}

// ── 3. V002: dead code after return ─────────────────────────────

#[test]
fn cv_03_v002_dead_code_after_return() {
    let results = verify_cv("\
intent: \"Test dead code detection\"
scope: test.dead
risk: low

contract dead(n: Int) -> Int
  body:
    return n
    x = n + 1
    return x
");
    assert!(has_code(&results, "V002"),
            "Expected V002 for dead code after return, got codes: {:?}", codes(&results));
    assert!(count_code(&results, "V002") >= 2,
            "Should detect dead code for both statements after the return");
}

// ── 4. V003: high risk contract missing on_failure ──────────────

#[test]
fn cv_04_v003_high_risk_missing_on_failure() {
    let results = verify_cv("\
intent: \"Test on_failure requirement\"
scope: test.risk
risk: high

contract risky(x: Int) -> Int
  body:
    return x * x
");
    assert!(has_code(&results, "V003"),
            "Expected V003 for high-risk contract without on_failure, got codes: {:?}", codes(&results));
}

// ── 5. V004: postcondition references result but not all paths return ──

#[test]
fn cv_05_v004_postcondition_references_result_no_return() {
    let results = verify_cv("\
intent: \"Test postcondition result check\"
scope: test.postcondition
risk: low

contract check_result(n: Int) -> Int
  postcondition:
    result > 0
  body:
    if n > 0:
      return n
    x = 0
");
    assert!(has_code(&results, "V004"),
            "Expected V004 when postcondition references 'result' but not all paths return, got codes: {:?}",
            codes(&results));
}

// ── 6. V005: shared state access without declaration ────────────

#[test]
fn cv_06_v005_shared_state_not_declared() {
    let results = verify_cv("\
intent: \"Test shared state access\"
scope: test.shared
risk: low

shared ledger: Ledger
  access: transactional
  isolation: serializable
  audit: full_history

contract read_ledger() -> Int
  body:
    return ledger.balance
");
    assert!(has_code(&results, "V005"),
            "Expected V005 for undeclared shared state access, got codes: {:?}", codes(&results));
}

// ── 7. Simple contract passes all V-checks ──────────────────────

#[test]
fn cv_07_simple_contract_passes_all() {
    let results = verify_cv("\
intent: \"Test simple clean contract\"
scope: test.simple
risk: low

contract identity(x: Int) -> Int
  body:
    return x
");
    let v_codes: Vec<_> = results.iter().filter(|r| r.code.starts_with('V')).collect();
    assert!(
        v_codes.is_empty(),
        "Simple contract should have no V-codes, got: {:?}",
        v_codes.iter().map(|r| format!("{}: {}", r.code, r.message)).collect::<Vec<_>>()
    );
}

// ── 8. on_failure present passes V003 ───────────────────────────

#[test]
fn cv_08_on_failure_present_passes_v003() {
    let results = verify_cv("\
intent: \"Test on_failure handler\"
scope: test.failure
risk: high

contract safe(x: Int) -> Int
  body:
    return x * x
  on_failure:
    return 0
");
    assert!(!has_code(&results, "V003"),
            "Should not have V003 when on_failure is present");
}

// ── 9. if/else both returning passes V001 ───────────────────────

#[test]
fn cv_09_if_else_both_return_passes() {
    let results = verify_cv("\
intent: \"Test if else both return\"
scope: test.branches
risk: low

contract branching(n: Int) -> Int
  body:
    if n > 0:
      return n
    else:
      return 0
");
    assert!(!has_code(&results, "V001"),
            "Should not have V001 when both if and else branches return");
}

// ── 10. Dead code inside nested block ───────────────────────────

#[test]
fn cv_10_dead_code_in_nested_block() {
    let results = verify_cv("\
intent: \"Test nested dead code\"
scope: test.nested
risk: low

contract nested_dead(n: Int) -> Int
  body:
    if n > 0:
      return n
      x = 1
    return 0
");
    assert!(has_code(&results, "V002"),
            "Expected V002 for dead code inside if block after return, got codes: {:?}", codes(&results));
}

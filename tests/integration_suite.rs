#![allow(clippy::unwrap_used)]

use parley::grader::evaluate_case_transcript;
use parley::lockfile::{generate_lockfile, verify_lockfile};
use parley::memory::DialogueRegime;
use parley::repo_audit::audit_repository;
use parley::simulator::{simulate_conversation, SystemProfile};
use parley::spec::SuiteSpec;
use parley::transcript::{
    load_transcript_from_jsonl, verify_transcript_chain, TurnRecord, TurnSpeaker,
};
use rstest::rstest;

// 1. Layer 1: Unit tests for Merkle chain verification
#[test]
fn test_merkle_chain_integrity() {
    let mut turns = vec![
        TurnRecord {
            turn_index: 1,
            speaker: TurnSpeaker::SimulatedUser,
            utterance: "Prompt 1".to_string(),
            input_tokens: 100,
            output_tokens: 20,
            duration_ms: 50,
            prev_turn_hash: String::new(),
            turn_hash: String::new(),
        },
        TurnRecord {
            turn_index: 2,
            speaker: TurnSpeaker::SystemUnderTest,
            utterance: "Response 1".to_string(),
            input_tokens: 150,
            output_tokens: 40,
            duration_ms: 120,
            prev_turn_hash: String::new(),
            turn_hash: String::new(),
        },
    ];

    parley::transcript::compute_transcript_chain(&mut turns);
    assert!(verify_transcript_chain(&turns).is_ok());

    // Corrupt one turn utterance and verify detection
    turns[1].utterance = "Tampered Response".to_string();
    assert!(verify_transcript_chain(&turns).is_err());
}

// 2. Layer 2: Parameterized table tests for System Profiles
#[rstest]
#[case(SystemProfile::OptimalClarifier, DialogueRegime::ResolvedOptimal, true)]
#[case(
    SystemProfile::PrematureGuesser,
    DialogueRegime::PrematureExecutionTrap,
    false
)]
#[case(
    SystemProfile::AmnesiacDrifter,
    DialogueRegime::StateAmnesiaFailure,
    false
)]
#[case(
    SystemProfile::SycophanticCaver,
    DialogueRegime::SycophanticCapitulation,
    false
)]
fn test_system_profile_regimes(
    #[case] profile: SystemProfile,
    #[case] expected_regime: DialogueRegime,
    #[case] expected_pass: bool,
) {
    let suite = SuiteSpec::from_file("packs/softfactory.toml").unwrap();
    let case = &suite.cases[0];

    let outcome = simulate_conversation(case, profile, 42).unwrap();
    let scorecard = evaluate_case_transcript(case, &outcome.turns);

    assert_eq!(scorecard.memory.regime, expected_regime);
    assert_eq!(scorecard.passed, expected_pass);
}

// 3. Layer 3: Deterministic simulation property test across PRNG seeds
#[test]
fn test_simulation_determinism_across_seeds() {
    let suite = SuiteSpec::from_file("packs/softfactory.toml").unwrap();
    let case = &suite.cases[0];

    let outcome1 = simulate_conversation(case, SystemProfile::OptimalClarifier, 12345).unwrap();
    let outcome2 = simulate_conversation(case, SystemProfile::OptimalClarifier, 12345).unwrap();

    assert_eq!(outcome1.turns.len(), outcome2.turns.len());
    for (t1, t2) in outcome1.turns.iter().zip(outcome2.turns.iter()) {
        assert_eq!(t1.turn_hash, t2.turn_hash);
        assert_eq!(t1.utterance, t2.utterance);
    }
}

// 4. Layer 4: Counterfactual negative fixture verification
#[test]
fn test_counterfactual_negative_fixtures_fail_closed() {
    let suite = SuiteSpec::from_file("packs/softfactory.toml").unwrap();

    // Case 1: Amnesia fixture
    let case0 = &suite.cases[0];
    let amnesia_turns =
        load_transcript_from_jsonl("packs/fixtures/negative_amnesia_drift.jsonl").unwrap();
    let sc_amnesia = evaluate_case_transcript(case0, &amnesia_turns);
    assert_eq!(
        sc_amnesia.memory.regime,
        DialogueRegime::StateAmnesiaFailure
    );
    assert!(!sc_amnesia.passed);

    // Case 2: Sycophancy fixture
    let case1 = &suite.cases[1];
    let sycophancy_turns =
        load_transcript_from_jsonl("packs/fixtures/negative_sycophancy_cave.jsonl").unwrap();
    let sc_sycophancy = evaluate_case_transcript(case1, &sycophancy_turns);
    assert_eq!(
        sc_sycophancy.memory.regime,
        DialogueRegime::SycophanticCapitulation
    );
    assert!(!sc_sycophancy.passed);

    // Case 3: Premature guess fixture
    let guess_turns =
        load_transcript_from_jsonl("packs/fixtures/negative_premature_guess.jsonl").unwrap();
    let sc_guess = evaluate_case_transcript(case0, &guess_turns);
    assert_eq!(
        sc_guess.memory.regime,
        DialogueRegime::PrematureExecutionTrap
    );
    assert!(!sc_guess.passed);
}

// 5. End-to-end suite, lockfile and repo hygiene check
#[test]
fn test_end_to_end_suite_and_repo_audit() {
    let suite = SuiteSpec::from_file("packs/softfactory.toml").unwrap();
    assert!(suite.validate().is_ok());

    let lock = generate_lockfile(&suite, ".").unwrap();
    assert!(verify_lockfile(&suite, &lock, ".").is_ok());

    let audit = audit_repository(".");
    assert!(audit.passed, "Audit failed: {:?}", audit.checks);
    assert_eq!(audit.passed_checks, 8);
}

#![forbid(unsafe_code)]

pub mod economics;
pub mod envelope;
pub mod grader;
pub mod ingest;
pub mod lockfile;
pub mod memory;
pub mod repo_audit;
pub mod report;
pub mod simulator;
pub mod spec;
pub mod transcript;

pub use economics::{compute_dialogue_economics, DialogueEconomics};
pub use envelope::{EnvelopeStatus, HarnessEnvelope};
pub use grader::{evaluate_case_transcript, evaluate_suite, CaseScorecard, SuiteSummary};
pub use ingest::{ingest_harness_runs, IngestedConversation};
pub use lockfile::{generate_lockfile, verify_lockfile, CaseLockEntry, ParleyLockfile};
pub use memory::{analyze_dialogue_state, DialogueRegime, MemoryAnalysis};
pub use repo_audit::{audit_repository, RepoAuditReport, RepoCheckResult};
pub use report::{
    export_otel_spans, export_reval_ratchet, generate_mermaid_sequence, generate_mermaid_xychart,
};
pub use simulator::{simulate_conversation, SimulationOutcome, SystemProfile};
pub use spec::{
    AdversarialPushback, CaseSeverity, CaseSpec, GoalCriterion, PersonaSpec, PolicyGuard,
    SuiteMeta, SuiteSpec, WithheldConstraint,
};
pub use transcript::{
    compute_single_turn_hash, compute_transcript_chain, load_transcript_from_jsonl,
    verify_transcript_chain, TranscriptError, TurnRecord, TurnSpeaker,
};

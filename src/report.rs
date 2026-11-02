use crate::grader::{CaseScorecard, SuiteSummary};
use crate::transcript::{TurnRecord, TurnSpeaker};
use std::fmt::Write as _;

pub fn generate_mermaid_sequence(scorecard: &CaseScorecard, turns: &[TurnRecord]) -> String {
    let mut out = String::new();
    out.push_str("sequenceDiagram\n");
    out.push_str("    autonumber\n");
    out.push_str("    actor User as SimulatedUser\n");
    out.push_str("    participant System as SystemUnderTest\n");

    for turn in turns {
        let snippet: String = turn
            .utterance
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(60)
            .collect();
        let sanitized = snippet.replace('"', "'");
        match turn.speaker {
            TurnSpeaker::SimulatedUser => {
                let _ = writeln!(
                    out,
                    "    User->>System: Turn {} - \"{}\"",
                    turn.turn_index, sanitized
                );
            }
            TurnSpeaker::SystemUnderTest => {
                let _ = writeln!(
                    out,
                    "    System-->>User: Turn {} - \"{}\"",
                    turn.turn_index, sanitized
                );
            }
        }
    }

    let _ = writeln!(
        out,
        "    Note over User,System: Goal Completed: {} | Turn Efficiency: {:.2} | Memory Retention: {:.2}%",
        scorecard.goal_completed,
        scorecard.turn_efficiency,
        scorecard.memory.memory_retention_rate * 100.0
    );

    out
}

pub fn generate_mermaid_xychart(decay_rate: f64, pushback_collapse: f64) -> String {
    let turns = [1, 2, 3, 4, 5, 6, 8, 10];
    let mut x_labels = Vec::new();
    let mut single_turn_series = Vec::new();
    let mut multi_turn_series = Vec::new();

    for &t in &turns {
        x_labels.push(format!("\"Turn {t}\""));
        // Single turn assumption stays at baseline 95%
        single_turn_series.push(format!("{:.1}", 95.0));

        // Multi-turn model: 0.95 * (1 - decay_rate)^(t-1) * (if t >= 3 { 1 - pushback_collapse } else { 1.0 })
        let mut prob = 0.95 * (1.0 - decay_rate).powi(t - 1);
        if t >= 3 {
            prob *= 1.0 - pushback_collapse;
        }
        multi_turn_series.push(format!("{:.1}", prob * 100.0));
    }

    format!(
        "xychart-beta\n    title \"Single-Turn Apparent Pass vs Multi-Turn State Retention & Policy Survival\"\n    x-axis [{}]\n    y-axis \"Probability (%)\" 0 --> 100\n    line [{}]\n    line [{}]\n",
        x_labels.join(", "),
        single_turn_series.join(", "),
        multi_turn_series.join(", ")
    )
}

pub fn export_otel_spans(
    summary: &SuiteSummary,
    scorecards: &[CaseScorecard],
) -> serde_json::Value {
    let mut spans = Vec::new();

    for sc in scorecards {
        spans.push(serde_json::json!({
            "trace_id": format!("{:x}", md5_hash(&sc.case_id)),
            "span_id": format!("{:x}", md5_hash(&sc.title)),
            "name": format!("parley.case.{}", sc.case_id),
            "kind": "AGENT",
            "attributes": {
                "eval.case_id": sc.case_id,
                "eval.passed": sc.passed,
                "eval.goal_completed": sc.goal_completed,
                "eval.goal_score": sc.goal_score,
                "eval.total_turns": sc.total_turns,
                "eval.turn_efficiency": sc.turn_efficiency,
                "eval.memory_retention_rate": sc.memory.memory_retention_rate,
                "eval.sycophancy_resistance_rate": sc.memory.sycophancy_resistance_rate,
                "eval.regime": format!("{:?}", sc.memory.regime),
                "eval.stratum": sc.stratum,
                "openinference.span.kind": "AGENT"
            }
        }));
    }

    serde_json::json!({
        "resource": {
            "attributes": {
                "service.name": "tideos.parley",
                "service.version": "0.1.0",
                "suite.id": summary.suite_id,
                "suite.all_passed": summary.all_passed
            }
        },
        "spans": spans
    })
}

pub fn export_reval_ratchet(summary: &SuiteSummary) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "reval-ratchet-v1",
        "tool": "parley",
        "suite_id": summary.suite_id,
        "metrics": {
            "macro_avg_goal_score": summary.macro_avg_goal_score,
            "macro_avg_turn_efficiency": summary.macro_avg_turn_efficiency,
            "macro_avg_memory_retention": summary.macro_avg_memory_retention,
            "macro_avg_sycophancy_resistance": summary.macro_avg_sycophancy_resistance
        },
        "blocker_failures": summary.blocker_failures
    })
}

fn md5_hash(s: &str) -> u64 {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let res = h.finalize();
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&res[0..8]);
    u64::from_be_bytes(buf)
}

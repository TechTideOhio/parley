use serde::{Deserialize, Serialize};

use crate::transcript::TurnRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueEconomics {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_duration_ms: u64,
    pub context_accumulation_penalty_ratio: f64,
    pub estimated_cost_usd: f64,
}

pub fn compute_dialogue_economics(turns: &[TurnRecord]) -> DialogueEconomics {
    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut total_duration = 0u64;

    for turn in turns {
        total_input += turn.input_tokens as u64;
        total_output += turn.output_tokens as u64;
        total_duration += turn.duration_ms;
    }

    // In multi-turn dialogues, input context accumulates linearly/quadratically.
    // If there are N turns, single-turn would take only N*first_turn_input.
    let base_input = turns.first().map(|t| t.input_tokens as u64).unwrap_or(100);
    let naive_flat_input = base_input * turns.len().max(1) as u64;
    let context_penalty = if naive_flat_input > 0 {
        total_input as f64 / naive_flat_input as f64
    } else {
        1.0
    };

    // Estimated standard cost: $3.00 / 1M input, $15.00 / 1M output
    let cost = (total_input as f64 * 0.000003) + (total_output as f64 * 0.000015);

    DialogueEconomics {
        total_input_tokens: total_input,
        total_output_tokens: total_output,
        total_duration_ms: total_duration,
        context_accumulation_penalty_ratio: context_penalty,
        estimated_cost_usd: cost,
    }
}

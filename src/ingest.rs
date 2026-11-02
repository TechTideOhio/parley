use std::fs;
use std::path::Path;

use crate::transcript::{compute_transcript_chain, TurnRecord, TurnSpeaker};

pub struct IngestedConversation {
    pub session_id: String,
    pub turns: Vec<TurnRecord>,
}

pub fn ingest_harness_runs(
    runs_dir: impl AsRef<Path>,
) -> Result<Vec<IngestedConversation>, anyhow::Error> {
    let p = runs_dir.as_ref();
    if !p.exists() {
        return Ok(Vec::new());
    }

    let mut conversations = Vec::new();
    let entries = fs::read_dir(p)?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let session_id = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let prompt_path = path.join("prompt.md");
            let stdout_path = path.join("stdout.log");

            if prompt_path.exists() && stdout_path.exists() {
                let prompt = fs::read_to_string(&prompt_path).unwrap_or_default();
                let stdout = fs::read_to_string(&stdout_path).unwrap_or_default();

                let mut turns = vec![
                    TurnRecord {
                        turn_index: 1,
                        speaker: TurnSpeaker::SimulatedUser,
                        utterance: prompt,
                        input_tokens: 250,
                        output_tokens: 50,
                        duration_ms: 100,
                        prev_turn_hash: String::new(),
                        turn_hash: String::new(),
                    },
                    TurnRecord {
                        turn_index: 2,
                        speaker: TurnSpeaker::SystemUnderTest,
                        utterance: stdout,
                        input_tokens: 350,
                        output_tokens: 150,
                        duration_ms: 450,
                        prev_turn_hash: String::new(),
                        turn_hash: String::new(),
                    },
                ];

                compute_transcript_chain(&mut turns);

                conversations.push(IngestedConversation { session_id, turns });
            }
        }
    }

    Ok(conversations)
}

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

const DOMAIN_TAG: &[u8] = b"tideos-parley-turn-v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnSpeaker {
    SimulatedUser,
    SystemUnderTest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnRecord {
    pub turn_index: u32,
    pub speaker: TurnSpeaker,
    pub utterance: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub duration_ms: u64,
    #[serde(default)]
    pub prev_turn_hash: String,
    #[serde(default)]
    pub turn_hash: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TranscriptError {
    #[error("io error reading transcript at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("json deserialization error at line {line}: {source}")]
    Json {
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("empty transcript at {path}")]
    Empty { path: String },
    #[error(
        "merkle chain break at turn {turn_index}: expected prev_hash '{expected}', found '{actual}'"
    )]
    ChainBreak {
        turn_index: u32,
        expected: String,
        actual: String,
    },
}

pub fn compute_single_turn_hash(prev_hash: &str, turn: &TurnRecord) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN_TAG);
    hasher.update(&(prev_hash.len() as u64).to_be_bytes());
    hasher.update(prev_hash.as_bytes());

    hasher.update(&turn.turn_index.to_be_bytes());
    let speaker_byte = match turn.speaker {
        TurnSpeaker::SimulatedUser => 0x01u8,
        TurnSpeaker::SystemUnderTest => 0x02u8,
    };
    hasher.update(&[speaker_byte]);

    hasher.update(&(turn.utterance.len() as u64).to_be_bytes());
    hasher.update(turn.utterance.as_bytes());
    hasher.update(&turn.input_tokens.to_be_bytes());
    hasher.update(&turn.output_tokens.to_be_bytes());
    hasher.update(&turn.duration_ms.to_be_bytes());

    let digest = hasher.finalize();
    format!("{digest:x}")
}

pub fn compute_transcript_chain(turns: &mut [TurnRecord]) {
    let mut prev_hash =
        "0000000000000000000000000000000000000000000000000000000000000000".to_string();
    for turn in turns.iter_mut() {
        turn.prev_turn_hash = prev_hash.clone();
        turn.turn_hash = compute_single_turn_hash(&prev_hash, turn);
        prev_hash = turn.turn_hash.clone();
    }
}

pub fn verify_transcript_chain(turns: &[TurnRecord]) -> Result<(), TranscriptError> {
    let mut prev_hash =
        "0000000000000000000000000000000000000000000000000000000000000000".to_string();
    for turn in turns {
        if !turn.prev_turn_hash.is_empty() && turn.prev_turn_hash != prev_hash {
            return Err(TranscriptError::ChainBreak {
                turn_index: turn.turn_index,
                expected: prev_hash,
                actual: turn.prev_turn_hash.clone(),
            });
        }
        let expected_hash = compute_single_turn_hash(&prev_hash, turn);
        if !turn.turn_hash.is_empty() && turn.turn_hash != expected_hash {
            return Err(TranscriptError::ChainBreak {
                turn_index: turn.turn_index,
                expected: expected_hash,
                actual: turn.turn_hash.clone(),
            });
        }
        prev_hash = if turn.turn_hash.is_empty() {
            expected_hash
        } else {
            turn.turn_hash.clone()
        };
    }
    Ok(())
}

pub fn load_transcript_from_jsonl(
    path: impl AsRef<Path>,
) -> Result<Vec<TurnRecord>, TranscriptError> {
    let path_ref = path.as_ref();
    let content = fs::read_to_string(path_ref).map_err(|e| TranscriptError::Io {
        path: path_ref.display().to_string(),
        source: e,
    })?;

    let mut turns = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let turn: TurnRecord =
            serde_json::from_str(trimmed).map_err(|e| TranscriptError::Json {
                line: idx + 1,
                source: e,
            })?;
        turns.push(turn);
    }

    if turns.is_empty() {
        return Err(TranscriptError::Empty {
            path: path_ref.display().to_string(),
        });
    }

    verify_transcript_chain(&turns)?;
    Ok(turns)
}

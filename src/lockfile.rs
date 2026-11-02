use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::spec::SuiteSpec;

const DOMAIN_TAG: &[u8] = b"tideos-parley-lock-v1\0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseLockEntry {
    pub case_id: String,
    pub stratum: String,
    pub transcript_path: String,
    pub transcript_sha256: String,
    pub persona_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParleyLockfile {
    pub schema_version: String,
    pub suite_id: String,
    pub suite_version: String,
    pub overall_lock_digest: String,
    pub cases: BTreeMap<String, CaseLockEntry>,
}

pub fn hash_file_contents(path: impl AsRef<Path>) -> Result<String, std::io::Error> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn hash_persona(case: &crate::spec::CaseSpec) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN_TAG);
    hasher.update(case.persona.id.as_bytes());
    hasher.update(case.persona.role.as_bytes());
    hasher.update(case.persona.hidden_goal.as_bytes());
    hasher.update(case.persona.opening_prompt.as_bytes());
    for c in &case.persona.withheld_constraints {
        hasher.update(c.id.as_bytes());
        hasher.update(c.unlock_on_assistant_regex.as_bytes());
        hasher.update(c.must_hold_regex.as_bytes());
    }
    for pb in &case.persona.adversarial_pushbacks {
        hasher.update(&pb.inject_at_turn.to_be_bytes());
        hasher.update(pb.utterance.as_bytes());
        hasher.update(pb.forbidden_capitulation_regex.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

pub fn generate_lockfile(
    suite: &SuiteSpec,
    root: impl AsRef<Path>,
) -> Result<ParleyLockfile, anyhow::Error> {
    let root_ref = root.as_ref();
    let mut case_entries = BTreeMap::new();
    let mut overall_hasher = Sha256::new();
    overall_hasher.update(DOMAIN_TAG);
    overall_hasher.update(suite.suite.id.as_bytes());
    overall_hasher.update(suite.suite.version.as_bytes());

    for case in &suite.cases {
        let transcript_abs = root_ref.join(&case.transcript_path);
        let transcript_sha256 = hash_file_contents(&transcript_abs)?;
        let persona_hash = hash_persona(case);

        let entry = CaseLockEntry {
            case_id: case.id.clone(),
            stratum: case.stratum.clone(),
            transcript_path: case.transcript_path.clone(),
            transcript_sha256: transcript_sha256.clone(),
            persona_hash: persona_hash.clone(),
        };

        overall_hasher.update(&(entry.case_id.len() as u64).to_be_bytes());
        overall_hasher.update(entry.case_id.as_bytes());
        overall_hasher.update(entry.transcript_sha256.as_bytes());
        overall_hasher.update(entry.persona_hash.as_bytes());

        case_entries.insert(case.id.clone(), entry);
    }

    let overall_lock_digest = format!("{:x}", overall_hasher.finalize());

    Ok(ParleyLockfile {
        schema_version: "sha256-framed-v1".to_string(),
        suite_id: suite.suite.id.clone(),
        suite_version: suite.suite.version.clone(),
        overall_lock_digest,
        cases: case_entries,
    })
}

pub fn verify_lockfile(
    suite: &SuiteSpec,
    lock: &ParleyLockfile,
    root: impl AsRef<Path>,
) -> Result<(), anyhow::Error> {
    if suite.suite.id != lock.suite_id {
        anyhow::bail!(
            "lockfile suite_id '{}' does not match suite '{}'",
            lock.suite_id,
            suite.suite.id
        );
    }

    let regenerated = generate_lockfile(suite, root)?;
    if regenerated.overall_lock_digest != lock.overall_lock_digest {
        anyhow::bail!(
            "lockfile overall digest mismatch: expected '{}', found '{}'",
            lock.overall_lock_digest,
            regenerated.overall_lock_digest
        );
    }

    Ok(())
}

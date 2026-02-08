use sha2::{Digest, Sha256};

use crate::ast::ContractDef;
use crate::verify::fingerprint::{fingerprint_contract, BehavioralFingerprint};

#[derive(Debug, Clone)]
pub struct IntentHash {
    pub contract_name: String,
    pub intent_text: String,
    pub intent_hash: String,
    pub fingerprint_hash: String,
    pub combined_hash: String,
}

impl IntentHash {
    pub fn verify_against(&self, other: &IntentHash) -> IntentHashComparison {
        IntentHashComparison {
            contract_name: self.contract_name.clone(),
            intent_changed: self.intent_hash != other.intent_hash,
            behavior_changed: self.fingerprint_hash != other.fingerprint_hash,
            combined_match: self.combined_hash == other.combined_hash,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IntentHashComparison {
    pub contract_name: String,
    pub intent_changed: bool,
    pub behavior_changed: bool,
    pub combined_match: bool,
}

impl IntentHashComparison {
    pub fn is_drift(&self) -> bool {
        self.behavior_changed && !self.intent_changed
    }

    pub fn is_consistent(&self) -> bool {
        self.combined_match || (self.intent_changed && self.behavior_changed)
    }

    pub fn describe(&self) -> String {
        if self.combined_match {
            format!("contract '{}': no change", self.contract_name)
        } else if self.is_drift() {
            format!(
                "contract '{}': SEMANTIC DRIFT DETECTED — behavior changed but intent declaration was not updated",
                self.contract_name
            )
        } else if self.intent_changed && !self.behavior_changed {
            format!(
                "contract '{}': intent updated but behavior unchanged — verify intent still matches implementation",
                self.contract_name
            )
        } else {
            format!(
                "contract '{}': both intent and behavior changed — verify consistency",
                self.contract_name
            )
        }
    }
}

pub fn compute_intent_hash(
    contract: &ContractDef,
    intent_text: &str,
    fingerprint: Option<&BehavioralFingerprint>,
) -> IntentHash {
    let owned_fp;
    let fp = match fingerprint {
        Some(f) => f,
        None => {
            owned_fp = fingerprint_contract(contract);
            &owned_fp
        }
    };

    // Hash the intent text
    let intent_hash = sha256_hex(intent_text.as_bytes());

    // Hash the behavioral fingerprint (canonical JSON for determinism)
    let fp_json =
        serde_json::to_string(&fp.to_canonical_dict()).unwrap_or_else(|_| "{}".to_string());
    let fingerprint_hash = sha256_hex(fp_json.as_bytes());

    // Combined hash: SHA-256(intent_hash || fingerprint_hash)
    let combined_input = format!("{}{}", intent_hash, fingerprint_hash);
    let combined_hash = sha256_hex(combined_input.as_bytes());

    IntentHash {
        contract_name: contract.name.clone(),
        intent_text: intent_text.to_string(),
        intent_hash,
        fingerprint_hash,
        combined_hash,
    }
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

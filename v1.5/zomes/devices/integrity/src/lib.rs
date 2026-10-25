//! Flowsta Identity DNA v1.5 - devices integrity zome.
//!
//! Three public records that let a device prove its standing without the
//! API (planning/DEVICE_CERTIFICATES_0_7.md § 4):
//!
//!   - `EnrollmentKey`: identity I has an enrollment key E from `effective_at`.
//!     The first record for an identity takes effect at once; every later one
//!     seven days after it was created (the delay that stops a thief holding
//!     one device from making their phrase the key). A later record names the
//!     one before it, which is how validation knows it is not the first.
//!   - `EnrollmentCancelled`: a record above cancelled before it took effect.
//!   - `DeviceRemoved`: identity I removed the device whose key hashes to
//!     `device_hash`, at `at`; signed by I, by an enrollment key in force, or
//!     by another device with its certificate attached. A permanent tombstone.
//!
//! What this publishes: that I has an enrollment key, that I removed *a*
//! device at a time. Never a device's key in the clear, never a device count
//! (§ 7, the pseudonymity rule). Certificates are never written here.
//!
//! Statements are the 1.6.0 registry statements, carried over as they are,
//! so the Vault's builders and the API's checks keep working; keys inside
//! them are spelled as standard base64 of the 32 core bytes, times in Unix
//! milliseconds:
//!   flowsta-enrollment-register:v1:<I>:<E>:<created_at>        signed by I
//!   flowsta-enrollment-cancel:v1:<I>:<E>:<at>                  signed by I
//!   flowsta-device-register:v1:<I>:<C>:<install_id>:<capabilities>:<issued_at>   the certificate, signed by I (+ E or a co-signer)
//!   flowsta-device-removed:v1:<I>:<device_hash hex>:<at>       signed by I, E or C
//! The removal statement is new: it names the device by hash, since the
//! record must not carry the key.
//!
//! Validation decides what it can decide deterministically: signatures,
//! the delay rule through the `previous` chain, cancellation before effect,
//! a certificate's own signature. What needs a lookup (is E in force at
//! `at`; is the co-signing device itself removed) is the verifier rule in
//! the readers (§ 6), built on `get_*` in the coordinator.
use base64::Engine;
use hdi::prelude::*;
use sha2::{Digest, Sha256};

pub const ENROLLMENT_PREFIX: &str = "flowsta-enrollment-register:v1:";
pub const ENROLLMENT_CANCEL_PREFIX: &str = "flowsta-enrollment-cancel:v1:";
pub const REGISTER_PREFIX: &str = "flowsta-device-register:v1:";
pub const REMOVED_PREFIX: &str = "flowsta-device-removed:v1:";
pub const SEVEN_DAYS_MS: i64 = 7 * 24 * 60 * 60 * 1000;

// ── Entry types ─────────────────────────────────────────────────────

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct EnrollmentKey {
    /// The identity (its network key; the DID's key).
    pub identity: AgentPubKey,
    /// The enrollment key's public half (32 bytes).
    pub enrollment_key: Vec<u8>,
    /// When the record was made (Unix ms); the time inside the statement.
    pub created_at: i64,
    /// When it takes effect: `created_at` for the identity's first record,
    /// `created_at + 7 days` for every later one.
    pub effective_at: i64,
    /// The identity's previous EnrollmentKey record, when this is not the
    /// first (the chain validation walks).
    pub previous: Option<ActionHash>,
    /// I's Ed25519 signature over the enrollment statement (64 bytes).
    pub identity_signature: Vec<u8>,
}

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct EnrollmentCancelled {
    pub identity: AgentPubKey,
    /// The EnrollmentKey record this cancels.
    pub enrollment_record: ActionHash,
    pub enrollment_key: Vec<u8>,
    /// Unix ms; must be before the record's `effective_at`.
    pub at: i64,
    /// I's signature over the cancel statement.
    pub identity_signature: Vec<u8>,
}

/// A device certificate as the 1.6.0 registration statement with its
/// signatures: presented, never written on its own; attached to a removal
/// a device signs.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct DeviceCertificate {
    pub identity: AgentPubKey,
    /// The device key's 32 bytes.
    pub device_key: Vec<u8>,
    pub install_id: String,
    /// Comma-separated, sorted, as in the statement (e.g. "approve,login,read,sign").
    pub capabilities: String,
    pub issued_at: i64,
    pub identity_signature: Vec<u8>,
    /// E's co-signature when an enrollment key was in force at issue.
    pub enrollment_signature: Option<Vec<u8>>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum RemovalSigner {
    /// Signed by the identity key (any device, Phase A).
    Identity,
    /// Signed by the enrollment key of the named EnrollmentKey record.
    Enrollment { enrollment_record: ActionHash },
    /// Signed by a device's key, its certificate attached; the device
    /// becomes public by this act.
    Device { certificate: DeviceCertificate },
}

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct DeviceRemoved {
    pub identity: AgentPubKey,
    /// SHA-256 of the removed device key's 32 bytes.
    pub device_hash: Vec<u8>,
    /// Unix ms; authoritative for "in force".
    pub at: i64,
    pub signer: RemovalSigner,
    /// The signer's Ed25519 signature over the removal statement.
    pub signature: Vec<u8>,
}

#[hdk_entry_types]
#[unit_enum(UnitEntryTypes)]
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EntryTypes {
    EnrollmentKey(EnrollmentKey),
    EnrollmentCancelled(EnrollmentCancelled),
    DeviceRemoved(DeviceRemoved),
}

#[derive(Serialize, Deserialize)]
#[hdk_link_types]
pub enum LinkTypes {
    /// identity key -> EnrollmentKey record
    IdentityToEnrollmentKeys,
    /// identity key -> EnrollmentCancelled record
    IdentityToCancellations,
    /// identity key -> DeviceRemoved record
    IdentityToRemovals,
    /// device_hash (as an ExternalHash) -> DeviceRemoved record: a verifier
    /// hashes the key it was shown and looks it up
    RemovedDeviceIndex,
}

// ── Statements ──────────────────────────────────────────────────────

/// Standard base64 of a key's 32 core bytes, as the Vault and the API spell keys in statements.
pub fn key32(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// SHA-256 of a device key's 32 bytes: how a removal names the device.
pub fn device_hash_of(device_key_32: &[u8]) -> Vec<u8> {
    Sha256::digest(device_key_32).to_vec()
}

pub fn enrollment_statement(identity: &AgentPubKey, enrollment_key: &[u8], created_at: i64) -> String {
    format!("{ENROLLMENT_PREFIX}{}:{}:{}", key32(identity.get_raw_32()), key32(enrollment_key), created_at)
}

pub fn enrollment_cancel_statement(identity: &AgentPubKey, enrollment_key: &[u8], at: i64) -> String {
    format!("{ENROLLMENT_CANCEL_PREFIX}{}:{}:{}", key32(identity.get_raw_32()), key32(enrollment_key), at)
}

pub fn certificate_statement(c: &DeviceCertificate) -> String {
    format!(
        "{REGISTER_PREFIX}{}:{}:{}:{}:{}",
        key32(c.identity.get_raw_32()),
        key32(&c.device_key),
        c.install_id,
        c.capabilities,
        c.issued_at
    )
}

pub fn removal_statement(identity: &AgentPubKey, device_hash: &[u8], at: i64) -> String {
    format!("{REMOVED_PREFIX}{}:{}:{}", key32(identity.get_raw_32()), hex(device_hash), at)
}

/// The link base a removal is indexed under.
pub fn removed_device_base(device_hash: &[u8]) -> ExternalHash {
    let mut bytes = Vec::with_capacity(36);
    bytes.extend_from_slice(&[0x84, 0x24, 0x24]);
    bytes.extend_from_slice(&device_hash[..32]);
    bytes.push(0x00);
    ExternalHash::from_raw_36(bytes)
}

/// A raw Ed25519 check against a 32-byte verify key (any DHT location).
fn verifies(verify_key_32: &[u8], signature: &[u8], statement: &str) -> ExternResult<bool> {
    if verify_key_32.len() != 32 || signature.len() != 64 {
        return Ok(false);
    }
    let key = AgentPubKey::from_raw_32(verify_key_32.to_vec());
    let sig = Signature::from(<[u8; 64]>::try_from(signature).map_err(|_| wasm_error!("signature length"))?);
    verify_signature_raw(key, sig, statement.as_bytes().to_vec())
}

// ── Validation ──────────────────────────────────────────────────────

#[cfg_attr(not(feature = "integrity"), hdk_extern)]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op.flattened::<EntryTypes, LinkTypes>()? {
        FlatOp::CreateEntry(op_entry) => match op_entry {
            OpEntry::CreateEntry { app_entry, .. } => match app_entry {
                EntryTypes::EnrollmentKey(e) => validate_enrollment_key(&e),
                EntryTypes::EnrollmentCancelled(e) => validate_enrollment_cancelled(&e),
                EntryTypes::DeviceRemoved(e) => validate_device_removed(&e),
            },
            // None of these records is ever updated; a change is a new record.
            OpEntry::UpdateEntry { .. } => Ok(ValidateCallbackResult::Invalid(
                "device records cannot be updated".to_string(),
            )),
            _ => Ok(ValidateCallbackResult::Valid),
        },
        // A removal is a permanent tombstone; an enrollment record is history.
        FlatOp::Delete(OpDelete { .. }) => Ok(ValidateCallbackResult::Invalid(
            "device records cannot be deleted".to_string(),
        )),
        FlatOp::Link(OpLink::CreateLink { link_type, action }) => match link_type {
            LinkTypes::RemovedDeviceIndex => {
                // The index must start from the hash the record names.
                let target = match ActionHash::try_from(action.data.target_address.clone()) {
                    Ok(h) => h,
                    Err(_) => return Ok(ValidateCallbackResult::Invalid("RemovedDeviceIndex must point at a record".to_string())),
                };
                let record = must_get_valid_record(target)?;
                let removed: DeviceRemoved = match record.entry().to_app_option::<DeviceRemoved>() {
                    Ok(Some(r)) => r,
                    _ => return Ok(ValidateCallbackResult::Invalid("RemovedDeviceIndex must point at a DeviceRemoved record".to_string())),
                };
                if action.data.base_address != AnyLinkableHash::from(removed_device_base(&removed.device_hash)) {
                    return Ok(ValidateCallbackResult::Invalid("RemovedDeviceIndex must start from the removed device's hash".to_string()));
                }
                Ok(ValidateCallbackResult::Valid)
            }
            LinkTypes::IdentityToEnrollmentKeys | LinkTypes::IdentityToCancellations | LinkTypes::IdentityToRemovals => {
                // The link must start from the identity the record names.
                let target = match ActionHash::try_from(action.data.target_address.clone()) {
                    Ok(h) => h,
                    Err(_) => return Ok(ValidateCallbackResult::Invalid("identity links must point at a record".to_string())),
                };
                let record = must_get_valid_record(target)?;
                match record_identity(&record) {
                    Some(i) => {
                        if action.data.base_address == AnyLinkableHash::from(i) {
                            Ok(ValidateCallbackResult::Valid)
                        } else {
                            Ok(ValidateCallbackResult::Invalid("an identity link must start from the identity the record names".to_string()))
                        }
                    }
                    None => Ok(ValidateCallbackResult::Invalid("an identity link must point at a device record".to_string())),
                }
            }
        },
        FlatOp::Link(OpLink::DeleteLink { .. }) => Ok(ValidateCallbackResult::Invalid(
            "device record links cannot be removed".to_string(),
        )),
        _ => Ok(ValidateCallbackResult::Valid),
    }
}

/// The identity named by any of the three records.
fn record_identity(record: &Record) -> Option<AgentPubKey> {
    if let Ok(Some(e)) = record.entry().to_app_option::<EnrollmentKey>() { return Some(e.identity); }
    if let Ok(Some(e)) = record.entry().to_app_option::<EnrollmentCancelled>() { return Some(e.identity); }
    if let Ok(Some(e)) = record.entry().to_app_option::<DeviceRemoved>() { return Some(e.identity); }
    None
}

fn invalid(reason: &str) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Invalid(reason.to_string()))
}

/// Valid when: signed by I; `effective_at = created_at` for the identity's
/// first record (no `previous`), else `previous` names the identity's earlier
/// record and `effective_at = created_at + 7 days`, with `created_at` later
/// than the earlier record's.
fn validate_enrollment_key(e: &EnrollmentKey) -> ExternResult<ValidateCallbackResult> {
    if e.enrollment_key.len() != 32 { return invalid("enrollment_key must be 32 bytes"); }
    if e.created_at <= 0 { return invalid("created_at must be a positive timestamp"); }
    let statement = enrollment_statement(&e.identity, &e.enrollment_key, e.created_at);
    if !verifies(e.identity.get_raw_32(), &e.identity_signature, &statement)? {
        return invalid("the enrollment statement is not signed by the identity");
    }
    match &e.previous {
        None => {
            if e.effective_at != e.created_at { return invalid("the first enrollment key takes effect when it is created"); }
        }
        Some(prev_hash) => {
            let prev = must_get_valid_record(prev_hash.clone())?;
            let prev: EnrollmentKey = match prev.entry().to_app_option::<EnrollmentKey>() {
                Ok(Some(p)) => p,
                _ => return invalid("previous must name an EnrollmentKey record"),
            };
            if prev.identity != e.identity { return invalid("previous must belong to the same identity"); }
            if e.created_at <= prev.created_at { return invalid("a later enrollment key must be created after the previous one"); }
            if e.effective_at != e.created_at + SEVEN_DAYS_MS { return invalid("a later enrollment key takes effect seven days after it is created"); }
        }
    }
    Ok(ValidateCallbackResult::Valid)
}

/// Valid when: signed by I; `at` is before the named record's `effective_at`;
/// the record belongs to the same identity and the same key.
fn validate_enrollment_cancelled(e: &EnrollmentCancelled) -> ExternResult<ValidateCallbackResult> {
    let record = must_get_valid_record(e.enrollment_record.clone())?;
    let target: EnrollmentKey = match record.entry().to_app_option::<EnrollmentKey>() {
        Ok(Some(t)) => t,
        _ => return invalid("enrollment_record must name an EnrollmentKey record"),
    };
    if target.identity != e.identity || target.enrollment_key != e.enrollment_key {
        return invalid("a cancellation must name the record's identity and key");
    }
    if e.at >= target.effective_at { return invalid("an enrollment key can only be cancelled before it takes effect"); }
    let statement = enrollment_cancel_statement(&e.identity, &e.enrollment_key, e.at);
    if !verifies(e.identity.get_raw_32(), &e.identity_signature, &statement)? {
        return invalid("the cancel statement is not signed by the identity");
    }
    Ok(ValidateCallbackResult::Valid)
}

/// Valid when: signed by I; or by the enrollment key of a named record of
/// the same identity (whether it is in force at `at` is the readers' rule);
/// or by a device whose attached certificate carries I's signature and whose
/// key signs the removal (whether that device is itself removed is the
/// readers' rule).
fn validate_device_removed(e: &DeviceRemoved) -> ExternResult<ValidateCallbackResult> {
    if e.device_hash.len() != 32 { return invalid("device_hash must be 32 bytes"); }
    if e.at <= 0 { return invalid("at must be a positive timestamp"); }
    let statement = removal_statement(&e.identity, &e.device_hash, e.at);
    match &e.signer {
        RemovalSigner::Identity => {
            if !verifies(e.identity.get_raw_32(), &e.signature, &statement)? {
                return invalid("the removal is not signed by the identity");
            }
        }
        RemovalSigner::Enrollment { enrollment_record } => {
            let record = must_get_valid_record(enrollment_record.clone())?;
            let key: EnrollmentKey = match record.entry().to_app_option::<EnrollmentKey>() {
                Ok(Some(k)) => k,
                _ => return invalid("enrollment_record must name an EnrollmentKey record"),
            };
            if key.identity != e.identity { return invalid("the enrollment key must belong to the identity"); }
            if e.at < key.effective_at { return invalid("the enrollment key was not yet in force at the removal"); }
            if !verifies(&key.enrollment_key, &e.signature, &statement)? {
                return invalid("the removal is not signed by the enrollment key");
            }
        }
        RemovalSigner::Device { certificate } => {
            if certificate.identity != e.identity { return invalid("the certificate must be the identity's"); }
            if certificate.device_key.len() != 32 { return invalid("certificate device_key must be 32 bytes"); }
            if certificate.install_id.is_empty() || certificate.install_id.len() > 64 || certificate.install_id.contains(':') {
                return invalid("certificate install_id must be 1 to 64 characters without ':'");
            }
            if certificate.capabilities.len() > 64 { return invalid("certificate capabilities too long"); }
            let cert_statement = certificate_statement(certificate);
            if !verifies(e.identity.get_raw_32(), &certificate.identity_signature, &cert_statement)? {
                return invalid("the certificate is not signed by the identity");
            }
            if device_hash_of(&certificate.device_key) == e.device_hash {
                return invalid("a device cannot remove itself");
            }
            if !verifies(&certificate.device_key, &e.signature, &statement)? {
                return invalid("the removal is not signed by the certified device");
            }
        }
    }
    Ok(ValidateCallbackResult::Valid)
}

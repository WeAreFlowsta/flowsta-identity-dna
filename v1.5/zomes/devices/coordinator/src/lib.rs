//! Flowsta Identity DNA v1.5 - devices coordinator zome.
//!
//! Writes the three device records (the statements are signed by the Vault
//! app-side, where the identity and enrollment keys live; the conductor's
//! own key only authors) and answers the lookups the verifier rule needs:
//! an identity's enrollment keys and cancellations, its removals, and
//! whether a given device key was removed.
use devices_integrity::*;
use hdk::prelude::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityDevices(devices_integrity::EntryTypes),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RegisterEnrollmentKeyInput {
    pub identity: AgentPubKey,
    pub enrollment_key: Vec<u8>,
    pub created_at: i64,
    /// The identity's previous EnrollmentKey record, when this is not the first.
    pub previous: Option<ActionHash>,
    pub identity_signature: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CancelEnrollmentKeyInput {
    pub identity: AgentPubKey,
    pub enrollment_record: ActionHash,
    pub enrollment_key: Vec<u8>,
    pub at: i64,
    pub identity_signature: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RemoveDeviceInput {
    pub identity: AgentPubKey,
    /// The removed device key's 32 bytes; hashed here, never stored.
    pub device_key: Vec<u8>,
    pub at: i64,
    pub signer: RemovalSigner,
    pub signature: Vec<u8>,
}

/// The enrollment key in force for an identity at a time, resolved by the
/// readers' rule: the effective, uncancelled record with the latest
/// `effective_at` not after `at`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EnrollmentKeyInForce {
    pub record: ActionHash,
    pub enrollment_key: Vec<u8>,
    pub effective_at: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InForceInput {
    pub identity: AgentPubKey,
    pub at: i64,
}

fn record_hash_from(link: Link) -> Option<ActionHash> {
    ActionHash::try_from(link.target).ok()
}

fn records_linked_from(identity: &AgentPubKey, link_type: LinkTypes) -> ExternResult<Vec<Record>> {
    let base = AgentPubKey::from_raw_32(identity.get_raw_32().to_vec());
    let links = get_links(LinkQuery::try_new(base, link_type)?, GetStrategy::default())?;
    let mut out = Vec::new();
    for link in links {
        if let Some(hash) = record_hash_from(link) {
            if let Some(record) = get(hash, GetOptions::default())? {
                out.push(record);
            }
        }
    }
    Ok(out)
}

/// Where an identity's links start: its key re-derived for this DNA's
/// location bytes, so a key from any context finds the same base.
fn identity_base(identity: &AgentPubKey) -> AgentPubKey {
    AgentPubKey::from_raw_32(identity.get_raw_32().to_vec())
}

#[hdk_extern]
pub fn register_enrollment_key(input: RegisterEnrollmentKeyInput) -> ExternResult<ActionHash> {
    let effective_at = match input.previous {
        None => input.created_at,
        Some(_) => input.created_at + SEVEN_DAYS_MS,
    };
    let entry = EnrollmentKey {
        identity: identity_base(&input.identity),
        enrollment_key: input.enrollment_key,
        created_at: input.created_at,
        effective_at,
        previous: input.previous,
        identity_signature: input.identity_signature,
    };
    let hash = create_entry(&EntryZomes::IntegrityDevices(EntryTypes::EnrollmentKey(entry.clone())))?;
    create_link(entry.identity.clone(), hash.clone(), LinkTypes::IdentityToEnrollmentKeys, ())?;
    Ok(hash)
}

#[hdk_extern]
pub fn cancel_enrollment_key(input: CancelEnrollmentKeyInput) -> ExternResult<ActionHash> {
    let entry = EnrollmentCancelled {
        identity: identity_base(&input.identity),
        enrollment_record: input.enrollment_record,
        enrollment_key: input.enrollment_key,
        at: input.at,
        identity_signature: input.identity_signature,
    };
    let hash = create_entry(&EntryZomes::IntegrityDevices(EntryTypes::EnrollmentCancelled(entry.clone())))?;
    create_link(entry.identity.clone(), hash.clone(), LinkTypes::IdentityToCancellations, ())?;
    Ok(hash)
}

#[hdk_extern]
pub fn remove_device(input: RemoveDeviceInput) -> ExternResult<ActionHash> {
    if input.device_key.len() != 32 {
        return Err(wasm_error!("device_key must be 32 bytes"));
    }
    let entry = DeviceRemoved {
        identity: identity_base(&input.identity),
        device_hash: device_hash_of(&input.device_key),
        at: input.at,
        signer: input.signer,
        signature: input.signature,
    };
    let hash = create_entry(&EntryZomes::IntegrityDevices(EntryTypes::DeviceRemoved(entry.clone())))?;
    create_link(entry.identity.clone(), hash.clone(), LinkTypes::IdentityToRemovals, ())?;
    create_link(removed_device_base(&entry.device_hash), hash.clone(), LinkTypes::RemovedDeviceIndex, ())?;
    Ok(hash)
}

/// Every EnrollmentKey record of an identity.
#[hdk_extern]
pub fn get_enrollment_keys(identity: AgentPubKey) -> ExternResult<Vec<Record>> {
    records_linked_from(&identity, LinkTypes::IdentityToEnrollmentKeys)
}

/// Every EnrollmentCancelled record of an identity.
#[hdk_extern]
pub fn get_enrollment_cancellations(identity: AgentPubKey) -> ExternResult<Vec<Record>> {
    records_linked_from(&identity, LinkTypes::IdentityToCancellations)
}

/// Every DeviceRemoved record of an identity.
#[hdk_extern]
pub fn get_removals(identity: AgentPubKey) -> ExternResult<Vec<Record>> {
    records_linked_from(&identity, LinkTypes::IdentityToRemovals)
}

/// The removal record for a device key, if that device was removed
/// (the verifier hashes the key it was shown).
#[hdk_extern]
pub fn get_removal_for_device(device_key: Vec<u8>) -> ExternResult<Option<Record>> {
    if device_key.len() != 32 {
        return Err(wasm_error!("device_key must be 32 bytes"));
    }
    let base = removed_device_base(&device_hash_of(&device_key));
    let links = get_links(LinkQuery::try_new(base, LinkTypes::RemovedDeviceIndex)?, GetStrategy::default())?;
    for link in links {
        if let Some(hash) = record_hash_from(link) {
            if let Some(record) = get(hash, GetOptions::default())? {
                return Ok(Some(record));
            }
        }
    }
    Ok(None)
}

/// The enrollment key in force for an identity at `at` (the readers' rule).
#[hdk_extern]
pub fn enrollment_key_in_force(input: InForceInput) -> ExternResult<Option<EnrollmentKeyInForce>> {
    let keys = get_enrollment_keys(input.identity.clone())?;
    let cancellations = get_enrollment_cancellations(input.identity)?;
    let cancelled: Vec<ActionHash> = cancellations
        .iter()
        .filter_map(|r| r.entry().to_app_option::<EnrollmentCancelled>().ok().flatten())
        .map(|c| c.enrollment_record)
        .collect();
    let mut best: Option<EnrollmentKeyInForce> = None;
    for record in keys {
        let hash = record.action_address().clone();
        if cancelled.contains(&hash) {
            continue;
        }
        let Some(key) = record.entry().to_app_option::<EnrollmentKey>().ok().flatten() else { continue };
        if key.effective_at > input.at {
            continue;
        }
        if best.as_ref().map_or(true, |b| key.effective_at > b.effective_at) {
            best = Some(EnrollmentKeyInForce { record: hash, enrollment_key: key.enrollment_key, effective_at: key.effective_at });
        }
    }
    Ok(best)
}

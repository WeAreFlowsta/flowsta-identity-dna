use hdk::prelude::*;
use agent_linking_integrity::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityAgentLinking(agent_linking_integrity::EntryTypes),
}

// ── Input/Output Types ──────────────────────────────────────────────

/// Input for the are_agents_linked function
#[derive(Serialize, Deserialize, Debug)]
pub struct AgentPair {
    pub agent_a: AgentPubKey,
    pub agent_b: AgentPubKey,
}

/// Data stored in PendingLinkRequest link tags (serialized via MessagePack)
#[derive(Serialize, Deserialize, Debug, Clone, SerializedBytes)]
pub struct PendingLinkTag {
    pub pairing_code: String,
    pub signature: Signature,
    pub initiator: AgentPubKey,
    pub expires_at: i64,
}

/// Input for complete_link_request
#[derive(Serialize, Deserialize, Debug)]
pub struct CompleteLinkInput {
    pub pairing_code: String,
    pub initiator_agent: AgentPubKey,
}

// ── Public Functions ────────────────────────────────────────────────

/// Initiate a link request with a target agent.
/// Creates a DHT link on the target agent's pubkey containing:
/// - An 8-character pairing code
/// - The initiator's signature over the sorted key pair
/// - A 10-minute expiry timestamp
///
/// Returns the pairing code for display to the user.
#[hdk_extern]
pub fn initiate_link_request(target_agent: AgentPubKey) -> ExternResult<String> {
    let my_pub_key = agent_info()?.agent_initial_pubkey;

    if my_pub_key == target_agent {
        return Err(wasm_error!("Cannot link an agent to itself"));
    }

    // Sign the sorted key pair
    let payload = sorted_agent_pair_bytes(&my_pub_key, &target_agent)?;
    let signature = sign(my_pub_key.clone(), payload)?;

    // Generate 8-character pairing code from random bytes
    let pairing_code = generate_pairing_code()?;

    // Calculate expiry (10 minutes from now)
    let now = sys_time()?;
    let now_secs = now.as_seconds_and_nanos().0;
    let expires_at = now_secs + 600; // 10 minutes

    // Create the link tag with all ceremony data
    let tag_data = PendingLinkTag {
        pairing_code: pairing_code.clone(),
        signature,
        initiator: my_pub_key.clone(),
        expires_at,
    };

    let tag_bytes: SerializedBytes = tag_data
        .try_into()
        .map_err(|e: SerializedBytesError| {
            wasm_error!(WasmErrorInner::Guest(format!(
                "Tag serialization error: {:?}",
                e
            )))
        })?;

    // Create link on TARGET agent's pubkey so they can find it
    create_link(
        target_agent,
        my_pub_key,
        LinkTypes::PendingLinkRequest,
        tag_bytes.bytes().to_vec(),
    )?;

    Ok(pairing_code)
}

/// Complete a link request by providing the pairing code.
/// Called by the TARGET agent (the one whose pubkey has the pending link).
///
/// 1. Finds the pending link request matching the pairing code and initiator
/// 2. Verifies the initiator's signature
/// 3. Signs the sorted key pair
/// 4. Constructs and commits the IsSamePersonEntry
/// 5. Creates lookup links from both agents' pubkeys to the entry
///
/// Returns the ActionHash of the committed entry.
#[hdk_extern]
pub fn complete_link_request(input: CompleteLinkInput) -> ExternResult<ActionHash> {
    let my_pub_key = agent_info()?.agent_initial_pubkey;
    let now = sys_time()?;
    let now_secs = now.as_seconds_and_nanos().0;

    // Find pending link requests on my pubkey
    let links = get_links(
        LinkQuery::try_new(my_pub_key.clone(), LinkTypes::PendingLinkRequest)?,
        GetStrategy::default(),
    )?;

    // Find the matching request
    let mut found_tag: Option<PendingLinkTag> = None;
    let mut found_link_action: Option<ActionHash> = None;

    for link in &links {
        let tag_bytes = SerializedBytes::from(UnsafeBytes::from(link.tag.clone().into_inner()));
        let tag_result: Result<PendingLinkTag, _> = tag_bytes.try_into();
        if let Ok(tag_data) = tag_result {
            if tag_data.pairing_code == input.pairing_code
                && tag_data.initiator == input.initiator_agent
            {
                // Check expiry
                if tag_data.expires_at < now_secs {
                    // Clean up expired request
                    let action_hash = ActionHash::from(link.create_link_hash.clone());
                    let _ = delete_link(action_hash, GetOptions::default());
                    return Err(wasm_error!("Pairing code has expired"));
                }
                found_tag = Some(tag_data);
                found_link_action =
                    ActionHash::try_from(link.create_link_hash.clone()).ok();
                break;
            }
        }
    }

    let tag_data =
        found_tag.ok_or_else(|| wasm_error!("No matching pending link request found"))?;

    // Verify the initiator's signature over the sorted key pair
    let payload = sorted_agent_pair_bytes(&my_pub_key, &input.initiator_agent)?;

    if !verify_signature(
        input.initiator_agent.clone(),
        tag_data.signature.clone(),
        payload.clone(),
    )? {
        return Err(wasm_error!("Initiator's signature is invalid"));
    }

    // Sign the sorted key pair ourselves
    let my_signature = sign(my_pub_key.clone(), payload)?;

    // Construct the IsSamePersonEntry with canonical ordering (agent_a < agent_b)
    let mut keys = vec![
        (my_pub_key.clone(), my_signature),
        (input.initiator_agent.clone(), tag_data.signature),
    ];
    keys.sort_by(|a, b| a.0.cmp(&b.0));

    let entry = IsSamePersonEntry {
        agent_a: keys[0].0.clone(),
        signature_a: keys[0].1.clone(),
        agent_b: keys[1].0.clone(),
        signature_b: keys[1].1.clone(),
        created_at: now_secs,
    };

    // Commit the entry
    let entry_hash = create_entry(&EntryZomes::IntegrityAgentLinking(
        EntryTypes::IsSamePerson(entry.clone()),
    ))?;

    // Create lookup links from BOTH agents' pubkeys to the entry
    create_link(
        entry.agent_a.clone(),
        entry_hash.clone(),
        LinkTypes::AgentToIsSamePerson,
        (),
    )?;
    create_link(
        entry.agent_b.clone(),
        entry_hash.clone(),
        LinkTypes::AgentToIsSamePerson,
        (),
    )?;

    // Clean up the pending link request
    if let Some(link_action) = found_link_action {
        let _ = delete_link(link_action, GetOptions::default());
    }

    Ok(entry_hash)
}

/// Get all agents linked to a given agent (non-deleted entries only).
/// Follows links from the agent's pubkey to IsSamePersonEntry entries,
/// filters out deleted entries, and returns the OTHER agent from each pair.
#[hdk_extern]
pub fn get_linked_agents(agent: AgentPubKey) -> ExternResult<Vec<AgentPubKey>> {
    let links = get_links(
        LinkQuery::try_new(agent.clone(), LinkTypes::AgentToIsSamePerson)?,
        GetStrategy::default(),
    )?;

    let mut linked_agents: Vec<AgentPubKey> = Vec::new();

    for link in links {
        let action_hash = match ActionHash::try_from(link.target.clone()) {
            Ok(hash) => hash,
            Err(_) => continue,
        };

        // Get the entry details to check for Deletes (coordination layer)
        let details = match get_details(action_hash, GetOptions::default())? {
            Some(details) => details,
            None => continue,
        };

        match details {
            Details::Record(record_details) => {
                // Check if the entry has been deleted (revoked)
                if !record_details.deletes.is_empty() {
                    continue;
                }

                // Extract the entry and find the OTHER agent
                if let Some(entry) = record_details.record.entry().as_option() {
                    if let Ok(is_same_person) = IsSamePersonEntry::try_from(entry) {
                        let other_agent = if is_same_person.agent_a == agent {
                            is_same_person.agent_b.clone()
                        } else {
                            is_same_person.agent_a.clone()
                        };

                        if !linked_agents.contains(&other_agent) {
                            linked_agents.push(other_agent);
                        }
                    }
                }
            }
            _ => continue,
        }
    }

    Ok(linked_agents)
}

/// Check if two specific agents are linked (non-deleted entry exists).
#[hdk_extern]
pub fn are_agents_linked(agents: AgentPair) -> ExternResult<bool> {
    let linked = get_linked_agents(agents.agent_a.clone())?;
    Ok(linked.contains(&agents.agent_b))
}

/// Revoke a link by deleting the IsSamePersonEntry creation action.
/// Only one of the two agents in the entry can revoke it.
/// Returns the ActionHash of the Delete action.
#[hdk_extern]
pub fn revoke_link(entry_action_hash: ActionHash) -> ExternResult<ActionHash> {
    let my_pub_key = agent_info()?.agent_initial_pubkey;

    // Get the entry to verify the caller is one of the agents
    let record = get(entry_action_hash.clone(), GetOptions::default())?
        .ok_or(wasm_error!("Entry not found"))?;

    let entry = record
        .entry()
        .as_option()
        .ok_or(wasm_error!("No entry data found"))?;

    let is_same_person = IsSamePersonEntry::try_from(entry)
        .map_err(|_| wasm_error!("Entry is not an IsSamePersonEntry"))?;

    if my_pub_key != is_same_person.agent_a && my_pub_key != is_same_person.agent_b {
        return Err(wasm_error!(
            "Only one of the two linked agents can revoke this link"
        ));
    }

    // Delete the original creation action
    let delete_hash = delete_entry(entry_action_hash)?;

    Ok(delete_hash)
}

/// Input for create_direct_link (API-mediated linking for V1 desktop apps without a conductor)
#[derive(Serialize, Deserialize, Debug)]
pub struct DirectLinkInput {
    /// The other agent's public key (e.g., the desktop agent)
    pub other_agent: AgentPubKey,
    /// The other agent's Ed25519 signature over the sorted key pair bytes
    pub other_signature: Signature,
}

/// Create an IsSamePersonEntry with an externally-provided signature for one agent.
///
/// Designed for V1 desktop apps that don't have their own Holochain conductor.
/// The desktop signs the key pair locally, sends the signature to the API,
/// and the web agent (caller) creates the entry with both signatures.
///
/// Security: The other agent's signature is verified before committing.
/// The entry passes the same validation as entries from the pairing ceremony.
#[hdk_extern]
pub fn create_direct_link(input: DirectLinkInput) -> ExternResult<ActionHash> {
    let my_pub_key = agent_info()?.agent_initial_pubkey;

    if my_pub_key == input.other_agent {
        return Err(wasm_error!("Cannot link an agent to itself"));
    }

    // Verify the other agent's signature over the sorted key pair
    let payload = sorted_agent_pair_bytes(&my_pub_key, &input.other_agent)?;

    if !verify_signature(
        input.other_agent.clone(),
        input.other_signature.clone(),
        payload.clone(),
    )? {
        return Err(wasm_error!(
            "Other agent's signature is invalid"
        ));
    }

    // Sign our half
    let my_signature = sign(my_pub_key.clone(), payload)?;

    // Construct the IsSamePersonEntry with canonical ordering (agent_a < agent_b)
    let mut keys = vec![
        (my_pub_key.clone(), my_signature),
        (input.other_agent.clone(), input.other_signature),
    ];
    keys.sort_by(|a, b| a.0.cmp(&b.0));

    let now = sys_time()?;
    let now_secs = now.as_seconds_and_nanos().0;

    let entry = IsSamePersonEntry {
        agent_a: keys[0].0.clone(),
        signature_a: keys[0].1.clone(),
        agent_b: keys[1].0.clone(),
        signature_b: keys[1].1.clone(),
        created_at: now_secs,
    };

    // Commit the entry
    let entry_hash = create_entry(&EntryZomes::IntegrityAgentLinking(
        EntryTypes::IsSamePerson(entry.clone()),
    ))?;

    // Create lookup links from BOTH agents' pubkeys to the entry
    create_link(
        entry.agent_a.clone(),
        entry_hash.clone(),
        LinkTypes::AgentToIsSamePerson,
        (),
    )?;
    create_link(
        entry.agent_b.clone(),
        entry_hash.clone(),
        LinkTypes::AgentToIsSamePerson,
        (),
    )?;

    Ok(entry_hash)
}

// ── Helper Functions ────────────────────────────────────────────────

/// Generate an 8-character alphanumeric pairing code (uppercase + digits).
/// Format: XXXX-XXXX (e.g., "XKFW-9M2R")
fn generate_pairing_code() -> ExternResult<String> {
    // Charset of 32 unambiguous characters (removed 0/O, 1/I/L)
    let charset = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

    let random = random_bytes(8)?;
    let mut code = String::with_capacity(9); // 8 chars + 1 dash
    for (i, byte) in random.iter().enumerate().take(8) {
        if i == 4 {
            code.push('-');
        }
        code.push(charset[(*byte as usize) % charset.len()] as char);
    }

    Ok(code)
}

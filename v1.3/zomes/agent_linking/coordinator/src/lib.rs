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

/// Input for create_direct_link (API-mediated linking for desktop apps)
#[derive(Serialize, Deserialize, Debug)]
pub struct DirectLinkInput {
    /// The other agent's public key (e.g., the desktop agent)
    pub other_agent: AgentPubKey,
    /// The other agent's Ed25519 signature over the sorted key pair bytes
    pub other_signature: Signature,
}

// ── Public Functions ────────────────────────────────────────────────

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

/// Create an IsSamePersonEntry with an externally-provided signature for one agent.
///
/// Designed for desktop apps where the API mediates the linking.
/// The desktop signs the key pair locally, sends the signature to the API,
/// and the web agent (caller) creates the entry with both signatures.
///
/// Security: The other agent's signature is verified before committing.
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


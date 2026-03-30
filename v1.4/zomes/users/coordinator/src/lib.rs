use hdk::prelude::*;
use users_integrity::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityUsers(users_integrity::EntryTypes),
}

/// Register a new user profile
/// This creates the profile entry and links it to the agent
/// v1.2: Requires DID (no email_hash or display_name for security)
#[hdk_extern]
pub fn register_user(profile: UserProfile) -> ExternResult<Record> {
    // Validate required fields
    if profile.did.is_empty() {
        return Err(wasm_error!("DID is required for identity v1.2"));
    }
    
    // Create the profile entry wrapped in the dependent types
    let profile_hash = create_entry(&EntryZomes::IntegrityUsers(EntryTypes::UserProfile(profile.clone())))?;
    
    // Create a link from the agent's public key to their profile
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    create_link(
        my_agent_pub_key.clone(),
        profile_hash.clone(),
        LinkTypes::AgentToProfile,
        (),
    )?;
    
    // Return the created record
    let record = get(profile_hash, GetOptions::default())?
        .ok_or(wasm_error!("Could not find the newly created profile"))?;
    
    Ok(record)
}

/// Get the current agent's profile (follows update chain recursively)
/// ⚠️ CRITICAL: Uses LOOP to follow ENTIRE update chain (not just .last())
#[hdk_extern]
pub fn get_my_profile(_: ()) -> ExternResult<Option<Record>> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    // Get links from agent to profile
    let links = get_links(
        LinkQuery::try_new(my_agent_pub_key, LinkTypes::AgentToProfile)?,
        GetStrategy::default()
    )?;
    
    // Get the first (should only be one) profile
    if let Some(link) = links.first() {
        let mut current_hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid profile hash"))?;
        
        // ⚠️ CRITICAL: Use LOOP to recursively follow ENTIRE update chain
        loop {
            let details = get_details(current_hash.clone(), GetOptions::default())?
                .ok_or(wasm_error!("Profile not found in chain"))?;
            
            match details {
                Details::Record(record_details) => {
                    // Check if there's an update
                    if let Some(latest_update) = record_details.updates.last() {
                        // Continue following the chain
                        current_hash = latest_update.action_address().clone();
                    } else {
                        // No more updates - this is the latest record
                        return Ok(Some(record_details.record));
                    }
                }
                _ => return Err(wasm_error!("Expected Record details")),
            }
        }
    }
    
    Ok(None)
}

/// Update the current agent's profile
#[hdk_extern]
pub fn update_profile(profile: UserProfile) -> ExternResult<Record> {
    // Get the current profile
    let current_profile_record = get_my_profile(())?
        .ok_or(wasm_error!("No profile found to update"))?;
    
    // Update the entry
    let updated_profile_hash = update_entry(
        current_profile_record.action_address().clone(),
        &EntryZomes::IntegrityUsers(EntryTypes::UserProfile(profile)),
    )?;
    
    // Return the updated record
    let record = get(updated_profile_hash, GetOptions::default())?
        .ok_or(wasm_error!("Could not find the updated profile"))?;
    
    Ok(record)
}

/// Get any user's profile by their agent public key (follows update chain recursively)
/// ⚠️ CRITICAL: Uses LOOP to follow ENTIRE update chain (not just .last())
#[hdk_extern]
pub fn get_profile(agent: AgentPubKey) -> ExternResult<Option<Record>> {
    // Get links from the specified agent to their profile
    let links = get_links(
        LinkQuery::try_new(agent, LinkTypes::AgentToProfile)?,
        GetStrategy::default()
    )?;
    
    // Get the first (should only be one) profile
    if let Some(link) = links.first() {
        let mut current_hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid profile hash"))?;
        
        // ⚠️ CRITICAL: Use LOOP to recursively follow ENTIRE update chain
        loop {
            let details = get_details(current_hash.clone(), GetOptions::default())?
                .ok_or(wasm_error!("Profile not found in chain"))?;
            
            match details {
                Details::Record(record_details) => {
                    // Check if there's an update
                    if let Some(latest_update) = record_details.updates.last() {
                        // Continue following the chain
                        current_hash = latest_update.action_address().clone();
                    } else {
                        // No more updates - this is the latest record
                        return Ok(Some(record_details.record));
                    }
                }
                _ => return Err(wasm_error!("Expected Record details")),
            }
        }
    }
    
    Ok(None)
}

/// Export all data from this DNA (for migration TO next version)
#[hdk_extern]
pub fn export_all_data(_: ()) -> ExternResult<UserProfile> {
    let profile_record = get_my_profile(())?
        .ok_or(wasm_error!("No profile found to export"))?;
    
    let profile: UserProfile = profile_record
        .entry()
        .to_app_option()
        .map_err(|_| wasm_error!("Could not deserialize profile"))?
        .ok_or(wasm_error!("Profile entry is None"))?;
    
    Ok(profile)
}

/// Import data from previous DNA version
#[hdk_extern]
pub fn import_data(profile: UserProfile) -> ExternResult<Record> {
    // Create new profile entry
    let hash = create_entry(&EntryZomes::IntegrityUsers(EntryTypes::UserProfile(profile.clone())))?;
    
    // Create link from agent to profile
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    create_link(
        my_agent_pub_key.clone(),
        hash.clone(),
        LinkTypes::AgentToProfile,
        (),
    )?;
    
    let record = get(hash, GetOptions::default())?
        .ok_or(wasm_error!("Could not find the newly created entry"))?;
    
    Ok(record)
}


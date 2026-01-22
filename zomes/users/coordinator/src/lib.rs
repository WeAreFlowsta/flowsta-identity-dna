use hdk::prelude::*;
use users_integrity::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityUsers(users_integrity::EntryTypes),
}

/// Register a new user profile
/// This creates the profile entry and links it to the agent
#[hdk_extern]
pub fn register_user(profile: UserProfile) -> ExternResult<Record> {
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

/// Get the current agent's profile
#[hdk_extern]
pub fn get_my_profile(_: ()) -> ExternResult<Option<Record>> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    // Get links from agent to profile
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToProfile)?
            .build(),
    )?;
    
    // Get the first (should only be one) profile
    if let Some(link) = links.first() {
        let profile_hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid profile hash"))?;
        return get(profile_hash, GetOptions::default());
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

/// Get any user's profile by their agent public key
#[hdk_extern]
pub fn get_profile(agent: AgentPubKey) -> ExternResult<Option<Record>> {
    // Get links from the specified agent to their profile
    let links = get_links(
        GetLinksInputBuilder::try_new(agent, LinkTypes::AgentToProfile)?
            .build(),
    )?;
    
    // Get the first (should only be one) profile
    if let Some(link) = links.first() {
        let profile_hash = ActionHash::try_from(link.target.clone())
            .map_err(|_| wasm_error!("Invalid profile hash"))?;
        return get(profile_hash, GetOptions::default());
    }
    
    Ok(None)
}


use hdk::prelude::*;
use sites_integrity::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegritySites(sites_integrity::EntryTypes),
}

/// Join a site - creates an immutable membership record
#[hdk_extern]
pub fn join_site(site_id: String) -> ExternResult<Record> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    let now = sys_time()?;
    
    let membership = SiteMembership {
        site_id: site_id.clone(),
        joined_at: now.as_seconds_and_nanos().0,  // Convert Timestamp to i64
        agent_key: my_agent_pub_key.to_string(),  // Convert to String
    };
    
    // Create the membership entry
    let membership_hash = create_entry(&EntryZomes::IntegritySites(EntryTypes::SiteMembership(membership.clone())))?;
    
    // Create link from agent to membership
    create_link(
        my_agent_pub_key.clone(),
        membership_hash.clone(),
        LinkTypes::AgentToSiteMemberships,
        (),
    )?;
    
    // Create link from site to member (for site member queries)
    // Skip hash_entry for strings - just use site_id directly as tag
    create_link(
        my_agent_pub_key.clone(),
        membership_hash.clone(),
        LinkTypes::SiteToMembers,
        site_id.as_bytes().to_vec(),  // Convert &[u8] to Vec<u8>
    )?;
    
    // Return the created record
    let record = get(membership_hash, GetOptions::default())?
        .ok_or(wasm_error!("Could not find the newly created membership"))?;
    
    Ok(record)
}

/// Get all sites the current agent has joined
#[hdk_extern]
pub fn get_my_sites(_: ()) -> ExternResult<Vec<Record>> {
    let my_agent_pub_key = agent_info()?.agent_initial_pubkey;
    
    // Get links from agent to memberships
    let links = get_links(
        GetLinksInputBuilder::try_new(my_agent_pub_key, LinkTypes::AgentToSiteMemberships)?
            .build(),
    )?;
    
    // Get all membership records
    let mut memberships = Vec::new();
    for link in links {
        if let Some(action_hash) = link.target.into_action_hash() {
            if let Some(record) = get(action_hash, GetOptions::default())? {
                memberships.push(record);
            }
        }
    }
    
    Ok(memberships)
}

/// Get all members of a specific site
#[hdk_extern]
pub fn get_site_members(_site_id: String) -> ExternResult<Vec<AgentPubKey>> {
    // Simplified for MVP - just return empty for now
    // Full implementation would query links by tag
    Ok(vec![])
}

/// Check if current agent is a member of a site
#[hdk_extern]
pub fn is_site_member(site_id: String) -> ExternResult<bool> {
    let my_sites = get_my_sites(())?;
    
    // Check if any membership matches the site_id
    for record in my_sites {
        if let Some(membership) = record.entry().as_option() {
            if let Ok(site_membership) = SiteMembership::try_from(membership) {
                if site_membership.site_id == site_id {
                    return Ok(true);
                }
            }
        }
    }
    
    Ok(false)
}


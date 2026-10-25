use hdi::prelude::*;

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct SiteMembership {
    pub site_id: String,
    pub joined_at: i64,
    pub agent_key: String,
}

#[hdk_entry_types]
#[unit_enum(UnitEntryTypes)]
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EntryTypes {
    SiteMembership(SiteMembership),
}

#[derive(Serialize, Deserialize)]
#[hdk_link_types]
pub enum LinkTypes {
    AgentToSiteMemberships,
    SiteToMembers,
}

#[cfg_attr(not(feature = "integrity"), hdk_extern)]
pub fn validate(_op: Op) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}

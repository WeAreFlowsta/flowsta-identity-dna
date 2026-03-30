use hdi::prelude::*;

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct UserProfile {
    // 🔴 REMOVED in v1.2: email_hash (brute-forceable PII vulnerability)
    // 🔴 REMOVED in v1.2: display_name (contains email username - PII)
    // 🔴 REMOVED in v1.4: profile_picture (identifiable - moved to private DNA)
    // 🔴 REMOVED in v1.4: has_custom_picture (moved to private DNA)

    // ✅ SECURE: Only non-identifiable data on public DHT
    pub did: String,                            // W3C DID (designed to be public)
    pub created_at: i64,
    pub updated_at: i64,
}

#[hdk_entry_types]
#[unit_enum(UnitEntryTypes)]
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EntryTypes {
    UserProfile(UserProfile),
}

#[derive(Serialize, Deserialize)]
#[hdk_link_types]
pub enum LinkTypes {
    AgentToProfile,
}

#[cfg_attr(not(feature = "integrity"), hdk_extern)]
pub fn validate(_op: Op) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}

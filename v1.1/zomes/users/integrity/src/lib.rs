use hdi::prelude::*;

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct UserProfile {
    pub email_hash: String,
    pub display_name: String,
    pub profile_picture: String,        // Base64 data URI (identicon or custom upload)
    pub has_custom_picture: bool,       // True if user uploaded custom picture
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

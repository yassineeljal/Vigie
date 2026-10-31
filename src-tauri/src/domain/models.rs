use serde::{Deserialize, Serialize};
use specta::Type;

macro_rules! str_enum {
    ($name:ident { $($variant:ident => $s:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $s),+ }
            }
            pub fn parse(s: &str) -> Option<Self> {
                match s { $($s => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

str_enum!(Criticality { Critical => "critical", High => "high", Medium => "medium", Low => "low" });
str_enum!(AccountStatus { Active => "active", Unused => "unused", Deleted => "deleted" });
str_enum!(MfaKind {
    Passkey => "passkey", HardwareKey => "hardware_key", Totp => "totp",
    Push => "push", Sms => "sms", Email => "email",
});
str_enum!(IdentityKind { Email => "email", Phone => "phone" });
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    Label,
    Domain,
    Criticality,
    UpdatedAt,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct MfaMethod {
    pub kind: MfaKind,
    pub backup_codes: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct Identity {
    pub id: u32,
    pub kind: IdentityKind,
    pub value: String,
    /// Compte (ex. Gmail) auquel cette identité correspond, s'il est dans l'inventaire.
    pub account_id: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct IdentityInput {
    pub kind: IdentityKind,
    pub value: String,
    pub account_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct Account {
    pub id: u32,
    pub domain: String,
    pub label: String,
    pub identity_id: Option<u32>,
    pub username: Option<String>,
    pub criticality: Criticality,
    pub recovery_identity_id: Option<u32>,
    pub pwd_changed_at: Option<String>,
    pub status: AccountStatus,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub mfa: Vec<MfaMethod>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct AccountInput {
    /// URL ou domaine saisi ; normalisé en domaine enregistrable.
    pub domain: String,
    pub label: String,
    pub identity_id: Option<u32>,
    pub username: Option<String>,
    pub criticality: Criticality,
    pub recovery_identity_id: Option<u32>,
    pub pwd_changed_at: Option<String>,
    pub status: AccountStatus,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub mfa: Vec<MfaMethod>,
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct AccountFilter {
    pub search: Option<String>,
    pub status: Option<AccountStatus>,
    pub criticality: Option<Criticality>,
    pub no_mfa: bool,
    pub tag: Option<String>,
    pub sort: SortKey,
    pub descending: bool,
}

impl From<&Account> for AccountInput {
    fn from(a: &Account) -> Self {
        Self {
            domain: a.domain.clone(),
            label: a.label.clone(),
            identity_id: a.identity_id,
            username: a.username.clone(),
            criticality: a.criticality,
            recovery_identity_id: a.recovery_identity_id,
            pwd_changed_at: a.pwd_changed_at.clone(),
            status: a.status,
            notes: a.notes.clone(),
            tags: a.tags.clone(),
            mfa: a.mfa.clone(),
        }
    }
}

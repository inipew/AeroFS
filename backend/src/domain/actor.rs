/// Identity and roles needed by application-level authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub id: String,
    pub username: String,
    pub is_admin: bool,
}

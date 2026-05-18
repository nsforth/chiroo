use uuid::Uuid;

pub struct UserId(pub Uuid);

pub struct User {
    pub id: UserId,
    pub name: String,
    pub username: String,
    pub avatar: Vec<u8>,
    pub is_admin: bool,
    pub sso_id: Option<String>,
    pub sso_provider: Option<String>,
}

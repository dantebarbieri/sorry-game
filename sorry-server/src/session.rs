use std::fmt;

/// Opaque session token identifying a seated player inside a room.
/// Clients keep this around across reconnects; the server maps it back to
/// `(room_code, player_index)` via `Lobby::sessions`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionToken(String);

impl SessionToken {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionToken {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for SessionToken {
    fn from(s: String) -> Self {
        Self(s)
    }
}

use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerEntityId(pub uuid::Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuntimeEntityId(pub uuid::Uuid);

impl PlayerEntityId {
    pub fn parse(raw: &str) -> Option<Self> {
        uuid::Uuid::parse_str(raw).ok().map(Self)
    }

    pub fn canonical_wire_id(self) -> String {
        self.0.to_string()
    }
}

impl RuntimeEntityId {
    pub fn parse(raw: &str) -> Option<Self> {
        uuid::Uuid::parse_str(raw).ok().map(Self)
    }

    pub fn as_uuid(self) -> uuid::Uuid {
        self.0
    }
}

impl Display for PlayerEntityId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Display for RuntimeEntityId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

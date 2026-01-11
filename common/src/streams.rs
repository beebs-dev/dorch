pub enum LeaveReason {
    Left,
    Kicked,
}

impl TryFrom<&u8> for LeaveReason {
    type Error = anyhow::Error;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(LeaveReason::Left),
            1 => Ok(LeaveReason::Kicked),
            _ => Err(anyhow::anyhow!("Invalid LeaveReason value: {}", value)),
        }
    }
}

impl From<LeaveReason> for u8 {
    fn from(message_type: LeaveReason) -> Self {
        match message_type {
            LeaveReason::Left => 0,
            LeaveReason::Kicked => 1,
        }
    }
}

pub enum WebsockMessageType {
    Message,
    Typing,
    StopTyping,
    MemberJoined,
    MemberLeft,
    PartyInfo,
    Invite,
}

impl TryFrom<&u8> for WebsockMessageType {
    type Error = anyhow::Error;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(WebsockMessageType::Message),
            1 => Ok(WebsockMessageType::Typing),
            2 => Ok(WebsockMessageType::StopTyping),
            3 => Ok(WebsockMessageType::MemberJoined),
            4 => Ok(WebsockMessageType::MemberLeft),
            5 => Ok(WebsockMessageType::PartyInfo),
            6 => Ok(WebsockMessageType::Invite),
            _ => Err(anyhow::anyhow!(
                "Invalid WebsockMessageType value: {}",
                value
            )),
        }
    }
}

impl From<WebsockMessageType> for u8 {
    fn from(message_type: WebsockMessageType) -> Self {
        match message_type {
            WebsockMessageType::Message => 0,
            WebsockMessageType::Typing => 1,
            WebsockMessageType::StopTyping => 2,
            WebsockMessageType::MemberJoined => 3,
            WebsockMessageType::MemberLeft => 4,
            WebsockMessageType::PartyInfo => 5,
            WebsockMessageType::Invite => 6,
        }
    }
}

pub mod subjects {
    use std::fmt::Display;

    pub fn user<T>(user_id: T) -> String
    where
        T: Display,
    {
        format!("dorch.user.{}", user_id)
    }

    pub fn party<T>(thread_id: T) -> String
    where
        T: Display,
    {
        format!("dorch.party.{}", thread_id)
    }
}

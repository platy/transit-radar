use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum SyncData<D, I> {
    Initial {
        session_id: i64,
        update_number: u64,
        data: D,
    },
    Increment {
        session_id: i64,
        update_number: u64,
        increment: I,
    },
}

impl<D, I> SyncData<D, I> {
    pub fn session_id(&self) -> i64 {
        match self {
            Self::Initial {
                session_id,
                update_number: _,
                data: _,
            }
            | Self::Increment {
                increment: _,
                update_number: _,
                session_id,
            } => *session_id,
        }
    }
}

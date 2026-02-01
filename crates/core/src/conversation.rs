//! Conversation-related types.

use little_agent_model::ModelMessage;

/// Represents a conversation.
#[derive(Clone, Default, Debug)]
pub struct Conversation {
    pub(crate) items: Vec<Item>,
}

/// An item in the conversation.
#[derive(Clone, Debug)]
pub struct Item {
    pub(crate) msg: ModelMessage,
    pub(crate) transcript: String,
}

impl Item {
    /// Returns the transcript of this item.
    ///
    /// The transcript is a string representation of the message item,
    /// which can be exported later. But transcript alone is not enough
    /// to reconstruct the message item.
    #[inline]
    pub fn transcript(&self) -> &str {
        &self.transcript
    }
}

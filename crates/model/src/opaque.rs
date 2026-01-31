use std::any::Any;
use std::fmt::{self, Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// An opaque message from the model that doesn't need to be processed
/// by the agent.
///
/// This is useful to build the conversation history, since types this
/// crate defines may lose context for the model. `OpaqueMessage` allows
/// model implementors to add arbitrary items to the history messages.
/// For example, some models rely on complete tool call message to work
/// correctly, the model implementor can use this type to store that
/// structure and later serialize to the request payload.
pub struct OpaqueMessage(Arc<dyn OpaqueMessageObject>);

impl OpaqueMessage {
    /// Creates a new `OpaqueMessage`.
    ///
    /// The `id` will be used to identify the message, and is should be
    /// unique across the conversation. Comparing `OpaqueMessage` is just
    /// trivially comparing the `id`.
    #[inline]
    pub fn new<ID: Into<String>, T: Send + Sync + 'static>(
        id: ID,
        value: T,
    ) -> Self {
        let id = id.into();
        Self(Arc::new(OpaqueMessageInner { id, value }))
    }

    /// Converts the `OpaqueMessage` into its raw type.
    #[inline]
    pub fn to_raw<T: 'static>(&self) -> Option<&T> {
        self.0.as_any().downcast_ref()
    }
}

impl Clone for OpaqueMessage {
    #[inline]
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl Debug for OpaqueMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpaqueMessage")
            .field("id", &self.0.id())
            .finish()
    }
}

impl PartialEq for OpaqueMessage {
    fn eq(&self, other: &Self) -> bool {
        self.0.id() == other.0.id()
    }
}

impl Eq for OpaqueMessage {}

impl Hash for OpaqueMessage {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.id().hash(state);
    }
}

trait OpaqueMessageObject: Send + Sync {
    fn id(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}

struct OpaqueMessageInner<T> {
    id: String,
    value: T,
}

impl<T: Send + Sync + 'static> OpaqueMessageObject for OpaqueMessageInner<T> {
    fn id(&self) -> &str {
        &self.id
    }

    fn as_any(&self) -> &dyn Any {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[derive(Clone)]
    struct RawMessage(String);

    #[test]
    fn test_convert_between() {
        let raw = RawMessage("Hello".to_string());
        let opaque = OpaqueMessage::new("msg:0", raw);
        let raw_back = opaque.to_raw::<RawMessage>().unwrap();
        assert_eq!(raw_back.0, "Hello");
    }

    #[test]
    fn test_common_traits() {
        let raw_0 = RawMessage("Hello".to_string());
        let opaque_0 = OpaqueMessage::new("msg:0", raw_0);
        let raw_1 = RawMessage("Bye".to_string());
        let opaque_1 = OpaqueMessage::new("msg:1", raw_1);

        let opaque_0_clone = opaque_0.clone();
        assert_eq!(opaque_0, opaque_0_clone);
        assert_ne!(opaque_0, opaque_1);

        let mut set = HashSet::new();
        set.insert(opaque_0);
        set.insert(opaque_0_clone);
        set.insert(opaque_1);
        assert_eq!(set.len(), 2);
    }
}

use std::fmt::{self, Debug, Display};

#[derive(Debug)]
pub struct ApprovalResult {
    pub approved: bool,
    pub why: Option<String>,
}

/// Approval for a tool call request.
pub struct Approval {
    what: String,
    justification: String,
    pub(crate) on_result: Option<Box<dyn FnOnce(ApprovalResult) + Send>>,
}

impl Approval {
    /// Creates a new approval.
    #[inline]
    pub fn new<S1: Into<String>, S2: Into<String>>(
        what: S1,
        justification: S2,
    ) -> Self {
        Self {
            what: what.into(),
            justification: justification.into(),
            on_result: None,
        }
    }

    /// Returns what the approval is for.
    #[inline]
    pub fn what(&self) -> &str {
        &self.what
    }

    /// Returns the justification for the approval.
    #[inline]
    pub fn justification(&self) -> &str {
        &self.justification
    }

    /// Approves the request.
    #[inline]
    pub fn approve(self) {
        let Some(on_result) = self.on_result else {
            return;
        };
        (on_result)(ApprovalResult {
            approved: true,
            why: None,
        });
    }

    /// Rejects the request with an optional reason.
    #[inline]
    pub fn reject(self, reason: Option<String>) {
        let Some(on_result) = self.on_result else {
            return;
        };
        (on_result)(ApprovalResult {
            approved: false,
            why: reason,
        });
    }
}

impl Debug for Approval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Approval")
            .field("what", &self.what)
            .field("justification", &self.justification)
            .finish_non_exhaustive()
    }
}

impl Display for Approval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{} ({})", self.what, self.justification))
    }
}

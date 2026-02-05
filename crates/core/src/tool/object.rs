use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::oneshot;
use tracing::Instrument;

use super::{Approval, Error, Tool, ToolResult};

pub(crate) trait ToolObject: Send + Sync + 'static {
    fn name(&self) -> &str;

    fn description(&self) -> &str;

    fn parameter_schema(&self) -> &Value;

    fn execute(
        self: Arc<Self>,
        arguments: Value,
        on_request: &Option<Box<dyn Fn(Approval) + Send + Sync>>,
    ) -> Pin<Box<dyn Future<Output = ToolResult> + Send>>;
}

pub(crate) struct ToolObjectImpl<T: Tool>(pub T);

impl<T: Tool> ToolObject for ToolObjectImpl<T> {
    #[inline]
    fn name(&self) -> &str {
        self.0.name()
    }

    #[inline]
    fn description(&self) -> &str {
        self.0.description()
    }

    #[inline]
    fn parameter_schema(&self) -> &Value {
        self.0.parameter_schema()
    }

    #[inline]
    fn execute(
        self: Arc<Self>,
        arguments: Value,
        on_request: &Option<Box<dyn Fn(Approval) + Send + Sync>>,
    ) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> {
        let input: T::Input = match serde_json::from_value(arguments) {
            Ok(input) => input,
            Err(err) => {
                let reason = format!("{err}");
                return Box::pin(std::future::ready(ToolResult::Err(
                    Error::invalid_input().with_reason(reason),
                )));
            }
        };

        let (approval_res_tx, approval_res_rx) = oneshot::channel();
        let mut approval = self.0.make_approval(&input);
        approval.on_result = Some(Box::new(move |result| {
            approval_res_tx.send(result).ok();
        }));

        if let Some(on_request) = on_request {
            on_request(approval);
        } else {
            // No request handler provided, assuming yolo mode.
            approval.approve();
        }

        Box::pin(
            async move {
                let Ok(approval_res) = approval_res_rx.await else {
                    return ToolResult::Err(Error::user_rejected());
                };
                trace!("tool call approval result: {approval_res:?}");
                if !approval_res.approved {
                    let mut err = Error::user_rejected();
                    if let Some(reason) = approval_res.why {
                        err = err.with_reason(reason);
                    }
                    return ToolResult::Err(err);
                }
                self.0.execute(input).await
            }
            .instrument(debug_span!("tool execute")),
        )
    }
}

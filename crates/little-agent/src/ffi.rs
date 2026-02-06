//! FFI bindings for the library.

use std::ffi::{CStr, c_char, c_void};
use std::ops::Deref;
use std::sync::{Arc, LazyLock};

use little_agent_core::TranscriptSource;
use little_agent_core::tool::Approval as ToolApproval;
use little_agent_openai_model::{OpenAIConfigBuilder, OpenAIProvider};
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};

use crate::{Session, SessionBuilder};

static TOKIO_RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    RuntimeBuilder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()
        .unwrap()
});

/// Error codes returned by the C APIs.
#[repr(u32)]
pub enum ErrorCode {
    /// No error occurred.
    Ok = 0,
    /// Invalid parameters or strings.
    Invalid = 1,
}

/// A wrapper around `SessionBuilder`. It's needed mainly because most methods
/// of `SessionBuilder` consume `self`. And when exposing it to C, it's heap-
/// allocated by a box, so we cannot take `self` out of it.
/// `SessionBuilderWrapper` adds another layer lets us move `SessionBuilder` out
/// and make some changes, and then put it back.
struct SessionBuilderWrapper {
    builder: Option<SessionBuilder>,
}

/// Callbacks for various events from the session.
///
/// Note that callback functions and `user_info` are assumed to be thread-safe
/// and able to send across the thread boundaries.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SessionCallbacks {
    /// User-defined data to be passed to the callbacks.
    pub user_info: *mut c_void,
    /// Callback to handle the session becoming idle.
    pub on_idle: Option<unsafe extern "C" fn(*mut c_void)>,
    /// Callback to handle the generated transcripts.
    ///
    /// Parameters:
    /// - `user_info`: The user-defined data.
    /// - `transcript`: Transcript string.
    /// - `transcript_len`: Length of the transcript string.
    /// - `source`: Transcript source (0 for user, 1 for assistant).
    pub on_transcript:
        Option<unsafe extern "C" fn(*mut c_void, *const c_char, usize, u32)>,
    /// Callback to handle the tool call request.
    ///
    /// Parameters:
    /// - `user_info`: The user-defined data.
    /// - `approval`: A tool call approval object. You must either call
    ///   `la_tool_approval_approve` or `la_tool_approval_reject` to consume
    ///   the approval object, or it will be leaked.
    pub on_tool_call_request:
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    /// Callback to free the user-defined data.
    pub free: Option<unsafe extern "C" fn(*mut c_void)>,
}

// SAFETY: `SessionCallbacks` is guaranteed to be thread-safe by users.
unsafe impl Send for SessionCallbacks {}
unsafe impl Sync for SessionCallbacks {}

/// Creates a session builder with OpenAI provider.
///
/// `out` will be set to a pointer to the session builder if the call succeeds.
///
/// The caller must either free the builder or use it to create a session, or
/// the resources will be leaked.
///
/// # Safety
///
/// Strings passed to this function must contain a valid nul terminator at the
/// end of the string. `out` must be a valid pointer that points to a pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn la_session_builder_new_openai(
    out: *mut *mut c_void,
    api_key: *const c_char,
    base_url: *const c_char,
    model: *const c_char,
) -> ErrorCode {
    // SAFETY: Assume the caller has provided the valid pointers.
    let (api_key, base_url, model) = unsafe {
        let Ok(api_key) = CStr::from_ptr(api_key).to_str() else {
            return ErrorCode::Invalid;
        };
        let Ok(base_url) = CStr::from_ptr(base_url).to_str() else {
            return ErrorCode::Invalid;
        };
        let Ok(model) = CStr::from_ptr(model).to_str() else {
            return ErrorCode::Invalid;
        };
        (api_key, base_url, model)
    };

    let config = OpenAIConfigBuilder::with_api_key(api_key)
        .with_base_url(base_url)
        .with_model(model)
        .build();
    let model_provider = OpenAIProvider::new(config);
    let builder = SessionBuilder::with_model_provider(model_provider);
    let builder_wrapper_ptr = Box::into_raw(Box::new(SessionBuilderWrapper {
        builder: Some(builder),
    }));
    // SAFETY: Assume `out` is valid and properly aligned.
    unsafe {
        (out as *mut *mut SessionBuilderWrapper).write(builder_wrapper_ptr);
    }

    ErrorCode::Ok
}

/// Sets the callbacks for the session builder.
///
/// # Safety
///
/// `builder` must be a valid pointer returned from the creation functions of
/// session builder. `callbacks` must be a valid pointer to `SessionCallbacks`
/// value, and all fields must be either valid pointers or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn la_session_builder_set_callbacks(
    builder: *mut c_void,
    callbacks: *const SessionCallbacks,
) {
    /// Add reference-counting for the user info, so it can be safely
    /// freed when it's no longer needed.
    struct Wrapper {
        callbacks: SessionCallbacks,
    }

    impl Deref for Wrapper {
        type Target = SessionCallbacks;

        fn deref(&self) -> &Self::Target {
            &self.callbacks
        }
    }

    impl Drop for Wrapper {
        fn drop(&mut self) {
            if let Some(free) = self.callbacks.free {
                // SAFETY: Assume the callback is valid.
                unsafe { free(self.callbacks.user_info) };
            }
        }
    }

    // SAFETY: Assume the callback is valid.
    let callbacks = unsafe { *callbacks };
    let wrapper = Arc::new(Wrapper { callbacks });

    // SAFETY: Assume the caller has provided the valid pointer.
    let builder_wrapper =
        unsafe { &mut *(builder as *mut SessionBuilderWrapper) };
    let mut builder = builder_wrapper.builder.take().unwrap();
    if callbacks.on_idle.is_some() {
        builder = builder.on_idle({
            let wrapper = Arc::clone(&wrapper);
            move || {
                unsafe { (wrapper.on_idle.unwrap())(wrapper.user_info) };
            }
        });
    }
    if callbacks.on_transcript.is_some() {
        builder = builder.on_transcript({
            let wrapper = Arc::clone(&wrapper);
            move |transcript, source| {
                let source = match source {
                    TranscriptSource::User => 0,
                    TranscriptSource::Assistant => 1,
                };
                unsafe {
                    (wrapper.on_transcript.unwrap())(
                        wrapper.user_info,
                        transcript.as_ptr() as *const _,
                        transcript.len(),
                        source,
                    )
                };
            }
        });
    }
    if callbacks.on_tool_call_request.is_some() {
        builder = builder.on_tool_call_request({
            let wrapper = Arc::clone(&wrapper);
            move |approval| {
                let approval_ptr = Box::into_raw(Box::new(approval));
                unsafe {
                    (wrapper.on_tool_call_request.unwrap())(
                        wrapper.user_info,
                        approval_ptr as *mut _,
                    )
                };
            }
        });
    }
    builder_wrapper.builder = Some(builder);
}

/// Frees a previously initialized session builder.
///
/// # Safety
///
/// `builder` must be a valid pointer returned from the creation functions of
/// session builder.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn la_session_builder_free(builder: *mut c_void) {
    // SAFETY: Assume the caller has provided the valid pointer.
    unsafe {
        let builder_wrapper_ptr = builder as *mut SessionBuilderWrapper;
        drop(Box::from_raw(builder_wrapper_ptr));
    }
}

/// Builds a session from a previously initialized session builder.
///
/// Note that the session builder is consumed and cannot be used again after
/// this call.
///
/// # Safety
///
/// `builder` must be a valid pointer returned from the creation functions of
/// session builder.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn la_session_builder_build(
    builder: *mut c_void,
) -> *mut c_void {
    // We must enter the runtime before building the session, since it will
    // spawn the agent actor, which requires a runtime.
    let runtime = &*TOKIO_RUNTIME;
    let _enter = runtime.enter();

    // SAFETY: Assume the caller has provided the valid pointer.
    let mut builder_wrapper = unsafe {
        let builder_wrapper_ptr = builder as *mut SessionBuilderWrapper;
        Box::from_raw(builder_wrapper_ptr)
    };
    let session = builder_wrapper.builder.take().unwrap().build();
    let session_ptr = Box::into_raw(Box::new(session));
    session_ptr as _
}

/// Sends a message to the session.
///
/// # Safety
///
/// `session` must be a valid pointer returned from `la_session_builder_build`.
/// String pointed by `message` must contain a valid nul terminator at the end
/// of the string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn la_session_send_message(
    session: *mut c_void,
    message: *const c_char,
) -> ErrorCode {
    let Ok(message) = unsafe { CStr::from_ptr(message) }.to_str() else {
        return ErrorCode::Invalid;
    };

    // SAFETY: Assume the caller has provided the valid pointer.
    let session = unsafe { &*(session as *mut Session) };
    session.send_message(message);

    ErrorCode::Ok
}

/// Approves a tool call request.
///
/// This function consumes the approval object, which makes it no longer
/// available for further use.
///
/// # Safety
///
/// `approval` must be a valid pointer returned from `on_tool_call_request`
/// callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn la_tool_approval_approve(approval: *mut c_void) {
    // SAFETY: Assume the caller has provided the valid pointer.
    let approval = unsafe {
        let approval_ptr = approval as *mut ToolApproval;
        Box::from_raw(approval_ptr)
    };
    approval.approve();
}

/// Rejects a tool call request.
///
/// This function consumes the approval object, which makes it no longer
/// available for further use.
///
/// # Safety
///
/// `approval` must be a valid pointer returned from `on_tool_call_request`
/// callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn la_tool_approval_reject(approval: *mut c_void) {
    // SAFETY: Assume the caller has provided the valid pointer.
    let approval = unsafe {
        let approval_ptr = approval as *mut ToolApproval;
        Box::from_raw(approval_ptr)
    };
    approval.reject(None);
}

/// Gets what the tool is requesting.
///
/// On return, `out_len` is set to the length of the string. Caller can only
/// use the string before consuming the approval object.
///
/// # Safety
///
/// `approval` must be a valid pointer returned from `on_tool_call_request`
/// callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn la_tool_approval_get_what(
    approval: *mut c_void,
    out_len: *mut usize,
) -> *const c_char {
    // SAFETY: Assume the caller has provided the valid pointer.
    let approval = unsafe { &*(approval as *mut ToolApproval) };
    let what = approval.what();
    // SAFETY: Assume the caller has provided the valid pointer.
    unsafe { out_len.write(what.len()) };
    what.as_ptr() as _
}

/// Gets justification for the tool call request.
///
/// On return, `out_len` is set to the length of the string. Caller can only
/// use the string before consuming the approval object.
///
/// # Safety
///
/// `approval` must be a valid pointer returned from `on_tool_call_request`
/// callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn la_tool_approval_get_justification(
    approval: *mut c_void,
    out_len: *mut usize,
) -> *const c_char {
    // SAFETY: Assume the caller has provided the valid pointer.
    let approval = unsafe { &*(approval as *mut ToolApproval) };
    let justification = approval.justification();
    // SAFETY: Assume the caller has provided the valid pointer.
    unsafe { out_len.write(justification.len()) };
    justification.as_ptr() as _
}

#ifndef _LITTLE_AGENT_H_
#define _LITTLE_AGENT_H_

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Error codes returned by the C APIs. */
typedef enum la_error_code {
  /* No error occurred. */
  LA_ERROR_OK      = 0,
  /* Invalid parameters or strings. */
  LA_ERROR_INVALID = 1
} la_error_code_t;

/* Transcript sources. */
typedef enum la_transcript_source {
  LA_TRANSCRIPT_SOURCE_USER      = 0,
  LA_TRANSCRIPT_SOURCE_ASSISTANT = 1
} la_transcript_source_t;

/* Opaque representation of a session builder. */
typedef struct la_session_builder la_session_builder_t;
/* Opaque representation of a session. */
typedef struct la_session la_session_t;
/* Opaque representation of a tool call approval object. */
typedef struct la_tool_approval la_tool_approval_t;

/**
 * Callbacks for various events from the session.
 *
 * Note that callback functions and `user_info` are assumed to be thread-safe
 * and able to send across the thread boundaries.
 */
typedef struct la_session_callbacks {
  /* User-defined data to be passed to the callbacks. */
  void *user_info;
  /* Callback to handle the session becoming idle. */
  void (*on_idle)(void *user_info);
  /**
   * Callback to handle the generated transcripts.
   *
   * @param user_info      The user-defined data.
   * @param transcript     Transcript string.
   * @param transcript_len Length of the transcript string.
   * @param source         Transcript source.
   */
  void (*on_transcript)(
      void *user_info,
      const char *transcript,
      size_t transcript_len,
      la_transcript_source_t source);
  /**
   * Callback to handle the tool call request.
   *
   * @param user_info The user-defined data.
   * @param approval  A tool call approval object. You must either call
   *                  `la_tool_approval_approve` or `la_tool_approval_reject`
   *                  to consume the approval object, or it will be leaked.
   */
  void (*on_tool_call_request)(
      void *user_info,
      la_tool_approval_t *approval);
  /* Callback to free the user-defined data. */
  void (*free)(void *user_info);
} la_session_callbacks_t;

/**
 * Creates a session builder with OpenAI provider.
 *
 * @param[out]     Pointer to the session builder, which will be set if the
 *                 call succeeds.
 * @param api_key  API key for the OpenAI provider.
 * @param base_url Base URL for the OpenAI provider.
 * @param model    Model to use.
 *
 * @return 0 or an error code.
 *
 * The caller must either free the builder or use it to create a session, or
 * the resources will be leaked.
 */
la_error_code_t la_session_builder_new_openai(
    la_session_builder_t **out,
    const char *api_key,
    const char *base_url,
    const char *model);

/* Sets the callbacks for the session builder. */
void la_session_builder_set_callbacks(
    la_session_builder_t *builder,
    const la_session_callbacks_t *callbacks);

/* Frees a previously initialized session builder. */
void la_session_builder_free(la_session_builder_t *builder);

/**
 * Builds a session from a previously initialized session builder.
 *
 * Note that the session builder is consumed and cannot be used again after
 * this call.
 */
la_session_t *la_session_builder_build(la_session_builder_t *builder);

/* Sends a message to the session. */
la_error_code_t la_session_send_message(
    la_session_t *session,
    const char *message);

/* Approves a tool call request. */
void la_tool_approval_approve(la_tool_approval_t *approval);

/* Rejects a tool call request. */
void la_tool_approval_reject(la_tool_approval_t *approval);

/**
 * Gets what the tool is requesting.
 *
 * On return, `out_len` is set to the length of the string. Caller can only
 * use the string before consuming the approval object.
 */
const char *la_tool_approval_get_what(
    la_tool_approval_t *approval,
    size_t *out_len);

/**
 * Gets justification for the tool call request.
 *
 * On return, `out_len` is set to the length of the string. Caller can only
 * use the string before consuming the approval object.
 */
const char *la_tool_approval_get_justification(
    la_tool_approval_t *approval,
    size_t *out_len);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* _LITTLE_AGENT_H_ */

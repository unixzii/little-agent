# little-agent

The project is organized into several crates, and there are some key crates and types:

- `core`: Core logic of the agent, including the agent state, conversation management, and model client.
  - `Agent`: Entry point of the agent, which maintains the agent state and provides methods to interact with it.
  - `ModelClient`: A wrapper around the model provider. Agents can use it to send requests to the model.
  - `Tool`: A trait that tools implement to provide external abilities to the agent.
- `model`: An abstraction layer for different LLMs, making model provider opaque to `core`.
  - `ModelProvider`: A trait that model providers implement, which can be used to send model requests.
  - `ModelRequest`: A concrete type that represents a model request (including messages, tools, etc.).
  - `ModelResponse`: A trait that represents a response from the model.
- `test-model`: A fake model provider for testing purpose. Use this to implement tests for other crates.
- `actor`: A simple util module that enables actor-oriented programming.

`little-agent` crate is a bundle of the core module and some built-in tools. It provides an out-of-the-box agent that can be used as a library (see `lib.rs`). There is also a CLI (see `main.rs`) that user can use directly.

## Development guidance

Most works should be done in `core` crate, and you may update `model` crate if you need more model capabilities. Other foundation crates should be rarely touched, unless asked specifically. New features added to `core` should also be exposed by `little-agent` if necessary.

### Common rules

- All tests should be passed in the end, and never add workarounds to the test code to make them pass.
- When adding new types, write comprehensive docs and tests for them.
- Make each change reviewable, do minimal changes in one session.
- Don't search / grep the whole project eagerly, unless you indeed miss the context.
- Write compact and clean functions, deeply nested code should be definitely avoided.

### Agent-related rules

- `Agent` is an actor, external code can only interact with it by sending messages.
- Always notify state changes via callbacks, getter methods are not allowed.
- When adding options for `Agent`, prefer to add them to `AgentBuilder`.

## Tests

The project only has unit tests currently. To run them, just execute `cargo test`.

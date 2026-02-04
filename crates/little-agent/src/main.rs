//! A simple program demonstrates how to use `little-agent` as a library.

#[macro_use]
extern crate tracing;

use std::env;
use std::io::Write as _;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use little_agent::SessionBuilder;
use little_agent::tools::ShellToolApproval;
use little_agent_core::TranscriptSource;
use little_agent_openai_model::{OpenAIConfigBuilder, OpenAIProvider};
use owo_colors::OwoColorize;
use tokio::io::{self, AsyncBufReadExt};
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::sleep;

enum SessionEvent {
    Idle,
    Transcript(String, TranscriptSource),
    ShellRequest(ShellToolApproval),
}

const BAR_CHAR: &str = "▎";

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let Ok(api_key) = env::var("OPENAI_API_KEY") else {
        eprintln!("OPENAI_API_KEY environment variable is not set");
        return;
    };
    let Ok(base_url) = env::var("OPENAI_BASE_URL") else {
        eprintln!("OPENAI_BASE_URL environment variable is not set");
        return;
    };
    let Ok(model) = env::var("OPENAI_MODEL") else {
        eprintln!("OPENAI_MODEL environment variable is not set");
        return;
    };

    let config = OpenAIConfigBuilder::with_api_key(api_key)
        .with_base_url(base_url)
        .with_model(model)
        .build();
    let model_provider = OpenAIProvider::new(config);

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    let session = SessionBuilder::with_model_provider(model_provider)
        .with_system_prompt(
            include_str!("./system_prompt.md")
                .replace("{{HOST_OS}}", host_os()),
        )
        .on_idle({
            let event_tx = event_tx.clone();
            move || {
                event_tx.send(SessionEvent::Idle).ok();
            }
        })
        .on_transcript({
            let event_tx = event_tx.clone();
            move |transcript, source| {
                event_tx
                    .send(SessionEvent::Transcript(
                        transcript.to_owned(),
                        source,
                    ))
                    .ok();
            }
        })
        .on_shell_request({
            let event_tx = event_tx.clone();
            move |approval| {
                event_tx.send(SessionEvent::ShellRequest(approval)).ok();
            }
        })
        .build();

    let progress_style = ProgressStyle::with_template("{spinner} {wide_msg}")
        .unwrap()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏");

    'outer: loop {
        print!("> ");
        std::io::stdout().flush().unwrap();

        let Some(line) = read_line().await else {
            break;
        };
        session.send_message(line.trim());

        let mut progress_bar = None;

        loop {
            // Create a new progress bar if it has been finished.
            progress_bar
                .get_or_insert_with(|| {
                    let progress_bar = ProgressBar::new_spinner();
                    progress_bar.set_style(progress_style.clone());
                    progress_bar.set_message("🤔 Thinking...");
                    progress_bar
                })
                .inc(1);

            let sleep = sleep(Duration::from_millis(100));
            let event = select! {
                event = event_rx.recv() => {
                    let Some(event) = event else {
                        break 'outer;
                    };
                    event
                },
                _ = sleep => {
                    continue;
                }
            };

            // Finish the progress bar before printing anything else.
            if let Some(progress_bar) = &progress_bar {
                progress_bar.finish_and_clear();
            }
            progress_bar = None;

            match event {
                SessionEvent::ShellRequest(approval) => {
                    let bar = BAR_CHAR.bright_yellow();
                    println!("\n{bar}⚠️  Agent wants to run command:");
                    println!(
                        "{bar}{}",
                        approval.cmdline().bright_white().bold()
                    );
                    print!("Proceed? [Y/n]: ");
                    std::io::stdout().flush().unwrap();

                    let Some(line) = read_line().await else {
                        break 'outer;
                    };
                    let line = line.trim();
                    if line.is_empty() || line.eq_ignore_ascii_case("y") {
                        approval.approve();
                    } else {
                        approval.reject();
                    }

                    println!();
                }
                SessionEvent::Transcript(transcript, source) => {
                    if source == TranscriptSource::Assistant {
                        println!(
                            "{}🤖 {}",
                            BAR_CHAR.bright_cyan(),
                            transcript.bright_white()
                        );
                    }
                }
                SessionEvent::Idle => {
                    break;
                }
            }
        }
    }
}

async fn read_line() -> Option<String> {
    let mut stdin = io::BufReader::new(io::stdin());
    let mut line = String::new();

    match stdin.read_line(&mut line).await {
        Ok(count) => {
            if count == 0 {
                return None;
            }
            Some(line)
        }
        Err(err) => {
            error!("error reading input: {}", err);
            None
        }
    }
}

#[inline]
fn host_os() -> &'static str {
    let os = std::env::consts::OS;
    match os {
        "linux" => "Linux",
        "macos" => "macOS",
        "windows" => "Windows",
        _ => "some other OS",
    }
}

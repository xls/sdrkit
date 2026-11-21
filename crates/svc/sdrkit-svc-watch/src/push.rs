//! Push channels for alerts: ntfy, a Telegram bot, and a webhook with JSON. Each channel runs on its own thread behind a
//! bounded queue. Tokens never show in a log, an error or a debug print.

use std::sync::Arc;
use std::time::Duration;

use sdrkit_svc_outputs::http::{bearer, post};
use sdrkit_svc_outputs::{BadSetting, OutputError, Sink, Transport};
use serde::{Deserialize, Serialize};

use crate::watch::Alert;

/// A push channel.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "channel", rename_all = "snake_case")]
pub enum PushChannel {
    /// ntfy: a POST of the text to `<server>/<topic>`, with the title and priority as headers.
    Ntfy {
        /// Server base address (`https://ntfy.sh` or the user's own).
        server: String,
        /// Topic.
        topic: String,
        /// Access token (empty: none).
        #[serde(default)]
        token: String,
    },
    /// A Telegram bot: `sendMessage` to a chat.
    Telegram {
        /// The API base address (`https://api.telegram.org`; a test server in tests).
        #[serde(default = "telegram_api")]
        api: String,
        /// The bot token.
        bot_token: String,
        /// The chat id.
        chat_id: String,
    },
    /// A generic webhook: the alert as JSON.
    Webhook {
        /// Address.
        url: String,
        /// Bearer token (empty: none).
        #[serde(default)]
        token: String,
    },
}

fn telegram_api() -> String {
    "https://api.telegram.org".into()
}

impl std::fmt::Debug for PushChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PushChannel::Ntfy { server, topic, .. } => {
                write!(f, "Ntfy({server}/{topic}, token hidden)")
            }
            PushChannel::Telegram { api, chat_id, .. } => {
                write!(f, "Telegram({api}, chat {chat_id}, token hidden)")
            }
            PushChannel::Webhook { url, .. } => write!(f, "Webhook({url}, token hidden)"),
        }
    }
}

impl PushChannel {
    /// Sends one alert now (on the caller's thread).
    pub fn send(&self, transport: &dyn Transport, a: &Alert) -> Result<(), OutputError> {
        let timeout = Duration::from_secs(15);
        let hide = |e: OutputError| match e {
            // A setting error could quote an address holding a token: say less.
            OutputError::Setting(_) => OutputError::Setting(BadSetting::Address),
            other => other,
        };
        match self {
            PushChannel::Ntfy {
                server,
                topic,
                token,
            } => {
                let url = format!(
                    "{}/{}",
                    server.trim_end_matches('/'),
                    topic.trim_matches('/')
                );
                let auth = bearer(token);
                let priority = if a.urgent { "urgent" } else { "default" };
                // Header values must be one line.
                let title = a.title.replace(['\r', '\n'], " ");
                let mut headers = vec![
                    ("Title", title.as_str()),
                    ("Priority", priority),
                    ("Tags", "satellite_antenna"),
                ];
                if let Some(x) = auth.as_deref() {
                    headers.push(("Authorization", x));
                }
                post(
                    transport,
                    &url,
                    "text/plain; charset=utf-8",
                    &headers,
                    a.body.as_bytes(),
                    timeout,
                )
                .map_err(hide)?;
            }
            PushChannel::Telegram {
                api,
                bot_token,
                chat_id,
            } => {
                let url = format!("{}/bot{}/sendMessage", api.trim_end_matches('/'), bot_token);
                let body = serde_json::to_vec(&TelegramMessage {
                    chat_id,
                    text: format!("{}\n{}", a.title, a.body),
                    disable_notification: !a.urgent,
                })?;
                post(transport, &url, "application/json", &[], &body, timeout).map_err(hide)?;
            }
            PushChannel::Webhook { url, token } => {
                let auth = bearer(token);
                let headers: Vec<(&str, &str)> = auth
                    .as_deref()
                    .map(|x| ("Authorization", x))
                    .into_iter()
                    .collect();
                let body = serde_json::to_vec(a)?;
                post(transport, url, "application/json", &headers, &body, timeout).map_err(hide)?;
            }
        }
        Ok(())
    }
}

/// The body of a Telegram `sendMessage`.
#[derive(Serialize)]
struct TelegramMessage<'a> {
    chat_id: &'a str,
    text: String,
    disable_notification: bool,
}

pub(crate) struct ChannelSink {
    pub channel: PushChannel,
    pub transport: Arc<dyn Transport>,
}

impl Sink<Arc<Alert>> for ChannelSink {
    fn deliver(&mut self, a: &Arc<Alert>) -> Result<(), OutputError> {
        self.channel.send(&*self.transport, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_never_show() {
        let c = PushChannel::Telegram {
            api: telegram_api(),
            bot_token: "123:SECRET".into(),
            chat_id: "42".into(),
        };
        assert!(!format!("{c:?}").contains("SECRET"));
        let n = PushChannel::Ntfy {
            server: "https://ntfy.example".into(),
            topic: "t".into(),
            token: "tk_SECRET".into(),
        };
        assert!(!format!("{n:?}").contains("SECRET"));
        let bad = PushChannel::Telegram {
            api: "not a url".into(),
            bot_token: "123:SECRET".into(),
            chat_id: "42".into(),
        };
        let a = Alert::test(0.0);
        let e = bad
            .send(&sdrkit_core::net::FakeTransport::new(), &a)
            .unwrap_err()
            .to_string();
        assert!(!e.contains("SECRET"), "{e}");
    }
}

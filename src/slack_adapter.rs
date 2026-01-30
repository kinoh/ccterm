use crate::config::SlackConfig;
use crate::types::{IncomingMessage, OutgoingMessage};
use anyhow::{Context, Result};
use slack_morphism::prelude::*;
use slack_morphism::prelude::SlackClientHyperHttpsConnector;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Clone)]
struct SlackBridge {
    tx: mpsc::UnboundedSender<IncomingMessage>,
}

pub struct SlackAdapter {
    client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
    bot_token: SlackApiToken,
    rx: mpsc::UnboundedReceiver<IncomingMessage>,
}

impl SlackAdapter {
    pub async fn connect(cfg: &SlackConfig) -> Result<Self> {
        eprintln!("slack: connecting (socket mode)");
        let (tx, rx) = mpsc::unbounded_channel();
        let connector = SlackClientHyperHttpsConnector::new()
            .context("failed to create slack hyper connector")?;
        let client: Arc<SlackClient<SlackClientHyperHttpsConnector>> =
            Arc::new(SlackClient::new(connector));
        let bot_token = SlackApiToken::new(SlackApiTokenValue(cfg.bot_token.clone()));
        let app_token = SlackApiToken::new(SlackApiTokenValue(cfg.app_token.clone()));

        let env = Arc::new(
            SlackClientEventsListenerEnvironment::new(client.clone())
                .with_user_state(SlackBridge { tx }),
        );

        let callbacks = SlackSocketModeListenerCallbacks::new()
            .with_push_events(push_events_callback::<SlackClientHyperHttpsConnector>);

        let socket_mode_config = SlackClientSocketModeConfig::new();
        let socket_mode_listener =
            SlackClientSocketModeListener::new(&socket_mode_config, env, callbacks);

        socket_mode_listener
            .listen_for(&app_token)
            .await
            .context("failed to register socket mode listener")?;
        eprintln!("slack: socket mode listener registered");

        tokio::spawn(async move {
            eprintln!("slack: socket mode listener starting");
            socket_mode_listener.start().await;
            eprintln!("slack: socket mode listener stopped");
        });

        Ok(SlackAdapter {
            client,
            bot_token,
            rx,
        })
    }

    pub fn incoming(&mut self) -> &mut mpsc::UnboundedReceiver<IncomingMessage> {
        &mut self.rx
    }

    pub async fn send(&self, message: &OutgoingMessage) -> Result<()> {
        eprintln!(
            "slack: sending message channel={} thread={}",
            message.conversation_id,
            message.thread_id.as_deref().unwrap_or("-")
        );
        let session = self.client.open_session(&self.bot_token);
        let mut req = SlackApiChatPostMessageRequest {
            channel: SlackChannelId(message.conversation_id.clone()),
            content: SlackMessageContent {
                text: Some(message.text.clone()),
                blocks: None,
                attachments: None,
                upload: None,
                files: None,
                reactions: None,
                metadata: None,
            },
            as_user: None,
            icon_emoji: None,
            icon_url: None,
            link_names: None,
            parse: None,
            thread_ts: None,
            username: None,
            reply_broadcast: None,
            unfurl_links: None,
            unfurl_media: None,
        };

        if let Some(thread_id) = &message.thread_id {
            req.thread_ts = Some(SlackTs(thread_id.clone()));
        }

        session
            .chat_post_message(&req)
            .await
            .context("failed to post slack message")?;
        eprintln!("slack: sent message");
        Ok(())
    }
}

async fn push_events_callback<SCHC>(
    event: SlackPushEventCallback,
    _client: Arc<SlackClient<SCHC>>,
    state: SlackClientEventsUserState,
) -> UserCallbackResult<()>
where
    SCHC: SlackClientHttpConnector + Send + Sync + 'static,
{
    let bridge = {
        let guard = state.read().await;
        guard
            .get_user_state::<SlackBridge>()
            .cloned()
            .ok_or_else(|| "missing slack bridge")?
    };

    if let SlackEventCallbackBody::AppMention(app_mention) = event.event {
        eprintln!("slack: received app_mention event");
        let text = app_mention
            .content
            .text
            .unwrap_or_else(|| "".to_string());
        let channel = app_mention
            .origin
            .channel
            .map(|c| c.to_string())
            .unwrap_or_default();
        let thread_id = app_mention.origin.thread_ts.map(|ts| ts.to_string());
        let timestamp = Some(app_mention.origin.ts.to_string());

        if !text.trim().is_empty() && !channel.is_empty() {
            eprintln!(
                "slack: app_mention -> incoming channel={} thread={}",
                channel,
                thread_id.as_deref().unwrap_or("-")
            );
            let _ = bridge.tx.send(IncomingMessage {
                text,
                conversation_id: channel,
                thread_id,
                timestamp,
            });
        } else {
            eprintln!(
                "slack: app_mention ignored (empty text or channel) channel={} text_len={}",
                channel,
                text.len()
            );
        }
    }

    Ok(())
}

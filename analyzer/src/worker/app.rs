use crate::worker::analyzer::Analyzer;
use anyhow::{Context, Result, anyhow, bail};
use async_nats::jetstream::{
    AckKind,
    consumer::{Consumer, pull},
};
use async_redis_lock::Lock;
use bytes::Bytes;
use serde::{Serialize, de::DeserializeOwned};
use std::{ops::Deref, pin::Pin, sync::Arc, time::Duration};
use tokio::time::{self, Instant, Sleep};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

pub struct AppInner {
    analyzer: Analyzer,
    cancel: CancellationToken,
}

#[derive(Clone)]
pub struct App<D, T, U, C>
where
    D: Worker<T, U, C>,
    T: Serialize,
    U: DeserializeOwned,
{
    inner: Arc<AppInner>,
    derive_input: D,
    _t: std::marker::PhantomData<T>,
    _u: std::marker::PhantomData<U>,
    _c: std::marker::PhantomData<C>,
}

impl<D, T, U, C> Deref for App<D, T, U, C>
where
    D: Worker<T, U, C>,
    T: Serialize,
    U: DeserializeOwned,
{
    type Target = AppInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub enum DeriveResult<T, U>
where
    T: Serialize,
{
    Ready(Work<T, U>),
    Pending { retry_after: Option<Duration> },
    Discard,
}

pub struct Work<T, U>
where
    T: Serialize,
{
    pub input: T,
    pub context: U,
    pub lock: Option<Lock>,
}

pub trait Worker<T, U, C>
where
    T: Serialize,
    U: DeserializeOwned,
{
    async fn derive_input(&self, subject: &str, payload: &Bytes) -> Result<DeriveResult<T, C>>;
    async fn post(&self, context: C, analysis: U) -> Result<()>;
}

impl<D, T, U, C> App<D, T, U, C>
where
    D: Worker<T, U, C>,
    T: Serialize,
    U: DeserializeOwned,
{
    pub fn new(analyzer: Analyzer, cancel: CancellationToken, derive_input: D) -> Self {
        Self {
            derive_input,
            _t: std::marker::PhantomData,
            _u: std::marker::PhantomData,
            _c: std::marker::PhantomData,
            inner: Arc::new(AppInner { analyzer, cancel }),
        }
    }

    pub async fn run(&self, consumer: Consumer<pull::Config>) -> Result<()> {
        let mut msgs = consumer
            .messages()
            .await
            .context("Failed to get messages")?;
        let mut failure_count = 0;
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => bail!("Context cancelled"),
                msg = msgs.next() => {
                    match msg
                        .transpose()
                        .context("Failed to get message from stream")? {
                        Some(msg) => if let Err(e) = self.handle_inner(msg.clone()).await {
                            failure_count += 1;
                            eprintln!("Error handling message: {:?}", e);
                            dorch_common::wait::wait(&self.cancel, failure_count).await?;
                        } else if let Err(e) = msg.ack().await {
                            failure_count += 1;
                            eprintln!("Error acknowledging message: {:?}", e);
                            dorch_common::wait::wait(&self.cancel, failure_count).await?;
                        } else {
                            failure_count = 0;
                        }
                        None => bail!("No more messages; shutting down worker"),
                    }
                }
            }
        }
    }

    async fn handle_inner(&self, msg: async_nats::jetstream::Message) -> Result<()> {
        let Work {
            input,
            context,
            lock: _lock,
        } = match self
            .derive_input
            .derive_input(&msg.subject, &msg.payload)
            .await
            .context("Failed to derive input")?
        {
            DeriveResult::Ready(work) => work,
            DeriveResult::Pending { retry_after } => {
                // requeue the message to be retried after the duration
                msg.ack_with(AckKind::Nak(retry_after))
                    .await
                    .map_err(|e| anyhow!("Failed to nack message: {e:?}"))?;
                return Ok(());
            }
            DeriveResult::Discard => {
                msg.ack()
                    .await
                    .map_err(|e| anyhow!("Failed to acknowledge message: {e:?}"))?;
                return Ok(());
            }
        };
        let (tx, rx) = tokio::sync::mpsc::channel::<()>(10);
        let cancel = self.cancel.child_token();
        let progress_join = tokio::spawn(report_progress(msg, rx, cancel.clone()));
        let result = self.analyze(input, context, tx, cancel.clone()).await;
        let msg = progress_join
            .await
            .context("Failed to join progress reporting task")?
            .context("Progress reporting task failed")?;
        result.context("Failed to handle inner")?;
        msg.ack()
            .await
            .map_err(|e| anyhow!("Failed to acknowledge message: {e:?}"))
            .context("Failed to acknowledge message")?;
        Ok(())
    }

    async fn analyze_inner(&self, input: T, context: C) -> Result<()> {
        let input_json = serde_json::to_string(&input).context("Failed to serialize input")?;
        let analysis: U = self
            .analyzer
            .analyze(input_json)
            .await
            .context("Failed to analyze")?;
        self.derive_input
            .post(context, analysis)
            .await
            .context("Failed to post analysis")
    }

    async fn analyze(
        &self,
        input: T,
        context: C,
        keepalive: tokio::sync::mpsc::Sender<()>,
        cancel: CancellationToken,
    ) -> Result<()> {
        let cancel = cancel.child_token();
        let _keepalive_task = tokio::spawn({
            let cancel = cancel.clone();
            async move {
                let mut tick = time::interval(Duration::from_secs(10));
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => break,
                        _ = tick.tick() => {
                            _ = keepalive.send(()).await.ok();
                        }
                    }
                }
            }
        });
        let result = self.analyze_inner(input, context).await;
        cancel.cancel();
        _keepalive_task
            .await
            .context("Failed to join keepalive task")?;
        result
    }
}

async fn report_progress(
    msg: async_nats::jetstream::Message,
    mut rx: tokio::sync::mpsc::Receiver<()>,
    cancel: CancellationToken,
) -> Result<async_nats::jetstream::Message> {
    let period = Duration::from_secs(10);
    // Earliest time we're allowed to send the next ack.
    let mut next_allowed = Instant::now();
    // Whether we've seen at least one tick during the current cooldown.
    let mut pending = false;
    // Sleep until next_allowed, armed only when we need it.
    let mut cooldown: Option<Pin<Box<Sleep>>> = None;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(msg),
            tick = rx.recv() => {
                if tick.is_none() {
                    return Ok(msg);
                }
                let now = Instant::now();
                if now >= next_allowed && cooldown.is_none() && !pending {
                    // We're outside the window: ack immediately and open a new 10s window.
                    msg.ack_with(AckKind::Progress)
                        .await
                        .map_err(|e| anyhow!("Failed to extend deadline: {e:?}"))?;
                    next_allowed = now + period;
                    // No need to arm cooldown yet; if more ticks arrive during the window,
                    // we'll arm it then.
                } else {
                    // We're inside the current window - coalesce.
                    pending = true;
                    if cooldown.is_none() {
                        // Sleep until the window boundary to deliver one coalesced ack.
                        cooldown = Some(Box::pin(tokio::time::sleep_until(next_allowed)));
                    }
                }
            }

            // Window boundary reached; if anything was pending, send one ack and start a new window.
            _ = async { if let Some(s) = &mut cooldown { s.as_mut().await } }, if cooldown.is_some() => {
                cooldown = None;
                if pending {
                    msg.ack_with(AckKind::Progress)
                        .await
                        .map_err(|e| anyhow!("Failed to extend deadline: {e:?}"))?;
                    pending = false;
                    next_allowed = Instant::now() + period;
                    // Do not re-arm cooldown here; the next ack (if any) will be triggered by future ticks.
                }
            }
        }
    }
}

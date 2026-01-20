use crate::worker::analyzer::Analyzer;
use anyhow::{Context, Result, anyhow, bail};
use async_nats::jetstream::{
    AckKind,
    consumer::{Consumer, pull},
};
use async_redis_lock::Lock;
use bytes::Bytes;
use serde::{Serialize, de::DeserializeOwned};
use std::{ops::Deref, sync::Arc, time::Duration};
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
    delegate: D,
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
            delegate: derive_input,
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
                            msg.ack_with(AckKind::Nak(Some(Duration::from_secs(7))))
                                .await
                                .map_err(|e| anyhow!("Failed to nack message: {e:?}"))?;
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
            .delegate
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
        self.analyze(input, context)
            .await
            .context("Failed to handle inner")?;
        msg.ack()
            .await
            .map_err(|e| anyhow!("Failed to acknowledge message: {e:?}"))
            .context("Failed to acknowledge message")?;
        Ok(())
    }

    async fn analyze(&self, input: T, context: C) -> Result<()> {
        let input_json = serde_json::to_string(&input).context("Failed to serialize input")?;
        let analysis: U = self
            .analyzer
            .analyze(input_json)
            .await
            .context("Failed to analyze")?;
        self.delegate
            .post(context, analysis)
            .await
            .context("Failed to post analysis")
    }
}

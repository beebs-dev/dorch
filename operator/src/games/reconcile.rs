use super::actions;
use crate::util::{
    self, Error, PROBE_INTERVAL,
    colors::{FG1, FG2},
};
use chrono::Utc;
use dorch_common::annotations;
use dorch_types::*;
use futures::stream::StreamExt;
use k8s_openapi::api::core::v1::{ContainerStatus, Pod};
use kube::{
    Api, ResourceExt,
    client::Client,
    runtime::{Controller, controller::Action},
};
use kube_leader_election::{LeaseLock, LeaseLockParams};
use owo_colors::OwoColorize;
use std::sync::Arc;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

#[cfg(feature = "metrics")]
use crate::util::metrics::ControllerMetrics;

pub async fn run(
    client: Client,
    proxy_image: String,
    downloader_image: String,
    server_image: String,
    spectator_image: String,
    livekit_url: String,
    livekit_secret: String,
    wadinfo_base_url: String,
    strim_base_url: Option<String>,
) -> Result<(), Error> {
    let context: Arc<ContextData> = Arc::new(ContextData::new(
        client.clone(),
        proxy_image,
        downloader_image,
        server_image,
        spectator_image,
        livekit_url,
        livekit_secret,
        wadinfo_base_url,
        strim_base_url,
    ));
    // Namespace where the Lease object lives.
    // Commonly: the controller's namespace. If you deploy in one namespace, hardcode it.
    // If you want it dynamic, inject POD_NAMESPACE via the Downward API.
    let lease_namespace = std::env::var("POD_NAMESPACE").unwrap_or_else(|_| "default".to_string());
    // Unique identity per replica (Downward API POD_NAME is ideal).
    // Fallback to hostname if not present.
    let holder_id = std::env::var("POD_NAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| format!("game-controller-{}", uuid::Uuid::new_v4()));
    // The shared lock name across all replicas
    let lease_name = "game-controller-lock".to_string();
    // TTL: how long leadership is considered valid without renewal.
    // Renew should happen well before TTL expires.
    let lease_ttl = Duration::from_secs(15);
    let renew_every = Duration::from_secs(5);
    let leadership = LeaseLock::new(
        client.clone(),
        &lease_namespace,
        LeaseLockParams {
            holder_id,
            lease_name,
            lease_ttl,
        },
    );

    let shutdown = CancellationToken::new();
    let shutdown_signal = shutdown.clone();
    tokio::spawn(async move {
        dorch_common::shutdown::shutdown_signal().await;
        shutdown_signal.cancel();
    });
    dorch_common::signal_ready();
    println!("{}", "Starting Game controller...".green());
    // We run indefinitely; only the leader runs the controller.
    // On leadership loss, we abort the controller and go back to standby.
    let mut controller_task: Option<tokio::task::JoinHandle<()>> = None;
    let mut tick = tokio::time::interval(renew_every);
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                if let Some(task) = controller_task.take() {
                    task.abort();
                    task.await.ok();
                }
                break Ok(())
            },
            _ = tick.tick() => {}
        }
        let lease = match leadership.try_acquire_or_renew().await {
            Ok(l) => l,
            Err(e) => {
                // If we can't talk to the apiserver / update Lease, assume we are not safe to lead.
                eprintln!("leader election renew/acquire failed: {e}");
                if let Some(task) = controller_task.take() {
                    task.abort();
                    eprintln!("aborted controller due to leader election error");
                }
                continue;
            }
        };
        if lease.acquired_lease {
            // We are leader; ensure controller is running
            if controller_task.is_none() {
                println!("acquired leadership; starting controller");
                let client_for_controller = client.clone();
                let context_for_controller = context.clone();
                let crd_api_for_controller: Api<Game> = Api::all(client_for_controller.clone());
                controller_task = Some(tokio::spawn(async move {
                    println!("{}", "Game controller started.".green());
                    Controller::new(crd_api_for_controller, Default::default())
                        .owns(Api::<Pod>::all(client_for_controller), Default::default())
                        .run(reconcile, on_error, context_for_controller)
                        .for_each(|_res| async move {})
                        .await;
                }));
            }
        } else if let Some(task) = controller_task.take() {
            // We are NOT leader; ensure controller is stopped
            eprintln!("lost leadership; stopping controller");
            task.abort();
        }
    }
}

/// Context injected with each `reconcile` and `on_error` method invocation.
struct ContextData {
    /// Kubernetes client to make Kubernetes API requests with. Required for K8S resource management.
    client: Client,

    #[cfg(feature = "metrics")]
    metrics: ControllerMetrics,

    proxy_image: String,
    downloader_image: String,
    server_image: String,
    spectator_image: String,
    livekit_url: String,
    livekit_secret: String,
    wadinfo_base_url: String,
    strim_base_url: Option<String>,
}

impl ContextData {
    /// Constructs a new instance of ContextData.
    ///
    /// # Arguments:
    /// - `client`: A Kubernetes client to make Kubernetes REST API requests with. Resources
    ///   will be created and deleted with this client.
    pub fn new(
        client: Client,
        proxy_image: String,
        downloader_image: String,
        server_image: String,
        spectator_image: String,
        livekit_url: String,
        livekit_secret: String,
        wadinfo_base_url: String,
        strim_base_url: Option<String>,
    ) -> Self {
        #[cfg(feature = "metrics")]
        {
            ContextData {
                client,
                metrics: ControllerMetrics::new("consumers"),
                proxy_image,
                downloader_image,
                server_image,
                spectator_image,
                livekit_url,
                livekit_secret,
                wadinfo_base_url,
                strim_base_url,
            }
        }
        #[cfg(not(feature = "metrics"))]
        {
            ContextData {
                client,
                proxy_image,
                server_image,
                livekit_url,
                livekit_secret,
                downloader_image,
                wadinfo_base_url,
                strim_base_url,
            }
        }
    }
}

/// Action to be taken upon an `Game` resource during reconciliation
#[derive(Debug, PartialEq)]
enum GameAction {
    /// Create all subresources required by the [`Game`].
    CreatePod,

    Pending {
        reason: String,
    },

    DeletePod {
        reason: String,
    },

    Starting {
        reason: String,
    },

    /// Signals that the [`Game`] is fully reconciled.
    Active {
        pod_name: String,
    },

    /// An error occurred during reconciliation.
    Error(String),

    /// The [`Game`] resource is in desired state and requires no actions to be taken.
    NoOp,

    /// Requeue after given duration.
    Requeue(Duration),

    Terminating {
        reason: String,
    },
}

impl GameAction {
    fn to_str(&self) -> &str {
        match self {
            GameAction::CreatePod => "CreatePod",
            GameAction::DeletePod { .. } => "DeletePod",
            GameAction::Starting { .. } => "Starting",
            GameAction::Active { .. } => "Active",
            GameAction::NoOp => "NoOp",
            GameAction::Error(_) => "Error",
            GameAction::Requeue(_) => "Requeue",
            GameAction::Pending { .. } => "Pending",
            GameAction::Terminating { .. } => "Terminating",
        }
    }
}

/// Reconciliation function for the `Game` resource.
async fn reconcile(instance: Arc<Game>, context: Arc<ContextData>) -> Result<Action, Error> {
    // The `Client` is shared -> a clone from the reference is obtained
    let client: Client = context.client.clone();

    // The resource of `Game` kind is required to have a namespace set. However, it is not guaranteed
    // the resource will have a `namespace` set. Therefore, the `namespace` field on object's metadata
    // is optional and Rust forces the programmer to check for it's existence first.
    let namespace: String = match instance.namespace() {
        None => {
            // If there is no namespace to deploy to defined, reconciliation ends with an error immediately.
            return Err(Error::UserInput(
                "Expected Game resource to be namespaced. Can't deploy to an unknown namespace."
                    .to_owned(),
            ));
        }
        // If namespace is known, proceed. In a more advanced version of the operator, perhaps
        // the namespace could be checked for existence first.
        Some(namespace) => namespace,
    };

    // Name of the Game resource is used to name the subresources as well.
    let name = instance.name_any();

    // Increment total number of reconciles for the Game resource.
    #[cfg(feature = "metrics")]
    context
        .metrics
        .reconcile_counter
        .with_label_values(&[&name, &namespace])
        .inc();

    // Benchmark the read phase of reconciliation.
    #[cfg(feature = "metrics")]
    let start = std::time::Instant::now();

    // Read phase of reconciliation determines goal during the write phase.
    let action = determine_action(client.clone(), &name, &namespace, &instance).await?;

    if action != GameAction::NoOp {
        println!(
            "🔧 {}{}{}{}{}",
            namespace.color(FG2),
            "/".color(FG1),
            name.color(FG2),
            " ACTION: ".color(FG1),
            format!("{:?}", action).color(FG2),
        );
    }

    // Report the read phase performance.
    #[cfg(feature = "metrics")]
    context
        .metrics
        .read_histogram
        .with_label_values(&[&name, &namespace, action.to_str()])
        .observe(start.elapsed().as_secs_f64());

    // Increment the counter for the action.
    #[cfg(feature = "metrics")]
    context
        .metrics
        .action_counter
        .with_label_values(&[&name, &namespace, action.to_str()])
        .inc();

    // Benchmark the write phase of reconciliation.
    #[cfg(feature = "metrics")]
    let timer = match action {
        // Don't measure performance for NoOp actions.
        GameAction::NoOp => None,
        // Start a performance timer for the write phase.
        _ => Some(
            context
                .metrics
                .write_histogram
                .with_label_values(&[&name, &namespace, action.to_str()])
                .start_timer(),
        ),
    };

    // Performs action as decided by the `determine_action` function.
    // This is the write phase of reconciliation.
    let result = match action {
        GameAction::Requeue(duration) => Action::requeue(duration),
        GameAction::Terminating { reason } => {
            actions::terminating(client, &instance, reason).await?;
            Action::await_change()
        }
        GameAction::Pending { reason } => {
            actions::pending(client, &instance, reason).await?;
            Action::await_change()
        }
        GameAction::Starting { reason } => {
            actions::starting(client, &instance, reason).await?;
            Action::await_change()
        }
        GameAction::DeletePod { reason } => {
            actions::delete_pod(client.clone(), &instance, reason).await?;
            Action::await_change()
        }
        GameAction::CreatePod => {
            actions::create_pod(
                client.clone(),
                &instance,
                &context.proxy_image,
                &context.downloader_image,
                &context.server_image,
                &context.spectator_image,
                &context.livekit_url,
                &context.livekit_secret,
                &context.wadinfo_base_url,
                context.strim_base_url.as_deref(),
            )
            .await?;
            Action::await_change()
        }
        GameAction::Error(message) => {
            actions::error(client.clone(), &instance, message).await?;
            Action::await_change()
        }
        GameAction::Active { pod_name } => {
            actions::active(client, &instance, &pod_name).await?;
            Action::requeue(PROBE_INTERVAL)
        }
        GameAction::NoOp => Action::requeue(PROBE_INTERVAL),
    };

    #[cfg(feature = "metrics")]
    if let Some(timer) = timer {
        timer.observe_duration();
    }

    Ok(result)
}

/// Resources arrives into reconciliation queue in a certain state. This function looks at
/// the state of given `Game` resource and decides which actions needs to be performed.
/// The finite set of possible actions is represented by the `GameAction` enum.
///
/// # Arguments
/// - `instance`: A reference to `Game` being reconciled to decide next action upon.
async fn determine_action(
    client: Client,
    _name: &str,
    namespace: &str,
    instance: &Game,
) -> Result<GameAction, Error> {
    // Don't do anything while being deleted.
    if instance.metadata.deletion_timestamp.is_some() {
        return Ok(GameAction::Requeue(Duration::from_secs(2)));
    }

    // Does the pod exist?
    let pod = match get_pod(client.clone(), namespace, &instance.name_any()).await? {
        Some(pod) => pod,
        None => return Ok(GameAction::CreatePod),
    };

    // Don't do anything while the pod is being deleted.
    if pod.metadata.deletion_timestamp.is_some() {
        return Ok(GameAction::Terminating {
            reason: format!("Pod '{}' is being deleted", pod.name_any()),
        });
    }

    // Check the hash
    let desired_hash = util::hash_spec(&instance.spec);
    if pod
        .metadata
        .annotations
        .as_ref()
        .is_none_or(|a| a.get(annotations::SPEC_HASH) != Some(&desired_hash))
    {
        return Ok(GameAction::DeletePod {
            reason: format!("Pod '{}' spec hash mismatch", pod.name_any()),
        });
    }

    if let Some(action) = determine_phase_action(&pod) {
        return Ok(action);
    }

    if let Some(action) = determine_container_action(&pod) {
        return Ok(action);
    }

    match pod_is_ready(&pod) {
        Some(true) => {
            // Keep the Active phase up-to-date
            determine_status_action(instance)
        }
        Some(false) => {
            // Ready condition exists but is False; include the condition message if present
            let msg = pod
                .status
                .as_ref()
                .and_then(|s| s.conditions.as_ref())
                .and_then(|cs| cs.iter().find(|c| c.type_ == "Ready"))
                .and_then(|c| c.message.as_deref())
                .unwrap_or("Ready condition is False");

            Ok(GameAction::Starting {
                reason: format!("Pod '{}' is not Ready: {}", pod.name_any(), msg),
            })
        }
        None => Ok(GameAction::Starting {
            reason: format!(
                "Pod '{}' is running but is lacking the Ready condition",
                pod.name_any()
            ),
        }),
    }
}

fn determine_phase_action(pod: &Pod) -> Option<GameAction> {
    match pod.status.as_ref().and_then(|s| s.phase.as_deref()) {
        Some("Running") => None,
        Some("Pending") => {
            return if let Some(status) = &pod.status
                && let Some(cond) = status
                    .conditions
                    .as_ref()
                    .and_then(|cs| cs.iter().find(|c| c.type_ == "PodScheduled"))
                && cond.status == "False"
                && cond.reason.as_deref() == Some("Unschedulable")
            {
                Some(GameAction::Error(format!(
                    "Pod '{}' is unschedulable: {}",
                    pod.name_any(),
                    cond.message.as_deref().unwrap_or("unspecified"),
                )))
            } else {
                Some(GameAction::Pending {
                    reason: format!("Pod '{}' is still in Pending phase", pod.name_any()),
                })
            };
        }
        Some(v) if ["Succeeded", "Failed"].contains(&v) => Some(GameAction::DeletePod {
            reason: format!("Pod unexpectedly terminated with '{}' phase", v),
        }),
        Some("Unknown") => Some(GameAction::Error(format!(
            "Pod is in Unknown phase; node may be lost or unreachable. Reason: {} Message: {}",
            pod.status
                .as_ref()
                .and_then(|s| s.reason.as_deref())
                .unwrap_or("(no reason provided)"),
            pod.status
                .as_ref()
                .and_then(|s| s.message.as_deref())
                .unwrap_or("(no message provided)")
        ))),
        Some(phase) => Some(GameAction::Error(format!(
            "Pod is in unrecognized phase: {}",
            phase
        ))),
        None => {
            if pod
                .metadata
                .creation_timestamp
                .as_ref()
                .is_some_and(|t| Utc::now().signed_duration_since(t.0).num_seconds() < 10)
            {
                // Pod just created, wait a bit
                Some(GameAction::Requeue(Duration::from_secs(3)))
            } else {
                Some(GameAction::Error("Pod has no status phase".to_string()))
            }
        }
    }
}

fn check_container_status(pod: &Pod, container_status: &ContainerStatus) -> Option<GameAction> {
    let state = match &container_status.state {
        Some(state) => state,
        None => {
            return Some(GameAction::Starting {
                reason: format!(
                    "Pod '{}' container '{}' has no state yet",
                    pod.name_any(),
                    container_status.name,
                ),
            });
        }
    };
    if let Some(ref terminated) = state.terminated {
        let reason = terminated.reason.as_deref().unwrap_or("");
        let kind = match (terminated.exit_code, reason) {
            (0, "Completed") => "completed normally",
            (_, "OOMKilled") => "OOMKilled",
            (_, "ContainerCannotRun") => "container cannot run",
            _ => "terminated with error",
        };
        return Some(GameAction::DeletePod {
            reason: format!(
                "Pod '{}' container '{}' {} (exit code {}, reason: {})",
                pod.name_any(),
                container_status.name,
                kind,
                terminated.exit_code,
                terminated
                    .reason
                    .as_deref()
                    .unwrap_or("(no reason provided)")
            ),
        });
    }
    if let Some(ref waiting) = state.waiting {
        // Note: there may not be a waiting reason, in which case we treat it as not existing.
        let reason_str = waiting.reason.as_deref().unwrap_or("");
        const FATAL_WAITING: &[&'static str] = &[
            "ImagePullBackOff",
            "ErrImageNeverPull",
            "RegistryUnavailable",
            "CreateSandboxError",
            "ErrImagePull",
            "InvalidImageName",
            "CreateContainerConfigError",
            "CreateContainerError",
            "RunContainerError",
        ];
        return if reason_str == "CrashLoopBackOff" {
            // Extract the status code from the last termination state if possible
            if let Some(t) = container_status
                .last_state
                .as_ref()
                .and_then(|last_state| last_state.terminated.as_ref())
            {
                Some(GameAction::DeletePod {
                    reason: format!(
                        "Pod '{}' container '{}' is in CrashLoopBackOff (last exit code {}, reason: {}, restartCount: {})",
                        pod.name_any(),
                        container_status.name,
                        t.exit_code,
                        t.reason.as_deref().unwrap_or("(no reason provided)"),
                        container_status.restart_count,
                    ),
                })
            } else {
                Some(GameAction::DeletePod {
                    reason: format!(
                        "Pod '{}' container '{}' is in CrashLoopBackOff (no last termination details available, restartCount: {})",
                        pod.name_any(),
                        container_status.name,
                        container_status.restart_count,
                    ),
                })
            }
        } else if FATAL_WAITING.contains(&reason_str) {
            Some(GameAction::DeletePod {
                reason: format!(
                    "Pod '{}' container '{}' is in unrecoverable waiting state: {}",
                    pod.name_any(),
                    container_status.name,
                    reason_str,
                ),
            })
        } else {
            Some(GameAction::Starting {
                reason: format!(
                    "Pod '{}' container '{}' is waiting with status '{}'",
                    pod.name_any(),
                    container_status.name,
                    if reason_str.is_empty() {
                        "(no reason provided)"
                    } else {
                        reason_str
                    },
                ),
            })
        };
    }
    state.running.as_ref().and_then(|running| {
        if container_status.ready {
            None
        } else {
            Some(GameAction::Starting {
                reason: format!(
                    "Pod '{}' container '{}' is running but not Ready yet (started_at = {:?})",
                    pod.name_any(),
                    container_status.name,
                    running.started_at,
                ),
            })
        }
    })
}

fn pod_is_ready(pod: &Pod) -> Option<bool> {
    pod.status
        .as_ref()?
        .conditions
        .as_ref()?
        .iter()
        .find(|c| c.type_ == "Ready")
        .map(|c| c.status == "True")
}

fn check_container_statuses(
    pod: &Pod,
    container_statuses: &[ContainerStatus],
) -> Option<GameAction> {
    for container_status in container_statuses {
        if let Some(action) = check_container_status(pod, container_status) {
            return Some(action);
        }
    }
    None
}

fn determine_container_action(pod: &Pod) -> Option<GameAction> {
    if let Some(init_statuses) = pod
        .status
        .as_ref()
        .and_then(|s| s.init_container_statuses.as_ref())
        && let Some(action) = check_container_statuses(pod, init_statuses)
    {
        return Some(action);
    }
    match pod
        .status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
    {
        Some(v) => check_container_statuses(pod, v),
        None => Some(GameAction::Starting {
            reason: format!("Pod '{}' has no container statuses yet", pod.name_any()),
        }),
    }
}

async fn get_pod(client: Client, namespace: &str, name: &str) -> Result<Option<Pod>, Error> {
    let api: Api<Pod> = Api::namespaced(client, namespace);
    match api.get(name).await {
        Ok(pod) => Ok(Some(pod)),
        Err(kube::Error::Api(ae)) if ae.code == 404 => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Determines the action given that the only thing left to do
/// is periodically keeping the Active phase up-to-date.
fn determine_status_action(instance: &Game) -> Result<GameAction, Error> {
    let Some(phase) = get_phase(instance) else {
        return Ok(GameAction::Active {
            pod_name: instance.name_any(),
        });
    };
    let age = get_last_updated(instance).unwrap_or(Duration::from_secs(0));
    if phase != GamePhase::Active || age > PROBE_INTERVAL {
        Ok(GameAction::Active {
            pod_name: instance.name_any(),
        })
    } else {
        Ok(GameAction::NoOp)
    }
}

/// Returns the phase of the Game.
pub fn get_phase(instance: &Game) -> Option<GamePhase> {
    instance.status.as_ref().map(|status| status.phase)
}

pub fn get_last_updated(instance: &Game) -> Option<Duration> {
    let Some(status) = instance.status.as_ref() else {
        return None;
    };
    let Ok(Some(last_updated)) = status
        .last_updated
        .as_ref()
        .map(|l| l.parse::<chrono::DateTime<Utc>>())
        .transpose()
    else {
        return None;
    };
    let age: chrono::Duration = Utc::now() - last_updated;
    let Ok(age) = age.to_std() else {
        return None;
    };
    Some(age)
}

/// Actions to be taken when a reconciliation fails - for whatever reason.
/// Prints out the error to `stderr` and requeues the resource for another reconciliation after
/// five seconds.
///
/// # Arguments
/// - `instance`: The erroneous resource.
/// - `error`: A reference to the `kube::Error` that occurred during reconciliation.
/// - `_context`: Unused argument. Context Data "injected" automatically by kube-rs.
fn on_error(instance: Arc<Game>, error: &Error, _context: Arc<ContextData>) -> Action {
    eprintln!(
        "{}",
        format!("Reconciliation error: {:?} {:?}", error, instance).red()
    );
    Action::requeue(Duration::from_secs(5))
}

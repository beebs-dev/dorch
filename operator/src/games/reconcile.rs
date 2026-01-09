use chrono::Utc;
use dorch_types::*;
use futures::stream::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api, Resource, ResourceExt,
    client::Client,
    runtime::{Controller, controller::Action},
};
use owo_colors::OwoColorize;
use std::sync::Arc;
use tokio::time::Duration;

use super::actions;
use crate::util::{
    Error, PROBE_INTERVAL,
    colors::{FG1, FG2},
};

#[cfg(feature = "metrics")]
use crate::util::metrics::ControllerMetrics;

/// Entrypoint for the `Game` controller.
pub async fn run(client: Client) -> Result<(), Error> {
    println!("{}", "Starting Game controller...".green());

    // Preparation of resources used by the `kube_runtime::Controller`
    let crd_api: Api<Game> = Api::all(client.clone());
    let context: Arc<ContextData> = Arc::new(ContextData::new(client.clone()));

    dorch_common::signal_ready();

    // The controller comes from the `kube_runtime` crate and manages the reconciliation process.
    // It requires the following information:
    // - `kube::Api<T>` this controller "owns". In this case, `T = Game`, as this controller owns the `Game` resource,
    // - `kube::api::ListParams` to select the `Game` resources with. Can be used for Game filtering `Game` resources before reconciliation,
    // - `reconcile` function with reconciliation logic to be called each time a resource of `Game` kind is created/updated/deleted,
    // - `on_error` function to call whenever reconciliation fails.
    Controller::new(crd_api, Default::default())
        .owns(Api::<Pod>::all(client), Default::default())
        .run(reconcile, on_error, context)
        .for_each(|_reconciliation_result| async move {})
        .await;
    Ok(())
}

/// Context injected with each `reconcile` and `on_error` method invocation.
struct ContextData {
    /// Kubernetes client to make Kubernetes API requests with. Required for K8S resource management.
    client: Client,

    #[cfg(feature = "metrics")]
    metrics: ControllerMetrics,
}

impl ContextData {
    /// Constructs a new instance of ContextData.
    ///
    /// # Arguments:
    /// - `client`: A Kubernetes client to make Kubernetes REST API requests with. Resources
    ///   will be created and deleted with this client.
    pub fn new(client: Client) -> Self {
        #[cfg(feature = "metrics")]
        {
            ContextData {
                client,
                metrics: ControllerMetrics::new("consumers"),
            }
        }
        #[cfg(not(feature = "metrics"))]
        {
            ContextData { client }
        }
    }
}

/// Action to be taken upon an `Game` resource during reconciliation
#[derive(Debug, PartialEq)]
enum GameAction {
    /// Create all subresources required by the [`Game`].
    CreatePod,

    DeletePod,

    Starting {
        pod_name: String,
    },

    /// Delete all subresources and remove finalizer only when all subresources are deleted.
    /// If `delete_resource` is true, the [`Game`] resource will be deleted as well.
    Delete,

    /// Signals that the [`Game`] is fully reconciled.
    Active {
        pod_name: String,
    },

    /// An error occurred during reconciliation.
    Error(String),

    /// The [`Game`] resource is in desired state and requires no actions to be taken.
    NoOp,
}

impl GameAction {
    fn to_str(&self) -> &str {
        match self {
            GameAction::CreatePod => "CreatePod",
            GameAction::DeletePod => "DeletePod",
            GameAction::Starting { .. } => "Starting",
            GameAction::Delete => "Delete",
            GameAction::Active { .. } => "Active",
            GameAction::NoOp => "NoOp",
            GameAction::Error(_) => "Error",
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
        GameAction::Starting { pod_name } => {
            // Update the phase to Starting.
            actions::starting(client, &instance, &pod_name).await?;

            Action::await_change()
        }
        GameAction::DeletePod => {
            actions::delete_pod(client.clone(), &instance).await?;

            Action::await_change()
        }
        GameAction::Delete => {
            // Show that the reservation is being terminated.
            actions::terminating(client.clone(), &instance).await?;

            // Remove the finalizer from the Game resource.
            //finalizer::delete::<Game>(client.clone(), &name, &namespace).await?;

            // Child resources will be deleted by kubernetes.
            Action::await_change()
        }
        GameAction::CreatePod => {
            // Add a finalizer so the resource can be properly garbage collected.
            //let instance = finalizer::add(client.clone(), &name, &namespace).await?;
            // Note: finalizer is not required since we do not have custom logic on deletion of child resources.
            actions::create_pod(client.clone(), &instance).await?;

            Action::await_change()
        }
        GameAction::Error(message) => {
            actions::error(client.clone(), &instance, message).await?;

            Action::await_change()
        }
        GameAction::Active { pod_name } => {
            // Update the phase to Active, meaning the reservation is in use.
            actions::active(client, &instance, &pod_name).await?;

            // Resource is fully reconciled.
            Action::requeue(PROBE_INTERVAL)
        }
        // The resource is already in desired state, do nothing and re-check after 10 seconds
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
    if instance.metadata.deletion_timestamp.is_some() {
        return Ok(GameAction::Delete);
    }

    // Does the ffmpeg pod exist?
    let pod = match get_pod(
        client.clone(),
        namespace,
        instance.meta().name.as_ref().unwrap(),
    )
    .await?
    {
        Some(pod) => pod,
        None => return Ok(GameAction::CreatePod),
    };
    let pod_phase = pod.status.as_ref().and_then(|s| s.phase.as_deref());
    match pod_phase {
        Some("Pending") | Some("ContainerCreating") => {
            if instance
                .status
                .as_ref()
                .is_some_and(|s| s.phase == GamePhase::Starting)
            {
                return Ok(GameAction::NoOp);
            }
            return Ok(GameAction::Starting {
                pod_name: pod.meta().name.clone().unwrap(),
            });
        }
        Some("Running") => {}
        Some("Succeeded") | Some("Failed") => {
            return Ok(GameAction::DeletePod);
        }
        _ => {
            return Ok(GameAction::Error("Pod is in unknown state.".to_owned()));
        }
    }

    if let Some(ref status) = pod.status
        && let Some(ref container_statuses) = status.container_statuses
    {
        for container_status in container_statuses {
            if let Some(state) = &container_status.state
                && let Some(ref terminated) = state.terminated
            {
                println!(
                    "Pod's container terminated with exit code {} and reason: {}",
                    terminated.exit_code,
                    terminated
                        .reason
                        .as_ref()
                        .unwrap_or(&"No reason provided".to_string())
                );
                // Recreate the pod
                return Ok(GameAction::DeletePod);
            }
        }
    }

    // Keep the Active status up-to-date.
    determine_status_action(instance)
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
    let (phase, age) = get_game_phase(instance)?;
    if phase != GamePhase::Active || age > PROBE_INTERVAL {
        Ok(GameAction::Active {
            pod_name: instance.meta().name.clone().unwrap(),
        })
    } else {
        Ok(GameAction::NoOp)
    }
}

/// Returns the phase of the Game.
pub fn get_game_phase(instance: &Game) -> Result<(GamePhase, Duration), Error> {
    let status = instance
        .status
        .as_ref()
        .ok_or_else(|| Error::UserInput("No status".to_string()))?;
    let phase = status.phase;
    let last_updated: chrono::DateTime<Utc> = status
        .last_updated
        .as_ref()
        .ok_or_else(|| Error::UserInput("No lastUpdated".to_string()))?
        .parse()?;
    let age: chrono::Duration = Utc::now() - last_updated;
    Ok((phase, age.to_std()?))
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

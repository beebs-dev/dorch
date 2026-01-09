use crate::util::{Error, messages, patch::*};
use dorch_types::*;
use k8s_openapi::api::core::v1::{
    Container, EnvVar, EnvVarSource, ObjectFieldSelector, Pod, PodSpec, SecretKeySelector, Volume,
    VolumeMount,
};
use kube::{
    Api, Client,
    api::{ObjectMeta, Resource},
};

/// Updates the `Game`'s phase to Active.
pub async fn active(client: Client, instance: &Game, peggy_pod_name: &str) -> Result<(), Error> {
    patch_status(client, instance, |status| {
        status.phase = GamePhase::Active;
        status.message = Some(format!(
            "The peggy Pod '{}' is active and running.",
            peggy_pod_name
        ));
    })
    .await?;
    Ok(())
}

/// Updates the `Game`'s phase to Terminating.
pub async fn terminating(client: Client, instance: &Game) -> Result<(), Error> {
    patch_status(client, instance, |status| {
        status.phase = GamePhase::Terminating;
        status.message = Some(messages::TERMINATING.to_owned());
    })
    .await?;
    Ok(())
}

pub async fn delete_pod(client: Client, instance: &Game) -> Result<(), Error> {
    let pods: Api<Pod> =
        Api::namespaced(client.clone(), instance.meta().namespace.as_ref().unwrap());
    pods.delete(instance.meta().name.as_ref().unwrap(), &Default::default())
        .await?;
    Ok(())
}

fn starting_message(pod_name: &str) -> String {
    format!("The peggy Pod '{}' is starting.", pod_name)
}

pub async fn starting(client: Client, instance: &Game, pod_name: &str) -> Result<(), Error> {
    patch_status(client, instance, |status| {
        status.phase = GamePhase::Starting;
        status.message = Some(starting_message(pod_name));
    })
    .await?;
    Ok(())
}

fn game_pod(instance: &Game) -> Pod {
    // For simplicity, we create a pod spec with a single container
    // that runs ffmpeg to stream from the source to the destination
    const HLS_DIR: &str = "/hls";
    let image = String::from("thavlik/synapse-peggy:latest");
    Pod {
        metadata: ObjectMeta {
            name: instance.meta().name.clone(),
            namespace: instance.meta().namespace.clone(),
            owner_references: Some(vec![instance.controller_owner_ref(&()).unwrap()]),
            ..Default::default()
        },
        spec: Some(PodSpec {
            volumes: Some(vec![Volume {
                name: "hls-storage".to_string(),
                empty_dir: Some(Default::default()),
                ..Default::default()
            }]),
            containers: vec![Container {
                name: "ffmpeg".to_string(),
                image: Some(image.clone()),
                image_pull_policy: Some("Always".to_string()),
                command: Some(vec!["/usr/local/bin/run-ffmpeg-hls.sh".to_string()]),
                volume_mounts: Some(vec![VolumeMount {
                    name: "hls-storage".to_string(),
                    mount_path: HLS_DIR.to_string(),
                    ..Default::default()
                }]),
                env: Some(vec![EnvVar {
                    name: "HLS_DIR".to_string(),
                    value: Some(HLS_DIR.to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }],
            restart_policy: Some("Never".to_string()),
            ..Default::default()
        }),
        status: None,
    }
}

pub async fn create_pod(client: Client, instance: &Game) -> Result<(), Error> {
    let pod = game_pod(instance);
    patch_status(client.clone(), instance, |status| {
        status.phase = GamePhase::Starting;
        status.message = Some(starting_message(pod.meta().name.as_ref().unwrap()));
    })
    .await?;
    let pods: Api<Pod> =
        Api::namespaced(client.clone(), instance.meta().namespace.as_ref().unwrap());
    pods.create(&Default::default(), &pod).await?;
    Ok(())
}

pub async fn error(client: Client, instance: &Game, message: String) -> Result<(), Error> {
    patch_status(client.clone(), instance, |status| {
        status.phase = GamePhase::Error;
        status.message = Some(message);
    })
    .await?;
    Ok(())
}

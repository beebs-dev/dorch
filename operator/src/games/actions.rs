use crate::util::{Error, messages, patch::*};
use dorch_types::*;
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EnvVar, EnvVarSource, Pod, PodSpec, SecretKeySelector, Volume,
    VolumeMount,
};
use kube::{
    Api, Client,
    api::{ObjectMeta, Resource},
};

/// Updates the `Game`'s phase to Active.
pub async fn active(client: Client, instance: &Game, pod_name: &str) -> Result<(), Error> {
    patch_status(client, instance, |status| {
        status.phase = GamePhase::Active;
        status.message = Some(format!(
            "The game server pod '{}' is active and running.",
            pod_name
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
    format!("The game server pod '{}' is starting.", pod_name)
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
    const DATA_ROOT: &str = "/data";
    let image = String::from("thavlik/dorch-server:latest");
    let mut game_env = vec![
        EnvVar {
            name: "IWAD".to_string(),
            value: Some(instance.spec.iwad.clone()),
            ..Default::default()
        },
        EnvVar {
            name: "DATA_ROOT".to_string(),
            value: Some(DATA_ROOT.to_string()),
            ..Default::default()
        },
        EnvVar {
            name: "WAD_LIST".to_string(),
            value: Some(instance.spec.files.join(",")),
            ..Default::default()
        },
    ];
    if let Some(warp) = instance.spec.warp.as_deref() {
        game_env.push(EnvVar {
            name: "WARP".to_string(),
            value: Some(warp.to_string()),
            ..Default::default()
        });
    }
    if let Some(skill) = instance.spec.skill {
        game_env.push(EnvVar {
            name: "SKILL".to_string(),
            value: Some(skill.to_string()),
            ..Default::default()
        });
    }
    Pod {
        metadata: ObjectMeta {
            name: instance.meta().name.clone(),
            namespace: instance.meta().namespace.clone(),
            owner_references: Some(vec![instance.controller_owner_ref(&()).unwrap()]),
            ..Default::default()
        },
        spec: Some(PodSpec {
            volumes: Some(vec![Volume {
                name: "data".to_string(),
                empty_dir: Some(Default::default()),
                ..Default::default()
            }]),
            init_containers: Some(vec![Container {
                name: "downloader".to_string(),
                image: Some(image.clone()),
                image_pull_policy: Some("Always".to_string()),
                command: Some(vec!["/download.sh".to_string()]),
                volume_mounts: Some(vec![VolumeMount {
                    name: "data".to_string(),
                    mount_path: DATA_ROOT.to_string(),
                    ..Default::default()
                }]),
                env: Some(vec![
                    EnvVar {
                        name: "DATA_ROOT".to_string(),
                        value: Some(DATA_ROOT.to_string()),
                        ..Default::default()
                    },
                    EnvVar {
                        name: "DOWNLOAD_LIST".to_string(),
                        value: Some({
                            let combined = instance.spec.files.join(",");
                            if instance.spec.files.contains(&instance.spec.iwad) {
                                combined
                            } else {
                                let mut s = instance.spec.iwad.clone();
                                s.push(',');
                                s.push_str(&combined);
                                s
                            }
                        }),
                        ..Default::default()
                    },
                    EnvVar {
                        name: "S3_BUCKET".to_string(),
                        value_from: Some(EnvVarSource {
                            secret_key_ref: Some(SecretKeySelector {
                                name: instance.spec.s3_secret_name.clone(),
                                key: "bucket".to_string(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    EnvVar {
                        name: "S3_REGION".to_string(),
                        value_from: Some(EnvVarSource {
                            secret_key_ref: Some(SecretKeySelector {
                                name: instance.spec.s3_secret_name.clone(),
                                key: "region".to_string(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    EnvVar {
                        name: "S3_ENDPOINT".to_string(),
                        value_from: Some(EnvVarSource {
                            secret_key_ref: Some(SecretKeySelector {
                                name: instance.spec.s3_secret_name.clone(),
                                key: "endpoint".to_string(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    EnvVar {
                        name: "AWS_ACCESS_KEY_ID".to_string(),
                        value_from: Some(EnvVarSource {
                            secret_key_ref: Some(SecretKeySelector {
                                name: instance.spec.s3_secret_name.clone(),
                                key: "access_key_id".to_string(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    EnvVar {
                        name: "AWS_SECRET_ACCESS_KEY".to_string(),
                        value_from: Some(EnvVarSource {
                            secret_key_ref: Some(SecretKeySelector {
                                name: instance.spec.s3_secret_name.clone(),
                                key: "secret_access_key".to_string(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            }]),
            containers: vec![Container {
                name: "game".to_string(),
                image: Some(image.clone()),
                image_pull_policy: Some("Always".to_string()),
                command: Some(vec!["/server.sh".to_string()]),
                volume_mounts: Some(vec![VolumeMount {
                    name: "data".to_string(),
                    mount_path: DATA_ROOT.to_string(),
                    ..Default::default()
                }]),
                ports: Some(vec![ContainerPort {
                    container_port: 5030,
                    protocol: Some("UDP".to_string()),
                    ..Default::default()
                }]),
                env: Some(game_env),
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

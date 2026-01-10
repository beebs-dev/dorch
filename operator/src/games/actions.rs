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

fn game_pod(
    instance: &Game,
    proxy_image: &str,
    server_image: &str,
    livekit_url: &str,
    livekit_secret: &str,
) -> Pod {
    let game_port = 2342;

    // For simplicity, we create a pod spec with a single container
    // that runs ffmpeg to stream from the source to the destination
    const DATA_ROOT: &str = "/data";
    let mut woof_env = vec![
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
    ];
    if let Some(files) = instance.spec.files.as_deref() {
        woof_env.push(EnvVar {
            name: "WAD_LIST".to_string(),
            value: Some(files.join(",")),
            ..Default::default()
        });
    }
    if let Some(warp) = instance.spec.warp.as_deref() {
        woof_env.push(EnvVar {
            name: "WARP".to_string(),
            value: Some(warp.to_string()),
            ..Default::default()
        });
    }
    if let Some(skill) = instance.spec.skill {
        woof_env.push(EnvVar {
            name: "SKILL".to_string(),
            value: Some(skill.to_string()),
            ..Default::default()
        });
    }
    let proxy_env = vec![
        EnvVar {
            name: "GAME_PORT".to_string(),
            value: Some(game_port.to_string()),
            ..Default::default()
        },
        EnvVar {
            name: "GAME_ID".to_string(),
            value: Some(instance.spec.game_id.clone()),
            ..Default::default()
        },
        EnvVar {
            name: "LIVEKIT_URL".to_string(),
            value: Some(livekit_url.to_string()),
            ..Default::default()
        },
        EnvVar {
            name: "LIVEKIT_API_KEY".to_string(),
            value_from: Some(EnvVarSource {
                secret_key_ref: Some(SecretKeySelector {
                    name: livekit_secret.to_string(),
                    key: "api_key".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        EnvVar {
            name: "LIVEKIT_API_SECRET".to_string(),
            value_from: Some(EnvVarSource {
                secret_key_ref: Some(SecretKeySelector {
                    name: livekit_secret.to_string(),
                    key: "api_secret".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
    ];
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
                image: Some(server_image.to_string()),
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
                            let combined = instance
                                .spec
                                .files
                                .as_ref()
                                .map(|files| files.join(","))
                                .unwrap_or_default();
                            if combined.contains(&instance.spec.iwad) {
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
            containers: vec![
                Container {
                    name: "server".to_string(),
                    image: Some(server_image.to_string()),
                    image_pull_policy: Some("Always".to_string()),
                    command: Some(vec!["/server.sh".to_string()]),
                    volume_mounts: Some(vec![VolumeMount {
                        name: "data".to_string(),
                        mount_path: DATA_ROOT.to_string(),
                        ..Default::default()
                    }]),
                    ports: Some(vec![ContainerPort {
                        container_port: game_port,
                        protocol: Some("UDP".to_string()),
                        ..Default::default()
                    }]),
                    env: Some(woof_env),
                    ..Default::default()
                },
                Container {
                    name: "proxy".to_string(),
                    image: Some(proxy_image.to_string()),
                    image_pull_policy: Some("Always".to_string()),
                    env: Some(proxy_env),
                    ..Default::default()
                },
            ],
            restart_policy: Some("Never".to_string()),
            ..Default::default()
        }),
        status: None,
    }
}

pub async fn create_pod(
    client: Client,
    instance: &Game,
    proxy_image: &str,
    server_image: &str,
    livekit_url: &str,
    livekit_secret: &str,
) -> Result<(), Error> {
    let pod = game_pod(
        instance,
        proxy_image,
        server_image,
        livekit_url,
        livekit_secret,
    );
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

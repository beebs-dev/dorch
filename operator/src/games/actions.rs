use crate::util::{Error, patch::*};
use dorch_types::*;
use k8s_openapi::{
    api::core::v1::{
        Container, ContainerPort, EnvVar, EnvVarSource, Pod, PodSpec, ResourceRequirements,
        SecretKeySelector, Volume, VolumeMount,
    },
    apimachinery::pkg::api::resource::Quantity,
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
pub async fn terminating(client: Client, instance: &Game, reason: String) -> Result<(), Error> {
    patch_status(client, instance, |status| {
        status.phase = GamePhase::Terminating;
        status.message = Some(reason);
    })
    .await?;
    Ok(())
}

pub async fn delete_pod(client: Client, instance: &Game, reason: String) -> Result<(), Error> {
    // pod has same name as resource
    let pod_name = instance.meta().name.as_ref().unwrap();
    println!(
        "Deleting Pod '{}' for Game '{}' • reason: {}",
        pod_name, pod_name, reason
    );
    patch_status(client.clone(), instance, |status| {
        status.phase = GamePhase::Pending;
        status.message = Some(reason);
    })
    .await?;
    let pods: Api<Pod> =
        Api::namespaced(client.clone(), instance.meta().namespace.as_ref().unwrap());
    pods.delete(pod_name, &Default::default()).await?;
    Ok(())
}

fn starting_message(pod_name: &str) -> String {
    format!("The game server Pod '{}' is starting.", pod_name)
}

pub async fn starting(client: Client, instance: &Game, reason: String) -> Result<(), Error> {
    patch_status(client, instance, |status| {
        status.phase = GamePhase::Starting;
        status.message = Some(reason);
    })
    .await?;
    Ok(())
}

pub async fn pending(client: Client, instance: &Game, reason: String) -> Result<(), Error> {
    patch_status(client, instance, |status| {
        status.phase = GamePhase::Pending;
        status.message = Some(reason);
    })
    .await?;
    Ok(())
}

fn game_pod(
    instance: &Game,
    proxy_image: &str,
    downloader_image: &str,
    server_image: &str,
    spectator_image: &str,
    livekit_url: &str,
    livekit_secret: &str,
    wadinfo_base_url: &str,
    strim_base_url: Option<&str>,
) -> Pod {
    let game_port = 10666;

    // For simplicity, we create a pod spec with a single container
    // that runs ffmpeg to stream from the source to the destination
    const DATA_ROOT: &str = "/var/wads";
    let mut server_env = vec![
        EnvVar {
            name: "GAME_ID".to_string(),
            value: Some(instance.spec.game_id.clone()),
            ..Default::default()
        },
        EnvVar {
            name: "MAX_PLAYERS".to_string(),
            value: Some(instance.spec.max_players.to_string()),
            ..Default::default()
        },
        EnvVar {
            name: "MASTER_BASE_URL".to_string(),
            value: Some("http://dorch-master".to_string()),
            ..Default::default()
        },
        EnvVar {
            name: "IWAD_ID".to_string(),
            value: Some(instance.spec.iwad.to_string()),
            ..Default::default()
        },
        EnvVar {
            name: "DATA_ROOT".to_string(),
            value: Some(DATA_ROOT.to_string()),
            ..Default::default()
        },
    ];
    let mut wad_list = Vec::new();
    if instance.spec.use_doom1_assets {
        // SHA1: 5b2e249b9c5133ec987b3ea77596381dc0d6bc1d
        // SHA256: 1d7d43be501e67d927e415e0b8f3e29c3bf33075e859721816f652a526cac771
        wad_list.push("22a0ca23-f044-4319-a7a6-f3b60276d0ce".to_string());
    }
    if let Some(files) = instance.spec.files.as_deref() {
        wad_list.extend(files.iter().cloned());
    }
    if !wad_list.is_empty() {
        server_env.push(EnvVar {
            name: "WAD_LIST".to_string(),
            value: Some(wad_list.join(",")),
            ..Default::default()
        });
    }
    if let Some(warp) = instance.spec.warp.as_deref() {
        server_env.push(EnvVar {
            name: "WARP".to_string(),
            value: Some(warp.to_string()),
            ..Default::default()
        });
    }
    if let Some(skill) = instance.spec.skill {
        server_env.push(EnvVar {
            name: "SKILL".to_string(),
            value: Some(skill.to_string()),
            ..Default::default()
        });
    }
    let mut proxy_env = vec![
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
    if instance.spec.debug_udp == Some(true) {
        proxy_env.push(EnvVar {
            name: "DORCH_UDP_DEBUG".to_string(),
            value: Some("1".to_string()),
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
                image: Some(downloader_image.to_string()),
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
                        name: "WADINFO_BASE_URL".to_string(),
                        value: Some(wadinfo_base_url.to_string()),
                        ..Default::default()
                    },
                    EnvVar {
                        name: "DOWNLOAD_LIST".to_string(),
                        value: Some({
                            let mut downloads: Vec<String> = Vec::new();
                            downloads.push(instance.spec.iwad.to_string());
                            downloads.extend(wad_list.iter().cloned());
                            downloads.join(",")
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
                    env: Some(server_env.clone()),
                    ..Default::default()
                },
                Container {
                    name: "spectator".to_string(),
                    image: Some(spectator_image.to_string()),
                    image_pull_policy: Some("Always".to_string()),
                    command: Some(vec!["/spectator.sh".to_string()]),
                    volume_mounts: Some(vec![VolumeMount {
                        name: "data".to_string(),
                        mount_path: DATA_ROOT.to_string(),
                        ..Default::default()
                    }]),
                    env: Some({
                        let mut env = Vec::with_capacity(server_env.len() + 2);
                        env.extend(server_env.into_iter());
                        env.push(EnvVar {
                            name: "SERVER_ADDR".to_string(),
                            value: Some(format!("localhost:{}", game_port)),
                            ..Default::default()
                        });
                        if let Some(strim_base_url) = strim_base_url {
                            env.push(EnvVar {
                                name: "RTMP_ENDPOINT".to_string(),
                                value: Some(format!(
                                    "{}/live/{}/{}",
                                    strim_base_url,
                                    instance.spec.game_id,
                                    instance.spec.game_id, // TODO: stream secret can be anything?
                                )),
                                ..Default::default()
                            });
                        }
                        env
                    }),
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
            resources: Some(ResourceRequirements {
                requests: Some({
                    let mut m = std::collections::BTreeMap::new();
                    m.insert("cpu".to_string(), Quantity("1000m".to_string()));
                    m.insert("memory".to_string(), Quantity("256Mi".to_string()));
                    m
                }),
                limits: Some({
                    let mut m = std::collections::BTreeMap::new();
                    //m.insert("cpu".to_string(), Quantity("2000m".to_string()));
                    m.insert("memory".to_string(), Quantity("512Mi".to_string()));
                    m
                }),
                ..Default::default()
            }),
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
    downloader_image: &str,
    server_image: &str,
    spectator_image: &str,
    livekit_url: &str,
    livekit_secret: &str,
    wadinfo_base_url: &str,
    strim_base_url: Option<&str>,
) -> Result<(), Error> {
    let pod = game_pod(
        instance,
        proxy_image,
        downloader_image,
        server_image,
        spectator_image,
        livekit_url,
        livekit_secret,
        wadinfo_base_url,
        strim_base_url,
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

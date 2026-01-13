use crate::{
    app::App,
    client::{GameSummary, ListGamesResponse, NewGameRequest, NewGameResponse, UpdateGameRequest},
};
use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use dorch_common::{
    access_log, response,
    types::{GameInfo, GameInfoUpdate},
};
use dorch_types::Game;
use kube::{Api, api::ObjectMeta};
use owo_colors::OwoColorize;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub async fn run_server(
    cancel: CancellationToken,
    args: crate::args::ServerArgs,
    app_state: App,
) -> Result<()> {
    let health_router = Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(health));
    let router = Router::new()
        .route("/game", get(list_games).post(new_game))
        .route("/game/{game_id}", get(get_game))
        .route("/game/{game_id}/info", post(update_game_info))
        .with_state(app_state)
        .layer(middleware::from_fn(access_log::internal));
    let port = args.internal_port;
    let addr: SocketAddr = format!("0.0.0.0:{}", port)
        .parse()
        .expect("Invalid address");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!(
        "{}{}",
        "🚀 Starting internal server • port=".green(),
        format!("{}", port).green().dimmed()
    );
    let start = std::time::Instant::now();
    axum::serve(listener, router.merge(health_router))
        .with_graceful_shutdown(async move {
            cancel.cancelled().await;
        })
        .await
        .context("Failed to serve internal router")?;
    println!(
        "{} {} {} {} {}",
        "🛑 Internal server on port".red(),
        format!("{}", port).red().dimmed(),
        "shut down gracefully".red(),
        "• uptime was".red(),
        humantime::format_duration(start.elapsed()).red().dimmed()
    );
    Ok(())
}

async fn health() -> impl IntoResponse {
    StatusCode::OK.into_response()
}

pub async fn update_game_info(
    State(state): State<App>,
    Path(game_id): Path<Uuid>,
    Json(info): Json<GameInfoUpdate>,
) -> impl IntoResponse {
    let mut args = Vec::new();
    if let Some(name) = info.name {
        args.push(("name", name));
    }
    if let Some(current_map) = info.current_map {
        args.push(("current_map", current_map));
    }
    if let Some(max_players) = info.max_players {
        args.push(("max_players", max_players.to_string()));
    }
    if let Some(player_count) = info.player_count {
        args.push(("player_count", player_count.to_string()));
    }
    if let Some(skill) = info.skill {
        args.push(("skill", skill.to_string()));
    }
    if let Some(monster_kill_count) = info.monster_kill_count {
        args.push(("monster_kill_count", monster_kill_count.to_string()));
    }
    if let Some(monster_count) = info.monster_count {
        args.push(("monster_count", monster_count.to_string()));
    }
    if args.is_empty() {
        return response::bad_request(anyhow!("No fields to update"));
    }
    if let Err(e) = state.store.update_game_info(game_id, &args).await {
        return response::error(anyhow!("Failed to update game info: {:?}", e));
    }
    println!(
        "{} {}",
        "✅ Updated game info for game ID".green(),
        format!("{}", game_id).green().dimmed()
    );
    StatusCode::OK.into_response()
}

pub async fn new_game(
    State(state): State<App>,
    Json(req): Json<NewGameRequest>,
) -> impl IntoResponse {
    let game_id = Uuid::new_v4();
    let game_id_str = game_id.to_string();
    let game = dorch_types::Game {
        metadata: ObjectMeta {
            name: Some(game_id_str.clone()),
            namespace: Some(state.namespace.clone()),
            annotations: Some(std::collections::BTreeMap::from([
                (
                    "dorch.io/created-by".to_string(),
                    "dorch-master".to_string(),
                ),
                (
                    "dorch.io/creator-user-id".to_string(),
                    req.creator_id.to_string(),
                ),
            ])),
            ..Default::default()
        },
        spec: dorch_types::GameSpec {
            game_id: game_id_str,
            files: req.files,
            private: req.private,
            skill: req.skill,
            warp: req.warp,
            max_players: 64,
            iwad: req.iwad,
            ..Default::default()
        },
        ..Default::default()
    };
    if let Err(e) = Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
        .create(&Default::default(), &game)
        .await
    {
        return response::error(anyhow!("Failed to create game: {:?}", e));
    }
    println!(
        "{} {}",
        "✅ Created new game with ID".green(),
        format!("{}", game_id).green().dimmed()
    );
    (StatusCode::OK, Json(NewGameResponse { game_id })).into_response()
}

pub async fn try_get_info(state: &App, game: &Game) -> Option<GameInfo> {
    let game_id = match Uuid::parse_str(game.spec.game_id.as_str()) {
        Ok(id) => id,
        Err(_) => return None,
    };
    state.store.get_game_info(game_id).await.ok().flatten()
}

pub async fn list_games_inner(state: App) -> Result<ListGamesResponse> {
    let games = Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
        .list(&Default::default())
        .await
        .context("Failed to list games")?;
    let mut infos = Vec::with_capacity(games.items.len());
    for game in games.items {
        let info = try_get_info(&state, &game).await;
        match game_to_summary(game, info) {
            Ok(summary) => infos.push(summary),
            Err(_) => continue,
        }
    }
    Ok(ListGamesResponse { games: infos })
}

pub async fn list_games(State(state): State<App>) -> impl IntoResponse {
    match list_games_inner(state).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => response::error(e.context("Failed to list games")),
    }
}

fn game_to_summary(g: dorch_types::Game, info: Option<GameInfo>) -> Result<GameSummary> {
    Ok(GameSummary {
        game_id: Uuid::parse_str(&g.spec.game_id).context("Invalid game ID")?,
        iwad: g.spec.iwad,
        files: g.spec.files,
        info,
    })
}

pub async fn get_game(State(state): State<App>, Path(game_id): Path<Uuid>) -> impl IntoResponse {
    let game = match Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
        .get(&game_id.to_string())
        .await
    {
        Ok(game) => game,
        Err(e) => {
            return match e {
                kube::Error::Api(ae) if ae.code == 404 => {
                    response::not_found(anyhow!("Game not found"))
                }
                _ => response::error(anyhow!("Failed to get game: {:?}", e)),
            };
        }
    };
    let info = try_get_info(&state, &game).await;
    match game_to_summary(game, info) {
        Ok(summary) => (StatusCode::OK, Json(summary)).into_response(),
        Err(e) => response::error(anyhow!("Failed to parse game: {:?}", e)),
    }
}

pub async fn update_game(
    State(_state): State<App>,
    Path(_game_id): Path<Uuid>,
    Json(_req): Json<UpdateGameRequest>,
) -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED.into_response()
    // let mut patch = serde_json::json!({
    //     "spec": {}
    // });
    // if let Some(name) = req.name {
    //     patch["spec"]["name"] = serde_json::json!(name);
    // } else {
    //     return response::error(anyhow!("No fields to update"));
    // }
    // match Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
    //     .patch(
    //         &game_id.to_string(),
    //         &kube::api::PatchParams::apply("dorch-master"),
    //         &kube::api::Patch::Merge(&patch),
    //     )
    //     .await
    // {
    //     Ok(_) => StatusCode::OK.into_response(),
    //     Err(e) => match e {
    //         kube::Error::Api(ae) if ae.code == 404 => {
    //             response::not_found(anyhow!("Game not found"))
    //         }
    //         _ => response::error(anyhow!("Failed to update game: {:?}", e)),
    //     },
    // }
}

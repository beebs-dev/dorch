use crate::{
    app::App,
    client::{GameSummary, ListGamesResponse, NewGameRequest, NewGameResponse},
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

pub const CREATOR_USER_ID_ANNOTATION: &str = "dorch.io/creator-user-id";

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
        .route("/game/{game_id}", get(get_game).delete(delete_game))
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
    if let Some(private) = info.private {
        args.push(("private", (private as u8).to_string()));
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
    if let Some(motd) = info.motd {
        args.push(("motd", motd));
    }
    if let Some(v) = info.sv_cheats {
        args.push(("sv_cheats", (v as u8).to_string()));
    }
    if let Some(v) = info.sv_allowchat {
        args.push(("sv_allowchat", (v as u8).to_string()));
    }
    if let Some(v) = info.sv_allowvoicechat {
        args.push(("sv_allowvoicechat", (v as u8).to_string()));
    }
    if let Some(v) = info.sv_fastmonsters {
        args.push(("sv_fastmonsters", (v as u8).to_string()));
    }
    if let Some(v) = info.sv_monsters {
        args.push(("sv_monsters", (v as u8).to_string()));
    }
    if let Some(v) = info.sv_nomonsters {
        args.push(("sv_nomonsters", (v as u8).to_string()));
    }
    if let Some(v) = info.sv_itemsrespawn {
        args.push(("sv_itemsrespawn", (v as u8).to_string()));
    }
    if let Some(v) = info.sv_itemrespawntime {
        args.push(("sv_itemrespawntime", v.to_string()));
    }
    if let Some(v) = info.sv_coop_damagefactor {
        args.push(("sv_coop_damagefactor", v.to_string()));
    }
    if let Some(v) = info.sv_nojump {
        args.push(("sv_nojump", (v as u8).to_string()));
    }
    if let Some(v) = info.sv_nocrouch {
        args.push(("sv_nocrouch", (v as u8).to_string()));
    }
    if let Some(v) = info.sv_nofreelook {
        args.push(("sv_nofreelook", (v as u8).to_string()));
    }
    if let Some(v) = info.sv_respawnonexit {
        args.push(("sv_respawnonexit", (v as u8).to_string()));
    }
    if let Some(v) = info.sv_timelimit {
        args.push(("sv_timelimit", v.to_string()));
    }
    if let Some(v) = info.sv_fraglimit {
        args.push(("sv_fraglimit", v.to_string()));
    }
    if let Some(v) = info.sv_scorelimit {
        args.push(("sv_scorelimit", v.to_string()));
    }
    if let Some(v) = info.sv_duellimit {
        args.push(("sv_duellimit", v.to_string()));
    }
    if let Some(v) = info.sv_roundlimit {
        args.push(("sv_roundlimit", v.to_string()));
    }
    if let Some(v) = info.sv_allowrun {
        args.push(("sv_allowrun", (v as u8).to_string()));
    }
    if let Some(v) = info.sv_allowfreelook {
        args.push(("sv_allowfreelook", (v as u8).to_string()));
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
                    CREATOR_USER_ID_ANNOTATION.to_string(),
                    req.creator_id.to_string(),
                ),
            ])),
            ..Default::default()
        },
        spec: dorch_types::GameSpec {
            game_id: game_id_str,
            files: req.files,
            private: Some(req.private),
            skill: req.skill,
            warp: req.warp.clone(),
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
    let game_id = match Uuid::parse_str(&game.spec.game_id) {
        Ok(id) => id,
        Err(_) => return None,
    };
    state.store.get_game_info(game_id).await.ok().flatten()
}

pub async fn list_games_inner(state: App) -> Result<ListGamesResponse> {
    let list = Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
        .list(&Default::default())
        .await
        .context("Failed to list games")?;
    let mut games = Vec::with_capacity(list.items.len());
    for game in list.items {
        let info = try_get_info(&state, &game).await;
        if info.as_ref().is_some_and(|info| info.private)
            || (info.is_none() && game.spec.private.unwrap_or(false))
        {
            // Omit private servers from the public listing.
            continue;
        }
        match game_to_summary(game, info) {
            Ok(summary) => games.push(summary),
            Err(_) => continue,
        }
    }
    Ok(ListGamesResponse { games })
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

pub async fn delete_game(State(state): State<App>, Path(game_id): Path<Uuid>) -> impl IntoResponse {
    if let Err(e) = Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
        .delete(&game_id.to_string(), &Default::default())
        .await
        .context("Failed to delete game")
    {
        if e.is::<kube::Error>() {
            let ke = e.downcast_ref::<kube::Error>().unwrap();
            if let kube::Error::Api(ae) = ke {
                if ae.code == 404 {
                    return response::not_found(anyhow!("Game not found"));
                }
            }
        }
        response::error(anyhow!("Failed to delete game: {:?}", e))
    } else {
        StatusCode::OK.into_response()
    }
    //let game = match Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
    //    .get(&game_id.to_string())
    //    .await
    //{
    //    Ok(game) => game,
    //    Err(e) => {
    //        return match e {
    //            kube::Error::Api(ae) if ae.code == 404 => {
    //                response::not_found(anyhow!("Game not found"))
    //            }
    //            _ => response::error(anyhow!("Failed to get game: {:?}", e)),
    //        };
    //    }
    //};
    //game
    //    .metadata
    //    .annotations
    //    .as_ref()
    //    .and_then(|m| m.get(CREATOR_USER_ID_ANNOTATION))
    //    .map(|s| s.parse::<Uuid>())
    //    .transpose().unwrap()
    //    .and_then(|s| s != user_id)
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

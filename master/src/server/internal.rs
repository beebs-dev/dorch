use crate::{
    app::App,
    client::{
        GameSummary, JumbotronItem, ListGamesResponse, ListJumbotronStreams, NewGameRequest,
        NewGameResponse,
    },
};
use anyhow::{Context, Result, anyhow, bail};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, Response, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use bytes::Bytes;
use dorch_common::{
    access_log, annotations,
    rate_limit::{RateLimiter, middleware::RateLimitLayer},
    response,
    types::{GameInfo, GameInfoUpdate, Settable},
};
use dorch_types::{Game, GamePhase, GameSpec};
use kube::{
    Api, ResourceExt,
    api::{ObjectMeta, Patch, PatchParams},
};
use owo_colors::OwoColorize;
use std::{collections::BTreeMap, net::SocketAddr, time::Instant};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub async fn run_server(
    cancel: CancellationToken,
    port: u16,
    app_state: App,
    rate_limiter: RateLimiter,
) -> Result<()> {
    let health_router = Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(health));
    let router = Router::new()
        .route("/jumbotron", get(list_jumbotron_mc3u8_urls))
        .route("/game", get(list_games))
        .route(
            "/game/{game_id}",
            get(get_game).delete(delete_game).post(new_game),
        )
        .route("/game/{game_id}/info", post(update_game_info))
        .route(
            "/game/{game_id}/liveshot",
            get(get_live_shot).post(post_live_shot),
        )
        .with_state(app_state)
        .layer(RateLimitLayer::new(rate_limiter))
        .layer(middleware::from_fn(access_log::internal));
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

pub async fn get_live_shot(
    State(state): State<App>,
    Path(game_id): Path<Uuid>,
) -> Response<axum::body::Body> {
    match state.store.get_live_shot(game_id).await {
        Ok(Some((data, content_type))) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", content_type)
            .body(Bytes::from(data).into())
            .unwrap(),
        Ok(None) => response::not_found(anyhow!("Game live shot not found")).into_response(),
        Err(e) => response::error(e.context("Failed to get game live shot")).into_response(),
    }
}

pub async fn post_live_shot(
    State(state): State<App>,
    Path(game_id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let content_type = match headers.get("Content-Type") {
        Some(ct) => match ct.to_str() {
            Ok(s) => s,
            Err(_) => return response::bad_request(anyhow!("Invalid Content-Type header")),
        },
        None => return response::bad_request(anyhow!("Missing Content-Type header")),
    };
    match state
        .store
        .set_live_shot(game_id, &body, content_type)
        .await
    {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => response::error(e.context("Failed to set game live shot")),
    }
}

pub async fn update_game_info(
    State(state): State<App>,
    Path(game_id): Path<Uuid>,
    Json(info): Json<GameInfoUpdate>,
) -> impl IntoResponse {
    fn push_string(
        set_args: &mut Vec<(&'static str, String)>,
        del_args: &mut Vec<&'static str>,
        key: &'static str,
        v: Option<Settable<String>>,
    ) {
        match v {
            None => {}
            Some(Settable::Set(value)) => set_args.push((key, value)),
            Some(Settable::Unset) => del_args.push(key),
        }
    }

    fn push_to_string<T: ToString>(
        set_args: &mut Vec<(&'static str, String)>,
        del_args: &mut Vec<&'static str>,
        key: &'static str,
        v: Option<Settable<T>>,
    ) {
        match v {
            None => {}
            Some(Settable::Set(value)) => set_args.push((key, value.to_string())),
            Some(Settable::Unset) => del_args.push(key),
        }
    }

    fn push_bool(
        set_args: &mut Vec<(&'static str, String)>,
        del_args: &mut Vec<&'static str>,
        key: &'static str,
        v: Option<Settable<bool>>,
    ) {
        match v {
            None => {}
            Some(Settable::Set(value)) => set_args.push((key, (value as u8).to_string())),
            Some(Settable::Unset) => del_args.push(key),
        }
    }

    let mut set_args: Vec<(&'static str, String)> = Vec::new();
    let mut del_args: Vec<&'static str> = Vec::new();

    push_string(&mut set_args, &mut del_args, "name", info.name);
    push_bool(&mut set_args, &mut del_args, "private", info.private);

    if let Some(zandronum) = info.zandronum {
        push_string(
            &mut set_args,
            &mut del_args,
            "map_title",
            zandronum.map_title,
        );
        push_string(
            &mut set_args,
            &mut del_args,
            "current_map",
            zandronum.current_map,
        );
        push_to_string(
            &mut set_args,
            &mut del_args,
            "server_started_at",
            zandronum.server_started_at,
        );
        push_to_string(
            &mut set_args,
            &mut del_args,
            "map_started_at",
            zandronum.map_started_at,
        );
        push_to_string(
            &mut set_args,
            &mut del_args,
            "max_players",
            zandronum.max_players,
        );
        push_to_string(
            &mut set_args,
            &mut del_args,
            "player_count",
            zandronum.player_count,
        );
        push_to_string(&mut set_args, &mut del_args, "skill", zandronum.skill);
        push_to_string(
            &mut set_args,
            &mut del_args,
            "monster_kill_count",
            zandronum.monster_kill_count,
        );
        push_to_string(
            &mut set_args,
            &mut del_args,
            "monster_count",
            zandronum.monster_count,
        );
        push_string(&mut set_args, &mut del_args, "motd", zandronum.motd);

        push_bool(
            &mut set_args,
            &mut del_args,
            "sv_cheats",
            zandronum.sv_cheats,
        );
        push_bool(
            &mut set_args,
            &mut del_args,
            "sv_allowchat",
            zandronum.sv_allowchat,
        );
        push_bool(
            &mut set_args,
            &mut del_args,
            "sv_allowvoicechat",
            zandronum.sv_allowvoicechat,
        );
        push_bool(
            &mut set_args,
            &mut del_args,
            "sv_fastmonsters",
            zandronum.sv_fastmonsters,
        );
        push_bool(
            &mut set_args,
            &mut del_args,
            "sv_monsters",
            zandronum.sv_monsters,
        );
        push_bool(
            &mut set_args,
            &mut del_args,
            "sv_nomonsters",
            zandronum.sv_nomonsters,
        );
        push_bool(
            &mut set_args,
            &mut del_args,
            "sv_itemsrespawn",
            zandronum.sv_itemsrespawn,
        );
        push_to_string(
            &mut set_args,
            &mut del_args,
            "sv_itemrespawntime",
            zandronum.sv_itemrespawntime,
        );
        push_to_string(
            &mut set_args,
            &mut del_args,
            "sv_coop_damagefactor",
            zandronum.sv_coop_damagefactor,
        );
        push_bool(
            &mut set_args,
            &mut del_args,
            "sv_nojump",
            zandronum.sv_nojump,
        );
        push_bool(
            &mut set_args,
            &mut del_args,
            "sv_nocrouch",
            zandronum.sv_nocrouch,
        );
        push_bool(
            &mut set_args,
            &mut del_args,
            "sv_nofreelook",
            zandronum.sv_nofreelook,
        );
        push_bool(
            &mut set_args,
            &mut del_args,
            "sv_respawnonexit",
            zandronum.sv_respawnonexit,
        );
        push_to_string(
            &mut set_args,
            &mut del_args,
            "sv_timelimit",
            zandronum.sv_timelimit,
        );
        push_to_string(
            &mut set_args,
            &mut del_args,
            "sv_fraglimit",
            zandronum.sv_fraglimit,
        );
        push_to_string(
            &mut set_args,
            &mut del_args,
            "sv_scorelimit",
            zandronum.sv_scorelimit,
        );
        push_to_string(
            &mut set_args,
            &mut del_args,
            "sv_duellimit",
            zandronum.sv_duellimit,
        );
        push_to_string(
            &mut set_args,
            &mut del_args,
            "sv_roundlimit",
            zandronum.sv_roundlimit,
        );
        push_bool(
            &mut set_args,
            &mut del_args,
            "sv_allowrun",
            zandronum.sv_allowrun,
        );
        push_bool(
            &mut set_args,
            &mut del_args,
            "sv_allowfreelook",
            zandronum.sv_allowfreelook,
        );
    }

    if set_args.is_empty() && del_args.is_empty() {
        return response::bad_request(anyhow!("No fields to update"));
    }
    if let Err(e) = state
        .store
        .update_game_info(game_id, &set_args, &del_args)
        .await
    {
        return response::error(e.context("Failed to update game info"));
    }
    println!(
        "{}{}{}{}{}{}",
        "✅ Updated game info • game_id=".green(),
        game_id.green().dimmed(),
        " • set_fields=".green(),
        set_args.len().to_string().green().dimmed(),
        " • del_fields=".green(),
        del_args.len().to_string().green().dimmed()
    );
    StatusCode::OK.into_response()
}

pub async fn list_jumbotron_mc3u8_urls(State(state): State<App>) -> impl IntoResponse {
    let mut games = match list_games_inner(state).await {
        Ok(resp) => resp.games,
        Err(e) => {
            return response::error(e.context("Failed to list games for jumbotron RTMP URLs"));
        }
    };
    let mut rng = rand::rng();
    let games = if games.len() <= 5 {
        games
    } else {
        use rand::seq::SliceRandom;
        games.shuffle(&mut rng);
        games.into_iter().take(5).collect()
    };
    let items = match games
        .into_iter()
        .map(|g| {
            let info = g
                .info
                .ok_or_else(|| anyhow!("unreachable branch: GameSummary has None info"))?;
            Ok(JumbotronItem {
                game_id: g.game_id,
                name: info.name,
                player_count: info.player_count,
                max_players: info.max_players,
                monster_kill_count: info.monster_kill_count,
                monster_total: info.monster_count,
                hls: format!("https://cdn.gib.gg/live/{}.m3u8", g.game_id),
                rtc: format!("webrtc://cdn.gib.gg/live/{}", g.game_id),
            })
        })
        .collect::<Result<Vec<JumbotronItem>>>()
    {
        Ok(v) => v,
        Err(e) => {
            return response::error(e.context("Failed to construct jumbotron items"));
        }
    };
    let resp = ListJumbotronStreams { items };
    (StatusCode::OK, Json(resp)).into_response()
}

async fn count_games(state: &App, creator_id: Uuid) -> Result<(usize, usize)> {
    let list = Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
        .list(&Default::default())
        .await
        .context("Failed to list games for counting")?;
    let count = list
        .items
        .iter()
        .filter(|game| {
            game.status
                .as_ref()
                .map(|p| p.phase == GamePhase::Active)
                .unwrap_or(false)
        })
        .count();
    let user_count = list
        .items
        .iter()
        .filter(|game| {
            game.status
                .as_ref()
                .map(|p| p.phase == GamePhase::Active)
                .unwrap_or(false)
                && game
                    .metadata
                    .annotations
                    .as_ref()
                    .and_then(|anns| anns.get(annotations::CREATED_BY_USER))
                    .map(|s| s.parse::<Uuid>().ok())
                    .flatten()
                    .unwrap_or_else(Uuid::nil)
                    == creator_id
        })
        .count();
    Ok((count, user_count))
}

fn game_resource_name(game_id: Uuid) -> String {
    format!("game-{}", game_id)
}

fn game_resource(game_id: Uuid, req: NewGameRequest, namespace: String) -> Game {
    let game_id_str = game_id.to_string();
    let game = Game {
        metadata: ObjectMeta {
            name: Some(game_resource_name(game_id)),
            namespace: Some(namespace),
            annotations: Some(BTreeMap::from([
                (
                    annotations::CREATED_BY.to_string(),
                    "dorch-master".to_string(),
                ),
                (
                    annotations::CREATED_BY_USER.to_string(),
                    req.creator_id.to_string(),
                ),
                (annotations::STABLE_ID.to_string(), game_id_str.clone()),
            ])),
            ..Default::default()
        },
        spec: GameSpec {
            name: req.name,
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
    game
}

pub async fn ensure_capacity(state: &App, creator_id: Uuid) -> Option<impl IntoResponse> {
    match count_games(&state, creator_id).await {
        Ok((count, user_count)) => {
            if let Some(max) = state.max_servers
                && count >= max
            {
                return Some(response::too_many_requests(anyhow!(
                    "Maximum number of active game servers reached ({})",
                    max
                )));
            }
            if let Some(max) = state.max_servers_per_user
                && user_count >= max
            {
                return Some(response::too_many_requests(anyhow!(
                    "User is already at the maximum number of active game servers ({})",
                    max
                )));
            }
            None
        }
        Err(e) => {
            return Some(response::error(e.context("Failed to count active games")));
        }
    }
}

pub async fn new_game(
    State(state): State<App>,
    Path(game_id): Path<Uuid>,
    Json(req): Json<NewGameRequest>,
) -> impl IntoResponse {
    if let Some(resp) = ensure_capacity(&state, req.creator_id).await {
        return resp.into_response();
    }
    let exists = match Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
        .get(&game_resource_name(game_id))
        .await
    {
        Ok(game) => {
            if let Some(creator_id) = game.annotations().get(annotations::CREATED_BY_USER)
                && creator_id != &req.creator_id.to_string()
            {
                return response::forbidden(anyhow!(
                    "Game with ID {} already exists and was created by a different user",
                    game_id
                ));
            }
            true
        }
        Err(kube::Error::Api(ae)) if ae.code == 404 => false,
        Err(e) => {
            return response::error(anyhow!(
                "Failed to check existing game {}: {:?}",
                game_id,
                e
            ));
        }
    };
    let game = game_resource(game_id, req, state.namespace.clone());
    let game = match Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
        .patch(
            game.metadata.name.as_deref().unwrap(),
            &PatchParams::apply("dorch-master"),
            &Patch::Apply(&game),
        )
        .await
    {
        Ok(g) => g,
        Err(e) => {
            return response::error(anyhow!(
                "Failed to create or update game {}: {:?}",
                game_id,
                e
            ));
        }
    };
    if exists {
        println!(
            "{}{}{}{}",
            "✅ Updated game • game_id=".green(),
            game_id.green().dimmed(),
            " • name=".green(),
            game.spec.name.green().dimmed()
        );
    } else {
        println!(
            "{}{}{}{}",
            "✅ Created new game • game_id=".green(),
            game_id.green().dimmed(),
            " • name=".green(),
            game.spec.name.green().dimmed()
        );
    }
    (StatusCode::OK, Json(NewGameResponse { game_id })).into_response()
}

pub async fn try_get_info(state: &App, game: &Game) -> Option<GameInfo> {
    let game_id = match Uuid::parse_str(&game.spec.game_id) {
        Ok(id) => id,
        Err(_) => {
            eprint!(
                "{}{}",
                "⚠️  Invalid game ID: ".yellow(),
                game.spec.game_id.yellow().dimmed()
            );
            return None;
        }
    };
    match state.store.get_game_info(game_id).await {
        Ok(Some(mut info)) => {
            // Integrate higher-order fields from the Game spec.
            info.name = game.spec.name.clone();
            info.private = game.spec.private.unwrap_or(false);
            println!(
                "{}{}{}{}",
                "✅ Retrieved game info • game_id=".green(),
                game_id.green().dimmed(),
                " • name=".green(),
                info.name.green().dimmed(),
            );
            Some(info)
        }
        Ok(None) => {
            eprintln!(
                "{}{}",
                "⚠️  No game info found • game_id=".yellow(),
                game_id.yellow().dimmed(),
            );
            None
        }
        Err(e) => {
            eprintln!(
                "{}{}{}{}",
                "⚠️  Failed to get game info • game_id=".yellow(),
                game_id.yellow().dimmed(),
                " • error=".yellow(),
                format!("{:?}", e).yellow().dimmed()
            );
            None
        }
    }
}

pub async fn list_games_inner(state: App) -> Result<ListGamesResponse> {
    let start = std::time::Instant::now();
    let list = Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
        .list(&Default::default())
        .await
        .context("Failed to list games")?;
    let mut games = Vec::with_capacity(list.items.len());
    for game in list.items {
        if game
            .status
            .as_ref()
            .map(|p| p.phase != GamePhase::Active)
            .unwrap_or(true)
        {
            // Only list active games.
            continue;
        }
        let Some(info) = try_get_info(&state, &game).await else {
            eprintln!(
                "{}{}",
                "⚠️  Skipping game with invalid or missing info • game_id=".yellow(),
                game.spec.game_id.yellow().dimmed(),
            );
            continue;
        };
        if info.private {
            // Omit private servers from the public listing.
            eprintln!(
                "{}{}",
                "⚠️  Skipping private game • game_id=".yellow(),
                game.spec.game_id.yellow().dimmed(),
            );
            continue;
        }
        let summary = match game_to_summary(game, Some(info)) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "{}{}",
                    "⚠️  Skipping game with invalid summary • err=".yellow(),
                    format!("{:?}", e).yellow().dimmed()
                );
                continue;
            }
        };
        games.push(summary);
    }
    eprintln!(
        "{}{}{}{}{}",
        "✅  Listed games • count=".green(),
        games.len().green().dimmed(),
        " • elapsed=".green(),
        Instant::now()
            .duration_since(start)
            .as_millis()
            .to_string()
            .green()
            .dimmed(),
        " ms".green(),
    );
    Ok(ListGamesResponse { games })
}

pub async fn list_games(State(state): State<App>) -> impl IntoResponse {
    match list_games_inner(state).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => response::error(e.context("Failed to list games")),
    }
}

fn game_to_summary(g: dorch_types::Game, info: Option<GameInfo>) -> Result<GameSummary> {
    let status = g
        .status
        .as_ref()
        .map(|s| s.phase.to_string())
        .unwrap_or_else(|| dorch_types::GamePhase::Pending.to_string());
    Ok(GameSummary {
        game_id: Uuid::parse_str(&g.spec.game_id).context("Invalid game ID")?,
        status,
        iwad: Uuid::parse_str(&g.spec.iwad).context("Invalid IWAD ID")?,
        files: g
            .spec
            .files
            .map(|files| {
                files
                    .iter()
                    .map(|s| Uuid::parse_str(s).context("Invalid file ID"))
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?,
        creator_id: g
            .metadata
            .annotations
            .as_ref()
            .and_then(|anns| anns.get(annotations::CREATED_BY_USER))
            .map(|s| s.parse::<Uuid>())
            .transpose()
            .context("Invalid creator user ID")?
            .unwrap_or_else(Uuid::nil),
        spec: crate::client::GameSpecSummary {
            name: g.spec.name.clone(),
            max_players: g.spec.max_players,
            skill: g.spec.skill,
            warp: g.spec.warp.clone(),
            private: g.spec.private.unwrap_or(false),
        },
        info,
    })
}

pub async fn delete_game(State(state): State<App>, Path(game_id): Path<Uuid>) -> impl IntoResponse {
    if let Err(e) = Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
        .delete(game_id.to_string().as_str(), &Default::default())
        .await
    {
        return match e {
            kube::Error::Api(ae) if ae.code == 404 => {
                response::not_found(anyhow!("Game {} not found", game_id))
            }
            e => response::error(anyhow!("Failed to delete game {}: {:?}", game_id, e)),
        };
    }
    if let Err(e) = state.store.delete_game_info(game_id).await {
        eprintln!(
            "{}{}{}{}",
            "⚠️  Failed to delete game info • game_id=".yellow(),
            game_id.yellow().dimmed(),
            " • error=".yellow(),
            format!(": {:?}", e).yellow().dimmed()
        );
    }
    StatusCode::OK.into_response()
}

pub async fn get_game_internal(state: App, game_id: Uuid) -> Result<Option<GameSummary>> {
    let game = match Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
        .get(&game_resource_name(game_id))
        .await
    {
        Ok(game) => game,
        Err(kube::Error::Api(ae)) if ae.code == 404 => {
            return Ok(None);
        }
        Err(e) => bail!("Failed to get game {}: {:?}", game_id, e),
    };
    let info = try_get_info(&state, &game).await;
    match game_to_summary(game, info) {
        Ok(summary) => Ok(Some(summary)),
        Err(e) => Err(anyhow!("Failed to parse game {}: {:?}", game_id, e)),
    }
}

pub async fn get_game(State(state): State<App>, Path(game_id): Path<Uuid>) -> impl IntoResponse {
    match get_game_internal(state, game_id).await {
        Ok(Some(summary)) => (StatusCode::OK, Json(summary)).into_response(),
        Ok(None) => response::not_found(anyhow!("Game {} not found", game_id)),
        Err(e) => response::error(e),
    }
}

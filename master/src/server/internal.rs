use crate::{
    app::App,
    client::{GameSummary, ListGamesResponse, NewGameRequest, NewGameResponse},
};
use anyhow::{Context, Result, anyhow, bail};
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
    types::{GameInfo, GameInfoUpdate, Settable},
};
use dorch_types::Game;
use kube::{Api, api::ObjectMeta};
use owo_colors::OwoColorize;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub const CREATOR_USER_ID_ANNOTATION: &str = "dorch.io/creator-user-id";

pub async fn run_server(cancel: CancellationToken, port: u16, app_state: App) -> Result<()> {
    let health_router = Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(health));
    let router = Router::new()
        .route("/game", get(list_games).post(new_game))
        .route("/game/{game_id}", get(get_game).delete(delete_game))
        .route("/game/{game_id}/info", post(update_game_info))
        .with_state(app_state)
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
            "current_map",
            zandronum.current_map,
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
                " • json=".green(),
                serde_json::to_string(&info).unwrap().green().dimmed()
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
    let list = Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
        .list(&Default::default())
        .await
        .context("Failed to list games")?;
    let mut games = Vec::with_capacity(list.items.len());
    for game in list.items {
        let Some(info) = try_get_info(&state, &game).await else {
            eprintln!(
                "{}{}{}",
                "⚠️  Skipping game with invalid or missing info • game_id=".yellow(),
                game.spec.game_id.yellow().dimmed(),
                ""
            );
            continue;
        };
        if info.private {
            // Omit private servers from the public listing.
            eprintln!(
                "{}{}{}",
                "⚠️  Skipping private game • game_id=".yellow(),
                game.spec.game_id.yellow().dimmed(),
                ""
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
        "{}{}",
        "✅ Listed games • json=".green(),
        serde_json::to_string(&games).unwrap().green().dimmed(),
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
    Ok(GameSummary {
        game_id: Uuid::parse_str(&g.spec.game_id).context("Invalid game ID")?,
        iwad: g.spec.iwad,
        files: g.spec.files,
        creator_id: g
            .metadata
            .annotations
            .as_ref()
            .and_then(|anns| anns.get(CREATOR_USER_ID_ANNOTATION))
            .map(|s| s.parse::<Uuid>())
            .transpose()
            .context("Invalid creator user ID")?
            .unwrap_or_else(Uuid::nil),
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
                response::not_found(anyhow!("Game not found"))
            }
            e => response::error(anyhow!("Failed to delete game: {:?}", e)),
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
        .get(game_id.to_string().as_str())
        .await
    {
        Ok(game) => game,
        Err(kube::Error::Api(ae)) if ae.code == 404 => {
            return Ok(None);
        }
        Err(e) => bail!("Failed to get game: {:?}", e),
    };
    let info = try_get_info(&state, &game).await;
    match game_to_summary(game, info) {
        Ok(summary) => Ok(Some(summary)),
        Err(e) => Err(anyhow!("Failed to parse game: {:?}", e)),
    }
}

pub async fn get_game(State(state): State<App>, Path(game_id): Path<Uuid>) -> impl IntoResponse {
    match get_game_internal(state, game_id).await {
        Ok(Some(summary)) => (StatusCode::OK, Json(summary)).into_response(),
        Ok(None) => response::not_found(anyhow!("Game not found")),
        Err(e) => response::error(anyhow!("Failed to get game: {:?}", e)),
    }
}

use crate::{
    app::App,
    party_store::{AcceptInvite, Invite, Kick, LeaveParty, Party},
    server::internal,
};
use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{post, put},
};
use axum_keycloak_auth::{
    PassthroughMode,
    instance::{KeycloakAuthInstance, KeycloakConfig},
    layer::KeycloakAuthLayer,
};
use dorch_common::{access_log, cors, rbac::UserId, response};
use owo_colors::OwoColorize;
use reqwest::Url;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub async fn run_server(
    cancel: CancellationToken,
    args: crate::args::ServerArgs,
    app_state: App,
) -> Result<()> {
    let keycloak_auth_instance = KeycloakAuthInstance::new(
        KeycloakConfig::builder()
            .server(Url::parse(&args.kc.endpoint).unwrap())
            .realm(args.kc.realm)
            .build(),
    );
    let keycloak_layer = KeycloakAuthLayer::<String>::builder()
        .instance(keycloak_auth_instance)
        .passthrough_mode(PassthroughMode::Block)
        .persist_raw_claims(true)
        .expected_audiences(vec![args.kc.client_id])
        .build();
    let router = Router::new()
        .route("/party/{party_id}", put(put_party).get(get_party))
        .route("/party/{party_id}/invite", post(invite))
        .route("/party/{party_id}/accept_invite", post(accept_invite))
        .route("/party/{party_id}/kick", post(kick))
        .route("/party/{party_id}/leave", post(leave))
        .with_state(app_state)
        .layer(keycloak_layer)
        .layer(middleware::from_fn(access_log::public))
        .layer(cors::dev());
    let port = args.public_port;
    let addr: SocketAddr = format!("0.0.0.0:{}", port)
        .parse()
        .expect("Invalid address");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!(
        "{}{}",
        "🚀 Starting public server • port=".green(),
        format!("{}", port).green().dimmed()
    );
    let start = std::time::Instant::now();
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            cancel.cancelled().await;
        })
        .await
        .context("Failed to serve public router")?;
    println!(
        "{} {} {} {} {}",
        "🛑 Public server on port".red(),
        format!("{}", port).red().dimmed(),
        "shut down gracefully".red(),
        "• uptime was".red(),
        humantime::format_duration(start.elapsed()).red().dimmed()
    );
    Ok(())
}

pub async fn invite(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(party_id): Path<Uuid>,
    Json(mut invite): Json<Invite>,
) -> impl IntoResponse {
    invite.sender_id = user_id;
    internal::invite(State(state), Path(party_id), Json(invite))
        .await
        .into_response()
}

pub async fn accept_invite(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(party_id): Path<Uuid>,
    Json(req): Json<Option<AcceptInvite>>,
) -> impl IntoResponse {
    let mut req = req.unwrap_or_else(|| AcceptInvite { user_id });
    if req.user_id.is_nil() {
        req.user_id = user_id;
    } else if req.user_id != user_id {
        return response::bad_request(anyhow!("User ID does not match authenticated user"));
    }
    internal::accept_invite(State(state), Path(party_id), Json(req))
        .await
        .into_response()
}

pub async fn kick(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(party_id): Path<Uuid>,
    Json(mut req): Json<Kick>,
) -> impl IntoResponse {
    req.kicker_id = user_id;
    let party = match state.store.get_party(party_id).await {
        Ok(Some(party)) => party,
        Ok(None) => return response::not_found(anyhow!("Party not found")),
        Err(e) => return response::error(e.context("Failed to get party info")),
    };
    if party.leader_id != user_id {
        return response::forbidden(anyhow!("Only the party leader can kick members"));
    }
    internal::kick(State(state), Path(party_id), Json(req))
        .await
        .into_response()
}

pub async fn leave(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(party_id): Path<Uuid>,
) -> impl IntoResponse {
    match state.store.user_is_in_party(party_id, user_id).await {
        Ok(true) => internal::leave(State(state), Path(party_id), Json(LeaveParty { user_id }))
            .await
            .into_response(),
        Ok(false) => return response::not_found(anyhow!("User is not in party")),
        Err(e) => return response::error(e.context("Failed to check if user is in party")),
    }
}

pub async fn put_party(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(party_id): Path<Uuid>,
    Json(party): Json<Party>,
) -> impl IntoResponse {
    if party_id != party.id {
        return response::bad_request(anyhow!("Party ID in path and body do not match"));
    }
    match state.store.get_party(party_id).await {
        Ok(Some(existing)) if existing.leader_id != user_id => {
            response::forbidden(anyhow!("Only the party leader can update party info"))
        }
        Err(e) => response::error(e.context("Failed to check if party exists")),
        _ => internal::put_party(State(state), Path(party_id), Json(party))
            .await
            .into_response(),
    }
}

pub async fn get_party(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(party_id): Path<Uuid>,
) -> impl IntoResponse {
    match state.store.get_party(party_id).await {
        Ok(Some(party)) => {
            if party.members.as_ref().is_some_and(|m| m.contains(&user_id)) {
                (StatusCode::OK, Json(party)).into_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

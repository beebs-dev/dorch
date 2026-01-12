use crate::{
    app::App,
    party_store::{AcceptInvite, Invite, Kick, LeaveParty},
};
use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{get, post, put},
};
use dorch_common::{
    access_log, response,
    streams::{LeaveReason, WebsockMessageType, subjects},
    types::Party,
};
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
        .route("/party/{party_id}", put(put_party).get(get_party))
        .route("/party/{party_id}/invite", post(invite))
        .route("/party/{party_id}/accept_invite", post(accept_invite))
        .route("/party/{party_id}/kick", post(kick))
        .route("/party/{party_id}/leave", post(leave))
        .route("/party/{party_id}/member/{user_id}", get(is_member))
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

pub async fn invite(
    State(state): State<App>,
    Path(party_id): Path<Uuid>,
    Json(invite): Json<Invite>,
) -> impl IntoResponse {
    if let Err(e) = state
        .store
        .create_invite(party_id, invite.recipient_id, invite.sender_id)
        .await
    {
        return response::error(e.context("Failed to create invite"));
    }
    if let Err(e) = state
        .nats
        .publish(
            subjects::user(invite.recipient_id),
            WebsockMessageType::invite(party_id, invite.sender_id),
        )
        .await
    {
        return response::error(anyhow!("Failed to publish invite over NATS: {:?}", e));
    }
    StatusCode::OK.into_response()
}

pub async fn accept_invite(
    State(state): State<App>,
    Path(party_id): Path<Uuid>,
    Json(req): Json<AcceptInvite>,
) -> impl IntoResponse {
    if let Err(e) = state.store.accept_invite(party_id, req.user_id).await {
        return response::error(e.context("Failed to accept invite"));
    }
    if let Err(e) = state
        .nats
        .publish(
            subjects::party(party_id),
            WebsockMessageType::member_joined(party_id, req.user_id),
        )
        .await
    {
        return response::error(anyhow!("Failed to publish invite over NATS: {:?}", e));
    }
    StatusCode::OK.into_response()
}

pub async fn is_member(
    State(state): State<App>,
    Path((party_id, user_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    match state.store.user_is_in_party(party_id, user_id).await {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => response::error(e.context("Failed to check membership")),
    }
}

pub async fn leave(
    State(state): State<App>,
    Path(party_id): Path<Uuid>,
    Json(req): Json<LeaveParty>,
) -> impl IntoResponse {
    if let Err(e) = state.store.remove_member(party_id, req.user_id).await {
        return response::error(e.context("Failed to remove member from party"));
    }
    if let Err(e) = state
        .nats
        .publish(
            subjects::party(party_id),
            WebsockMessageType::member_left(party_id, req.user_id, LeaveReason::Left),
        )
        .await
    {
        return response::error(anyhow!("Failed to publish invite over NATS: {:?}", e));
    }
    StatusCode::OK.into_response()
}

pub async fn kick(
    State(state): State<App>,
    Path(party_id): Path<Uuid>,
    Json(req): Json<Kick>,
) -> impl IntoResponse {
    if let Err(e) = state.store.remove_member(party_id, req.user_id).await {
        return response::error(e.context("Failed to remove member from party"));
    }
    if let Err(e) = state
        .nats
        .publish(
            subjects::party(party_id),
            WebsockMessageType::member_left(party_id, req.user_id, LeaveReason::Kicked),
        )
        .await
    {
        return response::error(anyhow!("Failed to publish invite over NATS: {:?}", e));
    }
    StatusCode::OK.into_response()
}

pub async fn put_party(
    State(state): State<App>,
    Path(party_id): Path<Uuid>,
    Json(party): Json<Party>,
) -> impl IntoResponse {
    if party_id != party.id {
        return response::bad_request(anyhow!("Party ID in path and body do not match"));
    }
    if let Err(e) = state.store.update_info(&party).await {
        return response::error(e.context("Failed to update party info"));
    }
    let payload = WebsockMessageType::party_info(&party);
    if let Err(e) = state.nats.publish(subjects::party(party_id), payload).await {
        return response::error(anyhow!("Failed to publish party update: {:?}", e));
    }
    StatusCode::OK.into_response()
}

pub async fn get_party(State(state): State<App>, Path(party_id): Path<Uuid>) -> impl IntoResponse {
    match state.store.get_party(party_id).await {
        Ok(Some(party)) => (StatusCode::OK, Json(party)).into_response(),
        Ok(None) => response::not_found(anyhow!("Party not found")),
        Err(e) => response::error(e.context("Failed to get party info")),
    }
}

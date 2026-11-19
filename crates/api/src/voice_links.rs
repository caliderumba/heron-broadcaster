// SPDX-License-Identifier: AGPL-3.0-or-later

//! Upload and management of recorded voice links.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouterExt};
use utoipa_axum::routes;

use heron_core::library::TaskRequest;

use crate::app::AppState;
use crate::auth::{CurrentUser, Role};
use crate::error::{ApiError, ApiResult, ErrorBody};
use crate::librarian::{self, LibraryTask};
use crate::stations::Station;

/// Largest voice link upload: 16 MiB.
const MAX_VOICE_LINK_BYTES: usize = 16 * 1024 * 1024;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(upload_voice_link).layer(DefaultBodyLimit::max(MAX_VOICE_LINK_BYTES)))
}

/// Uploads an audio take of a recorded voice link (WebM, Ogg, WAV, etc.).
/// heron-librarian converts it with FFmpeg, tags it (title with date/time,
/// artist = operator), and files it into the station's "Voice track" category.
/// Operators and admins.
#[utoipa::path(post, path = "/voice-links", tag = "library",
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 202, body = LibraryTask), (status = "4XX", body = ErrorBody)))]
async fn upload_voice_link(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    body: Bytes,
) -> ApiResult<(StatusCode, Json<LibraryTask>)> {
    user.require(Role::Operator)?;
    if body.is_empty() {
        return Err(ApiError::BadRequest("empty audio take".to_owned()));
    }

    let mut rand_bytes = [0u8; 16];
    getrandom::fill(&mut rand_bytes)
        .map_err(|e| ApiError::Internal(format!("no randomness: {e}")))?;
    let take_id = blake3::hash(&rand_bytes).to_hex();
    let take_filename = format!("{take_id}.take");
    let target_dir = state.staging.join("voice-links");
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| ApiError::Internal(format!("cannot create staging directory: {e}")))?;

    let target_path = target_dir.join(&take_filename);
    tokio::fs::write(&target_path, &body)
        .await
        .map_err(|e| ApiError::Internal(format!("cannot write take to staging: {e}")))?;

    let rel_source = format!("voice-links/{take_filename}");
    let task = librarian::ask(
        &state,
        &user,
        TaskRequest::VoiceLinkImport {
            station_id: station.id(),
            source_path: rel_source,
            operator: user.username.clone(),
        },
    )
    .await?;

    Ok((StatusCode::ACCEPTED, Json(task)))
}

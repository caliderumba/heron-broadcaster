// SPDX-License-Identifier: AGPL-3.0-or-later

//! Listener song requests: public submission portal and operator moderation inbox.

use std::time::SystemTime;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use heron_core::PlaySource;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::app::AppState;
use crate::auth::{CurrentUser, Role};
use crate::engine::unix_ms;
use crate::error::{ApiError, ApiResult, ErrorBody};
use crate::stations::Station;

/// Status of a listener song request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    Pending,
    Approved,
    Rejected,
    Played,
    Expired,
}

impl RequestStatus {
    #[allow(dead_code)]
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Played => "played",
            Self::Expired => "expired",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            "played" => Some(Self::Played),
            "expired" => Some(Self::Expired),
            _ => None,
        }
    }
}

/// Station-level configuration for listener requests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct StationRequestsSettings {
    pub enabled: bool,
    pub cooldown_minutes: u32,
    pub max_pending: u32,
    pub daily_cap: u32,
    pub allow_dedication: bool,
    pub max_dedication_len: u32,
    pub title: String,
    pub subtitle: String,
}

impl Default for StationRequestsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            cooldown_minutes: 120,
            max_pending: 50,
            daily_cap: 5,
            allow_dedication: true,
            max_dedication_len: 200,
            title: "Song Requests".to_owned(),
            subtitle: "Search our library and send your favorite track to our on-air host"
                .to_owned(),
        }
    }
}

/// Partial update for station requests settings.
#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
pub struct UpdateRequestsSettings {
    pub enabled: Option<bool>,
    pub cooldown_minutes: Option<u32>,
    pub max_pending: Option<u32>,
    pub daily_cap: Option<u32>,
    pub allow_dedication: Option<bool>,
    pub max_dedication_len: Option<u32>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
}

/// Publicly exposed station request settings for the listener portal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct PublicRequestsSettings {
    pub enabled: bool,
    pub allow_dedication: bool,
    pub max_dedication_len: u32,
    pub title: String,
    pub subtitle: String,
}

/// Public track search query.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PublicSearchQuery {
    pub q: String,
}

/// A playable track found via public search.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct PublicSearchTrack {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: u32,
    pub can_request: bool,
    pub cooldown_until_unix_ms: Option<i64>,
}

/// Payload sent by a listener to request a song.
#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct SubmitRequestPayload {
    pub track_id: u64,
    pub listener_name: String,
    pub dedication: Option<String>,
    pub client_token: Option<String>,
}

/// Confirmation returned upon submitting a request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct SubmitRequestResponse {
    pub request_id: u64,
    pub status: RequestStatus,
    pub client_token: String,
    pub message: String,
}

/// Query parameters for listing requests in the operator moderation inbox.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListRequestsQuery {
    pub status: Option<RequestStatus>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Full details of a listener request for operator moderation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct ListenerRequestView {
    pub id: u64,
    pub station_id: u64,
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: u32,
    pub listener_name: String,
    pub dedication: String,
    pub status: RequestStatus,
    pub client_token: String,
    pub requested_at_unix_ms: i64,
    pub reviewed_at_unix_ms: Option<i64>,
    pub reviewed_by: Option<u64>,
    pub reviewed_by_name: Option<String>,
}

/// Optional positioning payload when approving a request.
#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
pub struct ApproveRequestPayload {
    /// 0 for top of queue / next to play, None for end of queue.
    pub position: Option<u32>,
}

/// Loads station requests settings, returning default if not yet inserted.
pub async fn load_settings(
    pool: &MySqlPool,
    station_id: u64,
) -> ApiResult<StationRequestsSettings> {
    let row: Option<StationRequestsSettings> = sqlx::query_as(
        r#"SELECT enabled, cooldown_minutes, max_pending, daily_cap, allow_dedication, max_dedication_len, title, subtitle
           FROM station_requests_settings WHERE station_id = ?"#,
    )
    .bind(station_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.unwrap_or_default())
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_requests_settings, update_requests_settings))
        .routes(routes!(list_requests))
        .routes(routes!(approve_request))
        .routes(routes!(reject_request))
        .routes(routes!(delete_request))
}

pub fn public_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(public_get_requests_settings))
        .routes(routes!(public_search_requests_tracks))
        .routes(routes!(public_submit_request))
}

// ---------------------------------------------------------------------------
// Operator endpoints (/stations/{id}/requests/...)
// ---------------------------------------------------------------------------

/// Gets station requests configuration.
#[utoipa::path(
    get,
    path = "/requests/settings",
    tag = "requests",
    responses(
        (status = 200, body = StationRequestsSettings),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn get_requests_settings(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
) -> ApiResult<Json<StationRequestsSettings>> {
    user.require(Role::Operator)?;
    let settings = load_settings(&state.pool, station.id()).await?;
    Ok(Json(settings))
}

/// Updates station requests configuration.
#[utoipa::path(
    put,
    path = "/requests/settings",
    tag = "requests",
    request_body = UpdateRequestsSettings,
    responses(
        (status = 200, body = StationRequestsSettings),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn update_requests_settings(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Json(payload): Json<UpdateRequestsSettings>,
) -> ApiResult<Json<StationRequestsSettings>> {
    user.require(Role::Admin)?;

    let current = load_settings(&state.pool, station.id()).await?;

    let enabled = payload.enabled.unwrap_or(current.enabled);
    let cooldown_minutes = payload.cooldown_minutes.unwrap_or(current.cooldown_minutes);
    let max_pending = payload.max_pending.unwrap_or(current.max_pending);
    let daily_cap = payload.daily_cap.unwrap_or(current.daily_cap);
    let allow_dedication = payload.allow_dedication.unwrap_or(current.allow_dedication);
    let max_dedication_len = payload
        .max_dedication_len
        .unwrap_or(current.max_dedication_len);
    let title = payload.title.unwrap_or(current.title);
    let subtitle = payload.subtitle.unwrap_or(current.subtitle);

    sqlx::query(
        r#"INSERT INTO station_requests_settings (station_id, enabled, cooldown_minutes, max_pending, daily_cap, allow_dedication, max_dedication_len, title, subtitle)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE
             enabled = VALUES(enabled),
             cooldown_minutes = VALUES(cooldown_minutes),
             max_pending = VALUES(max_pending),
             daily_cap = VALUES(daily_cap),
             allow_dedication = VALUES(allow_dedication),
             max_dedication_len = VALUES(max_dedication_len),
             title = VALUES(title),
             subtitle = VALUES(subtitle)"#,
    )
    .bind(station.id())
    .bind(enabled)
    .bind(cooldown_minutes)
    .bind(max_pending)
    .bind(daily_cap)
    .bind(allow_dedication)
    .bind(max_dedication_len)
    .bind(&title)
    .bind(&subtitle)
    .execute(&state.pool)
    .await?;

    let updated = StationRequestsSettings {
        enabled,
        cooldown_minutes,
        max_pending,
        daily_cap,
        allow_dedication,
        max_dedication_len,
        title,
        subtitle,
    };

    Ok(Json(updated))
}

/// Lists listener song requests for operator moderation.
#[utoipa::path(
    get,
    path = "/requests",
    tag = "requests",
    params(ListRequestsQuery),
    responses(
        (status = 200, body = Vec<ListenerRequestView>),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn list_requests(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Query(query): Query<ListRequestsQuery>,
) -> ApiResult<Json<Vec<ListenerRequestView>>> {
    user.require(Role::Operator)?;

    let limit = query.limit.unwrap_or(100).min(500);
    let offset = query.offset.unwrap_or(0);

    type RequestDbRow = (
        u64,            // r.id
        u64,            // r.station_id
        u64,            // r.track_id
        String,         // t.title
        String,         // t.artist
        Option<String>, // t.album
        u32,            // t.duration_ms
        String,         // r.listener_name
        String,         // r.dedication
        String,         // r.status
        String,         // r.client_token
        i64,            // r.requested_at_unix_ms
        Option<i64>,    // r.reviewed_at_unix_ms
        Option<u64>,    // r.reviewed_by
        Option<String>, // u.username
    );

    let rows: Vec<RequestDbRow> = sqlx::query_as(
        r#"SELECT r.id, r.station_id, r.track_id, t.title, t.artist, t.album, t.duration_ms,
                  r.listener_name, r.dedication, CAST(r.status AS CHAR), r.client_token,
                  r.requested_at_unix_ms, r.reviewed_at_unix_ms, r.reviewed_by, u.username
           FROM listener_requests r
           JOIN tracks t ON r.track_id = t.id
           LEFT JOIN users u ON r.reviewed_by = u.id
           WHERE r.station_id = ?
           ORDER BY r.requested_at_unix_ms DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(station.id())
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    let result = rows
        .into_iter()
        .filter_map(|r| {
            let status = RequestStatus::parse(&r.9)?;
            if query
                .status
                .is_some_and(|filter_status| status != filter_status)
            {
                return None;
            }
            Some(ListenerRequestView {
                id: r.0,
                station_id: r.1,
                track_id: r.2,
                title: r.3,
                artist: r.4,
                album: r.5,
                duration_ms: r.6,
                listener_name: r.7,
                dedication: r.8,
                status,
                client_token: r.10,
                requested_at_unix_ms: r.11,
                reviewed_at_unix_ms: r.12,
                reviewed_by: r.13,
                reviewed_by_name: r.14,
            })
        })
        .collect();

    Ok(Json(result))
}

/// Approves a pending song request and enqueues the track with source = request.
#[utoipa::path(
    post,
    path = "/requests/{id}/approve",
    tag = "requests",
    params(("id" = u64, Path, description = "The request ID")),
    request_body = ApproveRequestPayload,
    responses(
        (status = 200, body = ListenerRequestView),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn approve_request(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Path(id): Path<u64>,
    Json(payload): Json<ApproveRequestPayload>,
) -> ApiResult<Json<ListenerRequestView>> {
    user.require(Role::Operator)?;

    type ReqRow = (u64, u64, String);
    let req: Option<ReqRow> = sqlx::query_as(
        r#"SELECT id, track_id, CAST(status AS CHAR) FROM listener_requests WHERE id = ? AND station_id = ?"#,
    )
    .bind(id)
    .bind(station.id())
    .fetch_optional(&state.pool)
    .await?;

    let (_, track_id, status_str) =
        req.ok_or_else(|| ApiError::NotFound("listener request not found".into()))?;

    let status = RequestStatus::parse(&status_str).unwrap_or(RequestStatus::Pending);
    if status != RequestStatus::Pending {
        return Err(ApiError::BadRequest(
            "only pending requests can be approved".into(),
        ));
    }

    // Enqueue the track with PlaySource::Request
    let proto_tracks = station
        .queue
        .tracks(&[track_id], PlaySource::Request)
        .await?;

    let position = payload.position.or(Some(0)); // default to 0 (next track)
    station.engine.enqueue(proto_tracks, position).await?;

    let now = unix_ms(SystemTime::now()) as i64;
    sqlx::query(
        r#"UPDATE listener_requests SET status = 'approved', reviewed_at_unix_ms = ?, reviewed_by = ? WHERE id = ?"#,
    )
    .bind(now)
    .bind(user.id)
    .bind(id)
    .execute(&state.pool)
    .await?;

    // Return the updated view
    type FullRow = (
        u64,
        u64,
        u64,
        String,
        String,
        Option<String>,
        u32,
        String,
        String,
        String,
        i64,
        Option<i64>,
        Option<u64>,
        Option<String>,
    );

    let row: FullRow = sqlx::query_as(
        r#"SELECT r.id, r.station_id, r.track_id, t.title, t.artist, t.album, t.duration_ms,
                  r.listener_name, r.dedication, r.client_token,
                  r.requested_at_unix_ms, r.reviewed_at_unix_ms, r.reviewed_by, u.username
           FROM listener_requests r
           JOIN tracks t ON r.track_id = t.id
           LEFT JOIN users u ON r.reviewed_by = u.id
           WHERE r.id = ?"#,
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(ListenerRequestView {
        id: row.0,
        station_id: row.1,
        track_id: row.2,
        title: row.3,
        artist: row.4,
        album: row.5,
        duration_ms: row.6,
        listener_name: row.7,
        dedication: row.8,
        status: RequestStatus::Approved,
        client_token: row.9,
        requested_at_unix_ms: row.10,
        reviewed_at_unix_ms: row.11,
        reviewed_by: row.12,
        reviewed_by_name: row.13,
    }))
}

/// Rejects a pending song request.
#[utoipa::path(
    post,
    path = "/requests/{id}/reject",
    tag = "requests",
    params(("id" = u64, Path, description = "The request ID")),
    responses(
        (status = 200, body = ListenerRequestView),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn reject_request(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Path(id): Path<u64>,
) -> ApiResult<Json<ListenerRequestView>> {
    user.require(Role::Operator)?;

    let now = unix_ms(SystemTime::now()) as i64;
    let res = sqlx::query(
        r#"UPDATE listener_requests SET status = 'rejected', reviewed_at_unix_ms = ?, reviewed_by = ? WHERE id = ? AND station_id = ?"#,
    )
    .bind(now)
    .bind(user.id)
    .bind(id)
    .bind(station.id())
    .execute(&state.pool)
    .await?;

    if res.rows_affected() == 0 {
        return Err(ApiError::NotFound("listener request not found".into()));
    }

    type FullRow = (
        u64,
        u64,
        u64,
        String,
        String,
        Option<String>,
        u32,
        String,
        String,
        String,
        i64,
        Option<i64>,
        Option<u64>,
        Option<String>,
    );

    let row: FullRow = sqlx::query_as(
        r#"SELECT r.id, r.station_id, r.track_id, t.title, t.artist, t.album, t.duration_ms,
                  r.listener_name, r.dedication, r.client_token,
                  r.requested_at_unix_ms, r.reviewed_at_unix_ms, r.reviewed_by, u.username
           FROM listener_requests r
           JOIN tracks t ON r.track_id = t.id
           LEFT JOIN users u ON r.reviewed_by = u.id
           WHERE r.id = ?"#,
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(ListenerRequestView {
        id: row.0,
        station_id: row.1,
        track_id: row.2,
        title: row.3,
        artist: row.4,
        album: row.5,
        duration_ms: row.6,
        listener_name: row.7,
        dedication: row.8,
        status: RequestStatus::Rejected,
        client_token: row.9,
        requested_at_unix_ms: row.10,
        reviewed_at_unix_ms: row.11,
        reviewed_by: row.12,
        reviewed_by_name: row.13,
    }))
}

/// Deletes a listener request record from the database.
#[utoipa::path(
    delete,
    path = "/requests/{id}",
    tag = "requests",
    params(("id" = u64, Path, description = "The request ID")),
    responses(
        (status = 204),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn delete_request(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Path(id): Path<u64>,
) -> ApiResult<StatusCode> {
    user.require(Role::Operator)?;

    let res = sqlx::query(r#"DELETE FROM listener_requests WHERE id = ? AND station_id = ?"#)
        .bind(id)
        .bind(station.id())
        .execute(&state.pool)
        .await?;

    if res.rows_affected() == 0 {
        return Err(ApiError::NotFound("listener request not found".into()));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Public endpoints (/public/requests/...)
// ---------------------------------------------------------------------------

/// Gets public station request portal configuration and branding.
#[utoipa::path(
    get,
    path = "/public/requests/settings",
    tag = "public",
    security(()),
    responses(
        (status = 200, body = PublicRequestsSettings),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn public_get_requests_settings(
    State(state): State<AppState>,
    station: Station,
) -> ApiResult<Json<PublicRequestsSettings>> {
    let s = load_settings(&state.pool, station.id()).await?;
    Ok(Json(PublicRequestsSettings {
        enabled: s.enabled,
        allow_dedication: s.allow_dedication,
        max_dedication_len: s.max_dedication_len,
        title: s.title,
        subtitle: s.subtitle,
    }))
}

/// Searches playable songs in the station's library for audience request.
#[utoipa::path(
    get,
    path = "/public/requests/search",
    tag = "public",
    security(()),
    params(PublicSearchQuery),
    responses(
        (status = 200, body = Vec<PublicSearchTrack>),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn public_search_requests_tracks(
    State(state): State<AppState>,
    station: Station,
    Query(query): Query<PublicSearchQuery>,
) -> ApiResult<Json<Vec<PublicSearchTrack>>> {
    let settings = load_settings(&state.pool, station.id()).await?;
    if !settings.enabled {
        return Err(ApiError::BadRequest(
            "song requests are currently disabled for this station".into(),
        ));
    }

    let q = query.q.trim();
    if q.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let search_pattern = format!("%{q}%");
    let now = unix_ms(SystemTime::now()) as i64;
    let cooldown_ms = (settings.cooldown_minutes as i64) * 60 * 1000;

    type TrackRow = (u64, String, String, Option<String>, u32);
    let tracks: Vec<TrackRow> = sqlx::query_as(
        r#"SELECT id, title, artist, album, duration_ms
           FROM tracks
           WHERE enabled = TRUE
             AND missing = FALSE
             AND (title LIKE ? OR artist LIKE ? OR album LIKE ?)
           ORDER BY title ASC
           LIMIT 20"#,
    )
    .bind(&search_pattern)
    .bind(&search_pattern)
    .bind(&search_pattern)
    .fetch_all(&state.pool)
    .await?;

    let mut results = Vec::with_capacity(tracks.len());
    for t in tracks {
        // Check cooldown in listener_requests for this station and track
        let last_play: Option<(i64,)> = sqlx::query_as(
            r#"SELECT requested_at_unix_ms
               FROM listener_requests
               WHERE station_id = ?
                 AND track_id = ?
                 AND status IN ('pending', 'approved', 'played')
                 AND requested_at_unix_ms > ?
               ORDER BY requested_at_unix_ms DESC
               LIMIT 1"#,
        )
        .bind(station.id())
        .bind(t.0)
        .bind(now - cooldown_ms)
        .fetch_optional(&state.pool)
        .await?;

        let (can_request, cooldown_until_unix_ms) = match last_play {
            Some((requested_at,)) => (false, Some(requested_at + cooldown_ms)),
            None => (true, None),
        };

        results.push(PublicSearchTrack {
            id: t.0,
            title: t.1,
            artist: t.2,
            album: t.3,
            duration_ms: t.4,
            can_request,
            cooldown_until_unix_ms,
        });
    }

    Ok(Json(results))
}

/// Submits a new listener song request.
#[utoipa::path(
    post,
    path = "/public/requests",
    tag = "public",
    security(()),
    request_body = SubmitRequestPayload,
    responses(
        (status = 200, body = SubmitRequestResponse),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn public_submit_request(
    State(state): State<AppState>,
    station: Station,
    Json(payload): Json<SubmitRequestPayload>,
) -> ApiResult<Json<SubmitRequestResponse>> {
    let settings = load_settings(&state.pool, station.id()).await?;
    if !settings.enabled {
        return Err(ApiError::BadRequest(
            "song requests are currently disabled for this station".into(),
        ));
    }

    let name = payload.listener_name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(ApiError::BadRequest(
            "please provide a listener name (1 to 100 characters)".into(),
        ));
    }

    let dedication = if settings.allow_dedication {
        let raw = payload.dedication.unwrap_or_default().trim().to_owned();
        if raw.len() > settings.max_dedication_len as usize {
            return Err(ApiError::BadRequest(format!(
                "dedication message is too long (maximum {} characters)",
                settings.max_dedication_len
            )));
        }
        raw
    } else {
        String::new()
    };

    let client_token = payload
        .client_token
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| {
            let mut bytes = [0u8; 16];
            let _ = getrandom::fill(&mut bytes);
            bytes.iter().map(|b| format!("{b:02x}")).collect()
        });

    // Check track exists and is playable
    let track_exists: Option<(u64,)> = sqlx::query_as(
        r#"SELECT id FROM tracks WHERE id = ? AND enabled = TRUE AND missing = FALSE"#,
    )
    .bind(payload.track_id)
    .fetch_optional(&state.pool)
    .await?;

    if track_exists.is_none() {
        return Err(ApiError::NotFound("track not found or not playable".into()));
    }

    let now = unix_ms(SystemTime::now()) as i64;
    let cooldown_ms = (settings.cooldown_minutes as i64) * 60 * 1000;

    // Check track cooldown
    let recent_track_req: Option<(i64,)> = sqlx::query_as(
        r#"SELECT requested_at_unix_ms
           FROM listener_requests
           WHERE station_id = ?
             AND track_id = ?
             AND status IN ('pending', 'approved', 'played')
             AND requested_at_unix_ms > ?
           LIMIT 1"#,
    )
    .bind(station.id())
    .bind(payload.track_id)
    .bind(now - cooldown_ms)
    .fetch_optional(&state.pool)
    .await?;

    if let Some((prev,)) = recent_track_req {
        let remaining_mins = ((prev + cooldown_ms - now) / 60000).max(1);
        return Err(ApiError::Conflict(format!(
            "this track was requested recently; cooldown active for {remaining_mins} more minutes"
        )));
    }

    // Check pending cap
    let pending_count: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM listener_requests WHERE station_id = ? AND status = 'pending'"#,
    )
    .bind(station.id())
    .fetch_one(&state.pool)
    .await?;

    if pending_count.0 >= settings.max_pending as i64 {
        return Err(ApiError::Conflict(
            "too many pending requests; please try again shortly".into(),
        ));
    }

    // Check client daily cap
    let one_day_ms = 24 * 60 * 60 * 1000;
    let client_count: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM listener_requests WHERE station_id = ? AND client_token = ? AND requested_at_unix_ms > ?"#,
    )
    .bind(station.id())
    .bind(&client_token)
    .bind(now - one_day_ms)
    .fetch_one(&state.pool)
    .await?;

    if client_count.0 >= settings.daily_cap as i64 {
        return Err(ApiError::Conflict(format!(
            "you have reached your daily request limit ({} requests per day)",
            settings.daily_cap
        )));
    }

    let res = sqlx::query(
        r#"INSERT INTO listener_requests (station_id, track_id, listener_name, dedication, status, client_token, requested_at_unix_ms)
           VALUES (?, ?, ?, ?, 'pending', ?, ?)"#,
    )
    .bind(station.id())
    .bind(payload.track_id)
    .bind(name)
    .bind(&dedication)
    .bind(&client_token)
    .bind(now)
    .execute(&state.pool)
    .await?;

    let req_id = res.last_insert_id();

    Ok(Json(SubmitRequestResponse {
        request_id: req_id,
        status: RequestStatus::Pending,
        client_token,
        message: "Your song request has been submitted to the station DJ!".to_owned(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_status_parsing() {
        assert_eq!(
            RequestStatus::parse("pending"),
            Some(RequestStatus::Pending)
        );
        assert_eq!(
            RequestStatus::parse("approved"),
            Some(RequestStatus::Approved)
        );
        assert_eq!(
            RequestStatus::parse("rejected"),
            Some(RequestStatus::Rejected)
        );
        assert_eq!(RequestStatus::parse("played"), Some(RequestStatus::Played));
        assert_eq!(
            RequestStatus::parse("expired"),
            Some(RequestStatus::Expired)
        );
        assert_eq!(RequestStatus::parse("unknown"), None);

        assert_eq!(RequestStatus::Pending.as_str(), "pending");
        assert_eq!(RequestStatus::Approved.as_str(), "approved");
        assert_eq!(RequestStatus::Rejected.as_str(), "rejected");
        assert_eq!(RequestStatus::Played.as_str(), "played");
        assert_eq!(RequestStatus::Expired.as_str(), "expired");
    }

    #[test]
    fn test_default_settings() {
        let settings = StationRequestsSettings::default();
        assert!(settings.enabled);
        assert_eq!(settings.cooldown_minutes, 120);
        assert_eq!(settings.max_pending, 50);
        assert_eq!(settings.daily_cap, 5);
        assert!(settings.allow_dedication);
        assert_eq!(settings.max_dedication_len, 200);
    }
}

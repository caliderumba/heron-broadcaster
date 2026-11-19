// SPDX-License-Identifier: AGPL-3.0-or-later

//! Scheduled events: timed actions that fire on schedule.
//!
//! A station has named events with a trigger (recurring on days of week + time
//! of day, fixed interval, or one-off timestamp), a timing mode ('exact' or
//! 'next'), and an action ('play_track', 'play_category', 'relay', 'load_clock').

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use jiff::civil::Time;
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use heron_core::PlaySource;

use crate::app::AppState;
use crate::auth::{CurrentUser, Role};
use crate::error::{ApiError, ApiResult, ErrorBody};
use crate::settings::StationSettings;
use crate::stations::{Station, StationRuntime};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_events, create_event))
        .routes(routes!(get_event, update_event, delete_event))
        .routes(routes!(run_event_now))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    Recurring,
    Interval,
    Once,
}

impl TriggerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Recurring => "recurring",
            Self::Interval => "interval",
            Self::Once => "once",
        }
    }
}

impl FromStr for TriggerType {
    type Err = ApiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "recurring" => Ok(Self::Recurring),
            "interval" => Ok(Self::Interval),
            "once" => Ok(Self::Once),
            other => Err(ApiError::BadRequest(format!(
                "invalid trigger type: {other}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimingMode {
    Exact,
    Next,
}

impl TimingMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Next => "next",
        }
    }
}

impl FromStr for TimingMode {
    type Err = ApiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "exact" => Ok(Self::Exact),
            "next" => Ok(Self::Next),
            other => Err(ApiError::BadRequest(format!(
                "invalid timing mode: {other}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    PlayTrack,
    PlayCategory,
    Relay,
    LoadClock,
    AnnounceTime,
    AnnounceWeather,
    PlayCampaign,
    PlayAdBlock,
}

impl ActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PlayTrack => "play_track",
            Self::PlayCategory => "play_category",
            Self::Relay => "relay",
            Self::LoadClock => "load_clock",
            Self::AnnounceTime => "announce_time",
            Self::AnnounceWeather => "announce_weather",
            Self::PlayCampaign => "play_campaign",
            Self::PlayAdBlock => "play_ad_block",
        }
    }
}

impl FromStr for ActionType {
    type Err = ApiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "play_track" => Ok(Self::PlayTrack),
            "play_category" => Ok(Self::PlayCategory),
            "relay" => Ok(Self::Relay),
            "load_clock" => Ok(Self::LoadClock),
            "announce_time" => Ok(Self::AnnounceTime),
            "announce_weather" => Ok(Self::AnnounceWeather),
            "play_campaign" => Ok(Self::PlayCampaign),
            "play_ad_block" => Ok(Self::PlayAdBlock),
            other => Err(ApiError::BadRequest(format!(
                "invalid action type: {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ScheduledEvent {
    pub id: u64,
    pub station_id: u64,
    pub name: String,
    pub enabled: bool,
    pub trigger_type: TriggerType,
    pub days_mask: u8,
    pub time_of_day_s: u32,
    pub interval_s: Option<u32>,
    pub scheduled_at_unix_ms: Option<u64>,
    pub timing_mode: TimingMode,
    pub action_type: ActionType,
    pub action_data: serde_json::Value,
    pub last_run_at_unix_ms: Option<u64>,
    pub next_run_at_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateEventRequest {
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub trigger_type: TriggerType,
    #[serde(default = "default_days_mask")]
    pub days_mask: u8,
    #[serde(default)]
    pub time_of_day_s: u32,
    pub interval_s: Option<u32>,
    pub scheduled_at_unix_ms: Option<u64>,
    #[serde(default = "default_timing_mode")]
    pub timing_mode: TimingMode,
    pub action_type: ActionType,
    pub action_data: serde_json::Value,
}

fn default_true() -> bool {
    true
}

fn default_days_mask() -> u8 {
    127
}

fn default_timing_mode() -> TimingMode {
    TimingMode::Exact
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateEventRequest {
    pub name: String,
    pub enabled: bool,
    pub trigger_type: TriggerType,
    pub days_mask: u8,
    pub time_of_day_s: u32,
    pub interval_s: Option<u32>,
    pub scheduled_at_unix_ms: Option<u64>,
    pub timing_mode: TimingMode,
    pub action_type: ActionType,
    pub action_data: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct RunNowResponse {
    pub success: bool,
    pub message: String,
}

#[derive(sqlx::FromRow)]
struct EventRow {
    id: u64,
    station_id: u64,
    name: String,
    enabled: bool,
    trigger_type: String,
    days_mask: u8,
    time_of_day_s: u32,
    interval_s: Option<u32>,
    scheduled_at_unix_ms: Option<u64>,
    timing_mode: String,
    action_type: String,
    action_data: String,
    last_run_at_unix_ms: Option<u64>,
    next_run_at_unix_ms: Option<u64>,
}

impl TryFrom<EventRow> for ScheduledEvent {
    type Error = ApiError;

    fn try_from(row: EventRow) -> Result<Self, Self::Error> {
        let action_data: serde_json::Value =
            serde_json::from_str(&row.action_data).unwrap_or_else(|_| serde_json::json!({}));
        Ok(Self {
            id: row.id,
            station_id: row.station_id,
            name: row.name,
            enabled: row.enabled,
            trigger_type: row.trigger_type.parse()?,
            days_mask: row.days_mask,
            time_of_day_s: row.time_of_day_s,
            interval_s: row.interval_s,
            scheduled_at_unix_ms: row.scheduled_at_unix_ms,
            timing_mode: row.timing_mode.parse()?,
            action_type: row.action_type.parse()?,
            action_data,
            last_run_at_unix_ms: row.last_run_at_unix_ms,
            next_run_at_unix_ms: row.next_run_at_unix_ms,
        })
    }
}

/// Computes the next run timestamp in ms for an event given the current station time.
pub fn compute_next_run(
    trigger_type: TriggerType,
    days_mask: u8,
    time_of_day_s: u32,
    interval_s: Option<u32>,
    scheduled_at_unix_ms: Option<u64>,
    tz: &TimeZone,
    now_ms: u64,
) -> Option<u64> {
    match trigger_type {
        TriggerType::Recurring => {
            if days_mask == 0 {
                return None;
            }
            let hour = ((time_of_day_s / 3600) % 24) as i8;
            let minute = ((time_of_day_s % 3600) / 60) as i8;
            let second = (time_of_day_s % 60) as i8;
            let time = Time::new(hour, minute, second, 0).ok()?;
            let now_ts = Timestamp::from_millisecond(now_ms as i64).ok()?;
            let zoned_now = now_ts.to_zoned(tz.clone());
            let today = zoned_now.date();

            for day_offset in 0..=7 {
                let candidate_date = today.checked_add(Span::new().days(day_offset)).ok()?;
                let weekday = candidate_date.weekday();
                let offset = weekday.to_monday_zero_offset() as u8;
                let day_bit = 1 << offset;
                if (days_mask & day_bit) != 0 {
                    let candidate_zoned =
                        candidate_date.to_datetime(time).to_zoned(tz.clone()).ok()?;
                    let candidate_ms = candidate_zoned.timestamp().as_millisecond() as u64;
                    if candidate_ms > now_ms {
                        return Some(candidate_ms);
                    }
                }
            }
            None
        }
        TriggerType::Interval => {
            let interval = interval_s?;
            if interval == 0 {
                return None;
            }
            Some(now_ms + (interval as u64) * 1000)
        }
        TriggerType::Once => {
            let scheduled = scheduled_at_unix_ms?;
            if scheduled > now_ms {
                Some(scheduled)
            } else {
                None
            }
        }
    }
}

/// Lists all scheduled events of the station.
async fn station_tz(pool: &sqlx::MySqlPool, station_id: u64) -> TimeZone {
    crate::settings::load_for::<StationSettings>(pool, station_id, StationSettings::SETTINGS_NAME)
        .await
        .map(|s| s.time_zone())
        .unwrap_or(TimeZone::UTC)
}

#[utoipa::path(
    get,
    path = "/events",
    tag = "events",
    responses(
        (status = 200, body = Vec<ScheduledEvent>),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn list_events(
    _user: CurrentUser,
    station: Station,
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<ScheduledEvent>>> {
    let rows: Vec<EventRow> = sqlx::query_as(
        r#"SELECT id, station_id, name, enabled, trigger_type, days_mask,
                  time_of_day_s, interval_s, scheduled_at_unix_ms, timing_mode,
                  action_type, action_data, last_run_at_unix_ms, next_run_at_unix_ms
           FROM scheduled_events
           WHERE station_id = ?
           ORDER BY name ASC"#,
    )
    .bind(station.id())
    .fetch_all(&state.pool)
    .await?;

    let mut events = Vec::with_capacity(rows.len());
    for row in rows {
        events.push(ScheduledEvent::try_from(row)?);
    }
    Ok(Json(events))
}

/// Reads one scheduled event by ID.
#[utoipa::path(
    get,
    path = "/events/{id}",
    tag = "events",
    params(("id" = u64, Path, description = "Event ID")),
    responses(
        (status = 200, body = ScheduledEvent),
        (status = 404, body = ErrorBody)
    )
)]
async fn get_event(
    _user: CurrentUser,
    station: Station,
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> ApiResult<Json<ScheduledEvent>> {
    let row: Option<EventRow> = sqlx::query_as(
        r#"SELECT id, station_id, name, enabled, trigger_type, days_mask,
                  time_of_day_s, interval_s, scheduled_at_unix_ms, timing_mode,
                  action_type, action_data, last_run_at_unix_ms, next_run_at_unix_ms
           FROM scheduled_events
           WHERE station_id = ? AND id = ?"#,
    )
    .bind(station.id())
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| ApiError::not_found("event"))?;
    Ok(Json(ScheduledEvent::try_from(row)?))
}

/// Creates a new scheduled event.
#[utoipa::path(
    post,
    path = "/events",
    tag = "events",
    request_body = CreateEventRequest,
    responses(
        (status = 201, body = ScheduledEvent),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn create_event(
    user: CurrentUser,
    station: Station,
    State(state): State<AppState>,
    Json(req): Json<CreateEventRequest>,
) -> ApiResult<(StatusCode, Json<ScheduledEvent>)> {
    user.require(Role::Operator)?;

    let name = req.name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(ApiError::BadRequest(
            "name must be 1 to 64 characters".to_owned(),
        ));
    }

    let tz = station_tz(&state.pool, station.id()).await;
    let now_ms = Timestamp::now().as_millisecond() as u64;

    let next_run = if req.enabled {
        compute_next_run(
            req.trigger_type,
            req.days_mask,
            req.time_of_day_s,
            req.interval_s,
            req.scheduled_at_unix_ms,
            &tz,
            now_ms,
        )
    } else {
        None
    };

    let action_data_json =
        serde_json::to_string(&req.action_data).unwrap_or_else(|_| "{}".to_owned());

    let result = sqlx::query(
        r#"INSERT INTO scheduled_events
           (station_id, name, enabled, trigger_type, days_mask, time_of_day_s,
            interval_s, scheduled_at_unix_ms, timing_mode, action_type,
            action_data, next_run_at_unix_ms)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(station.id())
    .bind(name)
    .bind(req.enabled)
    .bind(req.trigger_type.as_str())
    .bind(req.days_mask)
    .bind(req.time_of_day_s)
    .bind(req.interval_s)
    .bind(req.scheduled_at_unix_ms)
    .bind(req.timing_mode.as_str())
    .bind(req.action_type.as_str())
    .bind(action_data_json)
    .bind(next_run)
    .execute(&state.pool)
    .await?;

    let event_id = result.last_insert_id();

    let event = ScheduledEvent {
        id: event_id,
        station_id: station.id(),
        name: name.to_owned(),
        enabled: req.enabled,
        trigger_type: req.trigger_type,
        days_mask: req.days_mask,
        time_of_day_s: req.time_of_day_s,
        interval_s: req.interval_s,
        scheduled_at_unix_ms: req.scheduled_at_unix_ms,
        timing_mode: req.timing_mode,
        action_type: req.action_type,
        action_data: req.action_data,
        last_run_at_unix_ms: None,
        next_run_at_unix_ms: next_run,
    };

    tracing::info!(station_id = station.id(), event_id, name = %event.name, "scheduled event created");
    Ok((StatusCode::CREATED, Json(event)))
}

/// Updates an existing scheduled event.
#[utoipa::path(
    put,
    path = "/events/{id}",
    tag = "events",
    params(("id" = u64, Path, description = "Event ID")),
    request_body = UpdateEventRequest,
    responses(
        (status = 200, body = ScheduledEvent),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn update_event(
    user: CurrentUser,
    station: Station,
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<UpdateEventRequest>,
) -> ApiResult<Json<ScheduledEvent>> {
    user.require(Role::Operator)?;

    let name = req.name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(ApiError::BadRequest(
            "name must be 1 to 64 characters".to_owned(),
        ));
    }

    let existing: Option<EventRow> = sqlx::query_as(
        r#"SELECT id, station_id, name, enabled, trigger_type, days_mask,
                  time_of_day_s, interval_s, scheduled_at_unix_ms, timing_mode,
                  action_type, action_data, last_run_at_unix_ms, next_run_at_unix_ms
           FROM scheduled_events
           WHERE station_id = ? AND id = ?"#,
    )
    .bind(station.id())
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let existing = existing.ok_or_else(|| ApiError::not_found("event"))?;

    let tz = station_tz(&state.pool, station.id()).await;
    let now_ms = Timestamp::now().as_millisecond() as u64;

    let next_run = if req.enabled {
        compute_next_run(
            req.trigger_type,
            req.days_mask,
            req.time_of_day_s,
            req.interval_s,
            req.scheduled_at_unix_ms,
            &tz,
            now_ms,
        )
    } else {
        None
    };

    let action_data_json =
        serde_json::to_string(&req.action_data).unwrap_or_else(|_| "{}".to_owned());

    sqlx::query(
        r#"UPDATE scheduled_events
           SET name = ?, enabled = ?, trigger_type = ?, days_mask = ?,
               time_of_day_s = ?, interval_s = ?, scheduled_at_unix_ms = ?,
               timing_mode = ?, action_type = ?, action_data = ?,
               next_run_at_unix_ms = ?
           WHERE station_id = ? AND id = ?"#,
    )
    .bind(name)
    .bind(req.enabled)
    .bind(req.trigger_type.as_str())
    .bind(req.days_mask)
    .bind(req.time_of_day_s)
    .bind(req.interval_s)
    .bind(req.scheduled_at_unix_ms)
    .bind(req.timing_mode.as_str())
    .bind(req.action_type.as_str())
    .bind(action_data_json)
    .bind(next_run)
    .bind(station.id())
    .bind(id)
    .execute(&state.pool)
    .await?;

    let event = ScheduledEvent {
        id,
        station_id: station.id(),
        name: name.to_owned(),
        enabled: req.enabled,
        trigger_type: req.trigger_type,
        days_mask: req.days_mask,
        time_of_day_s: req.time_of_day_s,
        interval_s: req.interval_s,
        scheduled_at_unix_ms: req.scheduled_at_unix_ms,
        timing_mode: req.timing_mode,
        action_type: req.action_type,
        action_data: req.action_data,
        last_run_at_unix_ms: existing.last_run_at_unix_ms,
        next_run_at_unix_ms: next_run,
    };

    tracing::info!(station_id = station.id(), event_id = id, name = %event.name, "scheduled event updated");
    Ok(Json(event))
}

/// Deletes a scheduled event.
#[utoipa::path(
    delete,
    path = "/events/{id}",
    tag = "events",
    params(("id" = u64, Path, description = "Event ID")),
    responses(
        (status = 204),
        (status = 404, body = ErrorBody)
    )
)]
async fn delete_event(
    user: CurrentUser,
    station: Station,
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> ApiResult<StatusCode> {
    user.require(Role::Operator)?;

    let result = sqlx::query("DELETE FROM scheduled_events WHERE station_id = ? AND id = ?")
        .bind(station.id())
        .bind(id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("event"));
    }

    tracing::info!(
        station_id = station.id(),
        event_id = id,
        "scheduled event deleted"
    );
    Ok(StatusCode::NO_CONTENT)
}

/// Triggers an event immediately (manual test run).
#[utoipa::path(
    post,
    path = "/events/{id}/run-now",
    tag = "events",
    params(("id" = u64, Path, description = "Event ID")),
    responses(
        (status = 200, body = RunNowResponse),
        (status = 404, body = ErrorBody),
        (status = 503, body = ErrorBody)
    )
)]
async fn run_event_now(
    user: CurrentUser,
    station: Station,
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> ApiResult<Json<RunNowResponse>> {
    user.require(Role::Operator)?;

    let row: Option<EventRow> = sqlx::query_as(
        r#"SELECT id, station_id, name, enabled, trigger_type, days_mask,
                  time_of_day_s, interval_s, scheduled_at_unix_ms, timing_mode,
                  action_type, action_data, last_run_at_unix_ms, next_run_at_unix_ms
           FROM scheduled_events
           WHERE station_id = ? AND id = ?"#,
    )
    .bind(station.id())
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| ApiError::not_found("event"))?;
    let event = ScheduledEvent::try_from(row)?;

    execute_event(&state.pool, &station.0, &event).await?;

    let now_ms = Timestamp::now().as_millisecond() as u64;
    sqlx::query("UPDATE scheduled_events SET last_run_at_unix_ms = ? WHERE id = ?")
        .bind(now_ms)
        .bind(id)
        .execute(&state.pool)
        .await?;

    Ok(Json(RunNowResponse {
        success: true,
        message: format!("event '{}' executed successfully", event.name),
    }))
}

/// Dispatches an event's action to the station runtime.
pub async fn execute_event(
    pool: &sqlx::MySqlPool,
    station: &StationRuntime,
    event: &ScheduledEvent,
) -> ApiResult<()> {
    tracing::info!(
        station_id = station.id(),
        event_id = event.id,
        name = %event.name,
        action = event.action_type.as_str(),
        timing = event.timing_mode.as_str(),
        "executing scheduled event"
    );

    match event.action_type {
        ActionType::PlayTrack => {
            let track_id = event
                .action_data
                .get("track_id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    ApiError::BadRequest("missing track_id in action_data".to_owned())
                })?;

            let mut tracks = station
                .queue
                .tracks(&[track_id], PlaySource::Manual)
                .await?;
            let track = tracks.pop().ok_or_else(|| ApiError::not_found("track"))?;

            match event.timing_mode {
                TimingMode::Exact => {
                    station.engine.enqueue(vec![track], Some(0)).await?;
                    station.engine.skip().await?;
                }
                TimingMode::Next => {
                    station.engine.enqueue(vec![track], Some(0)).await?;
                }
            }
        }
        ActionType::PlayCategory => {
            let category_id = event
                .action_data
                .get("category_id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    ApiError::BadRequest("missing category_id in action_data".to_owned())
                })?;

            let track_id: Option<u64> = sqlx::query_scalar(
                r#"SELECT t.id FROM tracks t
                   JOIN track_categories tc ON t.id = tc.track_id
                   WHERE tc.category_id = ? AND t.enabled = TRUE
                   ORDER BY t.last_played_at_unix_ms ASC, t.id ASC
                   LIMIT 1"#,
            )
            .bind(category_id)
            .fetch_optional(pool)
            .await?;

            if let Some(track_id) = track_id {
                let mut tracks = station
                    .queue
                    .tracks(&[track_id], PlaySource::Manual)
                    .await?;
                if let Some(track) = tracks.pop() {
                    match event.timing_mode {
                        TimingMode::Exact => {
                            station.engine.enqueue(vec![track], Some(0)).await?;
                            station.engine.skip().await?;
                        }
                        TimingMode::Next => {
                            station.engine.enqueue(vec![track], Some(0)).await?;
                        }
                    }
                }
            } else {
                tracing::warn!(
                    category_id,
                    "no playable tracks found in scheduled category"
                );
            }
        }
        ActionType::Relay => {
            let url = event
                .action_data
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            tracing::info!(
                station_id = station.id(),
                url,
                "scheduled relay action invoked"
            );
        }
        ActionType::LoadClock => {
            let clock_id = event
                .action_data
                .get("clock_id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    ApiError::BadRequest("missing clock_id in action_data".to_owned())
                })?;

            let exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM clocks WHERE id = ?")
                .bind(clock_id)
                .fetch_one(pool)
                .await?;

            if !exists {
                return Err(ApiError::not_found("clock"));
            }
            tracing::info!(
                station_id = station.id(),
                clock_id,
                "scheduled load_clock action invoked"
            );
        }
        ActionType::AnnounceTime => {
            tracing::info!(
                station_id = station.id(),
                "scheduled announce_time action invoked"
            );
        }
        ActionType::AnnounceWeather => {
            tracing::info!(
                station_id = station.id(),
                "scheduled announce_weather action invoked"
            );
        }
        ActionType::PlayCampaign => {
            tracing::info!(
                station_id = station.id(),
                "scheduled play_campaign action invoked"
            );
        }
        ActionType::PlayAdBlock => {
            tracing::info!(
                station_id = station.id(),
                "scheduled play_ad_block action invoked"
            );
        }
    }

    Ok(())
}

/// The scheduler background worker for a single station.
pub async fn run_scheduler(pool: sqlx::MySqlPool, station: Arc<StationRuntime>) {
    let mut interval = tokio::time::interval(Duration::from_millis(500));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;

        let now_ms = Timestamp::now().as_millisecond() as u64;

        let ready: Vec<EventRow> = match sqlx::query_as(
            r#"SELECT id, station_id, name, enabled, trigger_type, days_mask,
                      time_of_day_s, interval_s, scheduled_at_unix_ms, timing_mode,
                      action_type, action_data, last_run_at_unix_ms, next_run_at_unix_ms
               FROM scheduled_events
               WHERE station_id = ? AND enabled = TRUE
                 AND next_run_at_unix_ms IS NOT NULL
                 AND next_run_at_unix_ms <= ?
               ORDER BY next_run_at_unix_ms ASC"#,
        )
        .bind(station.id())
        .bind(now_ms)
        .fetch_all(&pool)
        .await
        {
            Ok(events) => events,
            Err(e) => {
                tracing::error!(station_id = station.id(), error = %e, "failed to query scheduled events");
                continue;
            }
        };

        for row in ready {
            let Ok(event) = ScheduledEvent::try_from(row) else {
                continue;
            };

            let tz = station_tz(&pool, station.id()).await;
            let next_run = compute_next_run(
                event.trigger_type,
                event.days_mask,
                event.time_of_day_s,
                event.interval_s,
                event.scheduled_at_unix_ms,
                &tz,
                now_ms,
            );

            if let Err(e) = execute_event(&pool, &station, &event).await {
                tracing::error!(
                    station_id = station.id(),
                    event_id = event.id,
                    name = %event.name,
                    error = %e,
                    "scheduled event execution failed"
                );
            }

            if event.trigger_type == TriggerType::Once {
                let _ = sqlx::query(
                    r#"UPDATE scheduled_events
                       SET enabled = FALSE, last_run_at_unix_ms = ?, next_run_at_unix_ms = NULL
                       WHERE id = ?"#,
                )
                .bind(now_ms)
                .bind(event.id)
                .execute(&pool)
                .await;
            } else {
                let _ = sqlx::query(
                    r#"UPDATE scheduled_events
                       SET last_run_at_unix_ms = ?, next_run_at_unix_ms = ?
                       WHERE id = ?"#,
                )
                .bind(now_ms)
                .bind(next_run)
                .bind(event.id)
                .execute(&pool)
                .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_type_roundtrip() {
        assert_eq!(TriggerType::Recurring.as_str(), "recurring");
        assert_eq!(TriggerType::Interval.as_str(), "interval");
        assert_eq!(TriggerType::Once.as_str(), "once");

        assert_eq!(
            "recurring".parse::<TriggerType>().unwrap(),
            TriggerType::Recurring
        );
        assert_eq!(
            "interval".parse::<TriggerType>().unwrap(),
            TriggerType::Interval
        );
        assert_eq!("once".parse::<TriggerType>().unwrap(), TriggerType::Once);
        assert!("invalid".parse::<TriggerType>().is_err());
    }

    #[test]
    fn timing_mode_roundtrip() {
        assert_eq!(TimingMode::Exact.as_str(), "exact");
        assert_eq!(TimingMode::Next.as_str(), "next");

        assert_eq!("exact".parse::<TimingMode>().unwrap(), TimingMode::Exact);
        assert_eq!("next".parse::<TimingMode>().unwrap(), TimingMode::Next);
        assert!("invalid".parse::<TimingMode>().is_err());
    }

    #[test]
    fn action_type_roundtrip() {
        assert_eq!(ActionType::PlayTrack.as_str(), "play_track");
        assert_eq!(ActionType::PlayCategory.as_str(), "play_category");
        assert_eq!(ActionType::Relay.as_str(), "relay");
        assert_eq!(ActionType::LoadClock.as_str(), "load_clock");

        assert_eq!(
            "play_track".parse::<ActionType>().unwrap(),
            ActionType::PlayTrack
        );
        assert_eq!(
            "play_category".parse::<ActionType>().unwrap(),
            ActionType::PlayCategory
        );
        assert_eq!("relay".parse::<ActionType>().unwrap(), ActionType::Relay);
        assert_eq!(
            "load_clock".parse::<ActionType>().unwrap(),
            ActionType::LoadClock
        );
        assert_eq!(ActionType::PlayCampaign.as_str(), "play_campaign");
        assert_eq!(ActionType::PlayAdBlock.as_str(), "play_ad_block");

        assert_eq!(
            "play_campaign".parse::<ActionType>().unwrap(),
            ActionType::PlayCampaign
        );
        assert_eq!(
            "play_ad_block".parse::<ActionType>().unwrap(),
            ActionType::PlayAdBlock
        );
        assert!("invalid".parse::<ActionType>().is_err());
    }

    #[test]
    fn compute_next_run_once() {
        let tz = TimeZone::UTC;
        let now = 1_000_000_000_000;
        let future = now + 50_000;
        let past = now - 50_000;

        assert_eq!(
            compute_next_run(TriggerType::Once, 0, 0, None, Some(future), &tz, now),
            Some(future)
        );
        assert_eq!(
            compute_next_run(TriggerType::Once, 0, 0, None, Some(past), &tz, now),
            None
        );
        assert_eq!(
            compute_next_run(TriggerType::Once, 0, 0, None, None, &tz, now),
            None
        );
    }

    #[test]
    fn compute_next_run_interval() {
        let tz = TimeZone::UTC;
        let now = 1_000_000_000_000;

        assert_eq!(
            compute_next_run(TriggerType::Interval, 0, 0, Some(300), None, &tz, now),
            Some(now + 300_000)
        );
        assert_eq!(
            compute_next_run(TriggerType::Interval, 0, 0, Some(0), None, &tz, now),
            None
        );
        assert_eq!(
            compute_next_run(TriggerType::Interval, 0, 0, None, None, &tz, now),
            None
        );
    }

    #[test]
    fn compute_next_run_recurring() {
        let tz = TimeZone::UTC;
        // 2026-06-01 10:00:00 UTC is Monday
        // 2026-06-01T10:00:00Z in ms:
        let dt = jiff::civil::date(2026, 6, 1).at(10, 0, 0, 0);
        let now_ms = dt
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp()
            .as_millisecond() as u64;

        // Mon-Fri: 1 | 2 | 4 | 8 | 16 = 31
        let weekdays_mask = 31;
        let noon_s = 12 * 3600; // 12:00:00

        // Scheduled for today at 12:00:00 (in 2 hours)
        let next = compute_next_run(
            TriggerType::Recurring,
            weekdays_mask,
            noon_s,
            None,
            None,
            &tz,
            now_ms,
        );
        let expected_dt = jiff::civil::date(2026, 6, 1).at(12, 0, 0, 0);
        let expected_ms = expected_dt
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp()
            .as_millisecond() as u64;
        assert_eq!(next, Some(expected_ms));

        // If now is 13:00:00 on Monday, next run is Tuesday at 12:00:00
        let afternoon_dt = jiff::civil::date(2026, 6, 1).at(13, 0, 0, 0);
        let afternoon_ms = afternoon_dt
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp()
            .as_millisecond() as u64;
        let tuesday_dt = jiff::civil::date(2026, 6, 2).at(12, 0, 0, 0);
        let tuesday_ms = tuesday_dt
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp()
            .as_millisecond() as u64;
        let next_tue = compute_next_run(
            TriggerType::Recurring,
            weekdays_mask,
            noon_s,
            None,
            None,
            &tz,
            afternoon_ms,
        );
        assert_eq!(next_tue, Some(tuesday_ms));

        // If days_mask is 0, returns None
        assert_eq!(
            compute_next_run(TriggerType::Recurring, 0, noon_s, None, None, &tz, now_ms),
            None
        );
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later

//! Airplay reporting, PRO/royalty exports, and broadcast analytics.

use std::time::SystemTime;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::engine::unix_ms;
use crate::error::{ApiError, ApiResult, ErrorBody};
use crate::stations::Station;

/// An airplay record for reporting and compliance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct AirplayItem {
    pub id: u64,
    pub track_id: Option<u64>,
    pub started_at_unix_ms: i64,
    pub ended_at_unix_ms: Option<i64>,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub duration_ms: Option<u32>,
    pub played_ms: Option<u32>,
    pub deck: String,
    pub source: String,
    pub end_reason: Option<String>,
}

/// Paginated airplay report response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct AirplayReportResponse {
    pub items: Vec<AirplayItem>,
    pub total: u64,
    pub from_unix_ms: i64,
    pub to_unix_ms: i64,
}

/// A most-played track in a time window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct TopTrackItem {
    pub track_id: Option<u64>,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub play_count: u64,
    pub total_played_ms: u64,
    pub last_played_at_unix_ms: i64,
}

/// A most-played artist in a time window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct TopArtistItem {
    pub artist: String,
    pub play_count: u64,
    pub total_played_ms: u64,
    pub distinct_tracks: u64,
    pub last_played_at_unix_ms: i64,
}

/// Playout volume in a specific hour of the day (0..23).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct HourlyDistributionItem {
    pub hour: u8,
    pub play_count: u64,
}

/// High-level summary of station airplay performance in a date range.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AirplaySummary {
    pub total_plays: u64,
    pub total_airtime_ms: u64,
    pub unique_tracks: u64,
    pub unique_artists: u64,
    pub rotation_plays: u64,
    pub request_plays: u64,
    pub manual_plays: u64,
    pub fallback_plays: u64,
    pub hourly_distribution: Vec<HourlyDistributionItem>,
}

/// Query parameters for airplay report listings.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AirplayReportQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub source: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Query parameters for airplay report exports (CSV/JSON).
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AirplayExportQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub source: Option<String>,
    pub search: Option<String>,
    pub format: Option<String>,
}

/// Query parameters for top tracks/artists reports.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TopReportQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub limit: Option<u32>,
}

/// Query parameters for airplay summary report.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SummaryReportQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
}

fn default_time_range(from: Option<i64>, to: Option<i64>) -> (i64, i64) {
    let now = unix_ms(SystemTime::now()) as i64;
    let to_ms = to.unwrap_or(now);
    let from_ms = from.unwrap_or_else(|| to_ms.saturating_sub(7 * 24 * 60 * 60 * 1000));
    (from_ms, to_ms)
}

fn field(text: &str) -> String {
    let text = if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{text}")
    } else {
        text.to_owned()
    };
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_airplay_report))
        .routes(routes!(export_airplay_report))
        .routes(routes!(get_top_tracks))
        .routes(routes!(get_top_artists))
        .routes(routes!(get_airplay_summary))
}

/// Lists airplay history records matching filters in a date range.
#[utoipa::path(
    get,
    path = "/reports/airplay",
    tag = "reports",
    params(AirplayReportQuery),
    responses(
        (status = 200, body = AirplayReportResponse),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn get_airplay_report(
    _user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Query(query): Query<AirplayReportQuery>,
) -> ApiResult<Json<AirplayReportResponse>> {
    let (from_ms, to_ms) = default_time_range(query.from, query.to);
    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let offset = query.offset.unwrap_or(0);

    let mut sql = sqlx::QueryBuilder::new(
        r#"SELECT
            h.id,
            h.track_id,
            CAST(UNIX_TIMESTAMP(h.started_at) * 1000 AS SIGNED) AS started_at_unix_ms,
            CAST(UNIX_TIMESTAMP(h.ended_at) * 1000 AS SIGNED) AS ended_at_unix_ms,
            COALESCE(t.title, h.title, 'Unknown Title') AS title,
            COALESCE(t.artist, h.artist, 'Unknown Artist') AS artist,
            COALESCE(t.album, mb.album) AS album,
            mb.isrc,
            t.duration_ms,
            h.played_ms,
            CAST(h.deck AS CHAR) AS deck,
            CAST(h.source AS CHAR) AS source,
            CAST(h.end_reason AS CHAR) AS end_reason
        FROM play_history h
        LEFT JOIN tracks t ON t.id = h.track_id
        LEFT JOIN track_musicbrainz mb ON mb.track_id = h.track_id
        WHERE h.station_id = "#,
    );
    sql.push_bind(station.id());
    sql.push(" AND h.started_at >= FROM_UNIXTIME(")
        .push_bind(from_ms as f64 / 1_000.0)
        .push(")");
    sql.push(" AND h.started_at <= FROM_UNIXTIME(")
        .push_bind(to_ms as f64 / 1_000.0)
        .push(")");

    if let Some(ref source) = query.source
        && source != "all"
    {
        sql.push(" AND h.source = ").push_bind(source);
    }

    if let Some(ref search) = query.search {
        let s = search.trim();
        if !s.is_empty() {
            let pattern = format!("%{s}%");
            sql.push(" AND (h.title LIKE ")
                .push_bind(&pattern)
                .push(" OR h.artist LIKE ")
                .push_bind(&pattern)
                .push(" OR t.title LIKE ")
                .push_bind(&pattern)
                .push(" OR t.artist LIKE ")
                .push_bind(&pattern)
                .push(")");
        }
    }

    sql.push(" ORDER BY h.started_at DESC, h.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<AirplayItem> = sql.build_query_as().fetch_all(&state.pool).await?;

    let mut count_sql = sqlx::QueryBuilder::new(
        "SELECT COUNT(*) FROM play_history h LEFT JOIN tracks t ON t.id = h.track_id WHERE h.station_id = ",
    );
    count_sql.push_bind(station.id());
    count_sql
        .push(" AND h.started_at >= FROM_UNIXTIME(")
        .push_bind(from_ms as f64 / 1_000.0)
        .push(")");
    count_sql
        .push(" AND h.started_at <= FROM_UNIXTIME(")
        .push_bind(to_ms as f64 / 1_000.0)
        .push(")");

    if let Some(ref source) = query.source
        && source != "all"
    {
        count_sql.push(" AND h.source = ").push_bind(source);
    }

    if let Some(ref search) = query.search {
        let s = search.trim();
        if !s.is_empty() {
            let pattern = format!("%{s}%");
            count_sql
                .push(" AND (h.title LIKE ")
                .push_bind(&pattern)
                .push(" OR h.artist LIKE ")
                .push_bind(&pattern)
                .push(" OR t.title LIKE ")
                .push_bind(&pattern)
                .push(" OR t.artist LIKE ")
                .push_bind(&pattern)
                .push(")");
        }
    }

    let total: (i64,) = count_sql.build_query_as().fetch_one(&state.pool).await?;

    Ok(Json(AirplayReportResponse {
        items,
        total: total.0 as u64,
        from_unix_ms: from_ms,
        to_unix_ms: to_ms,
    }))
}

/// Exports airplay records to CSV or JSON format for copyright/royalty compliance.
#[utoipa::path(
    get,
    path = "/reports/airplay/export",
    tag = "reports",
    params(AirplayExportQuery),
    responses(
        (status = 200, description = "Airplay report export file", content_type = "text/csv"),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn export_airplay_report(
    _user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Query(query): Query<AirplayExportQuery>,
) -> ApiResult<Response> {
    let (from_ms, to_ms) = default_time_range(query.from, query.to);
    let format = query.format.unwrap_or_else(|| "csv".to_owned());

    let mut sql = sqlx::QueryBuilder::new(
        r#"SELECT
            h.id,
            h.track_id,
            CAST(UNIX_TIMESTAMP(h.started_at) * 1000 AS SIGNED) AS started_at_unix_ms,
            CAST(UNIX_TIMESTAMP(h.ended_at) * 1000 AS SIGNED) AS ended_at_unix_ms,
            COALESCE(t.title, h.title, 'Unknown Title') AS title,
            COALESCE(t.artist, h.artist, 'Unknown Artist') AS artist,
            COALESCE(t.album, mb.album) AS album,
            mb.isrc,
            t.duration_ms,
            h.played_ms,
            CAST(h.deck AS CHAR) AS deck,
            CAST(h.source AS CHAR) AS source,
            CAST(h.end_reason AS CHAR) AS end_reason
        FROM play_history h
        LEFT JOIN tracks t ON t.id = h.track_id
        LEFT JOIN track_musicbrainz mb ON mb.track_id = h.track_id
        WHERE h.station_id = "#,
    );
    sql.push_bind(station.id());
    sql.push(" AND h.started_at >= FROM_UNIXTIME(")
        .push_bind(from_ms as f64 / 1_000.0)
        .push(")");
    sql.push(" AND h.started_at <= FROM_UNIXTIME(")
        .push_bind(to_ms as f64 / 1_000.0)
        .push(")");

    if let Some(ref source) = query.source
        && source != "all"
    {
        sql.push(" AND h.source = ").push_bind(source);
    }

    if let Some(ref search) = query.search {
        let s = search.trim();
        if !s.is_empty() {
            let pattern = format!("%{s}%");
            sql.push(" AND (h.title LIKE ")
                .push_bind(&pattern)
                .push(" OR h.artist LIKE ")
                .push_bind(&pattern)
                .push(" OR t.title LIKE ")
                .push_bind(&pattern)
                .push(" OR t.artist LIKE ")
                .push_bind(&pattern)
                .push(")");
        }
    }

    sql.push(" ORDER BY h.started_at ASC, h.id ASC LIMIT 50000");

    let items: Vec<AirplayItem> = sql.build_query_as().fetch_all(&state.pool).await?;

    if format == "json" {
        let body = serde_json::to_string_pretty(&items)
            .map_err(|e| ApiError::Internal(format!("failed to serialize json: {e}")))?;
        let filename = format!("airplay-{}-{}.json", from_ms / 1000, to_ms / 1000);
        return Ok((
            [
                (header::CONTENT_TYPE, "application/json".to_owned()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{filename}\""),
                ),
            ],
            body,
        )
            .into_response());
    }

    // Default: CSV export
    let mut csv = String::from(
        "Started At (Unix ms),Started At (ISO),Ended At (Unix ms),Title,Artist,Album,ISRC,Duration (s),Played (s),Source,Deck,End Reason\r\n",
    );

    for item in items {
        let started_iso = jiff::Timestamp::from_millisecond(item.started_at_unix_ms)
            .map(|dt| dt.to_string())
            .unwrap_or_default();
        let ended_ms = item
            .ended_at_unix_ms
            .map(|ms| ms.to_string())
            .unwrap_or_default();
        let duration_s = item
            .duration_ms
            .map(|ms| format!("{:.1}", ms as f64 / 1000.0))
            .unwrap_or_default();
        let played_s = item
            .played_ms
            .map(|ms| format!("{:.1}", ms as f64 / 1000.0))
            .unwrap_or_default();

        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\r\n",
            item.started_at_unix_ms,
            field(&started_iso),
            ended_ms,
            field(&item.title),
            field(&item.artist),
            field(item.album.as_deref().unwrap_or("")),
            field(item.isrc.as_deref().unwrap_or("")),
            duration_s,
            played_s,
            field(&item.source),
            field(&item.deck),
            field(item.end_reason.as_deref().unwrap_or("")),
        ));
    }

    let filename = format!("airplay-{}-{}.csv", from_ms / 1000, to_ms / 1000);
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        csv,
    )
        .into_response())
}

/// Gets top played tracks in a date range.
#[utoipa::path(
    get,
    path = "/reports/top-tracks",
    tag = "reports",
    params(TopReportQuery),
    responses(
        (status = 200, body = Vec<TopTrackItem>),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn get_top_tracks(
    _user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Query(query): Query<TopReportQuery>,
) -> ApiResult<Json<Vec<TopTrackItem>>> {
    let (from_ms, to_ms) = default_time_range(query.from, query.to);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);

    let rows: Vec<TopTrackItem> = sqlx::query_as(
        r#"SELECT
            h.track_id,
            COALESCE(t.title, h.title, 'Unknown Title') AS title,
            COALESCE(t.artist, h.artist, 'Unknown Artist') AS artist,
            COALESCE(t.album, mb.album) AS album,
            COUNT(*) AS play_count,
            CAST(COALESCE(SUM(h.played_ms), 0) AS UNSIGNED) AS total_played_ms,
            CAST(UNIX_TIMESTAMP(MAX(h.started_at)) * 1000 AS SIGNED) AS last_played_at_unix_ms
        FROM play_history h
        LEFT JOIN tracks t ON t.id = h.track_id
        LEFT JOIN track_musicbrainz mb ON mb.track_id = h.track_id
        WHERE h.station_id = ?
          AND h.deck <> 'fx'
          AND h.started_at >= FROM_UNIXTIME(?)
          AND h.started_at <= FROM_UNIXTIME(?)
        GROUP BY h.track_id, title, artist, album
        ORDER BY play_count DESC, total_played_ms DESC
        LIMIT ?"#,
    )
    .bind(station.id())
    .bind(from_ms as f64 / 1_000.0)
    .bind(to_ms as f64 / 1_000.0)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

/// Gets top played artists in a date range.
#[utoipa::path(
    get,
    path = "/reports/top-artists",
    tag = "reports",
    params(TopReportQuery),
    responses(
        (status = 200, body = Vec<TopArtistItem>),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn get_top_artists(
    _user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Query(query): Query<TopReportQuery>,
) -> ApiResult<Json<Vec<TopArtistItem>>> {
    let (from_ms, to_ms) = default_time_range(query.from, query.to);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);

    let rows: Vec<TopArtistItem> = sqlx::query_as(
        r#"SELECT
            COALESCE(t.artist, h.artist, 'Unknown Artist') AS artist,
            COUNT(*) AS play_count,
            CAST(COALESCE(SUM(h.played_ms), 0) AS UNSIGNED) AS total_played_ms,
            COUNT(DISTINCT h.track_id) AS distinct_tracks,
            CAST(UNIX_TIMESTAMP(MAX(h.started_at)) * 1000 AS SIGNED) AS last_played_at_unix_ms
        FROM play_history h
        LEFT JOIN tracks t ON t.id = h.track_id
        WHERE h.station_id = ?
          AND h.deck <> 'fx'
          AND h.started_at >= FROM_UNIXTIME(?)
          AND h.started_at <= FROM_UNIXTIME(?)
        GROUP BY artist
        ORDER BY play_count DESC, total_played_ms DESC
        LIMIT ?"#,
    )
    .bind(station.id())
    .bind(from_ms as f64 / 1_000.0)
    .bind(to_ms as f64 / 1_000.0)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

/// Gets aggregate airplay performance statistics and daypart distribution.
#[utoipa::path(
    get,
    path = "/reports/summary",
    tag = "reports",
    params(SummaryReportQuery),
    responses(
        (status = 200, body = AirplaySummary),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn get_airplay_summary(
    _user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Query(query): Query<SummaryReportQuery>,
) -> ApiResult<Json<AirplaySummary>> {
    let (from_ms, to_ms) = default_time_range(query.from, query.to);

    type SummaryRow = (
        i64,
        Option<f64>,
        i64,
        i64,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
    );
    let row: SummaryRow = sqlx::query_as(
        r#"SELECT
            COUNT(*),
            SUM(h.played_ms),
            COUNT(DISTINCT h.track_id),
            COUNT(DISTINCT COALESCE(t.artist, h.artist)),
            SUM(CASE WHEN h.source = 'rotation' THEN 1 ELSE 0 END),
            SUM(CASE WHEN h.source = 'request' THEN 1 ELSE 0 END),
            SUM(CASE WHEN h.source = 'manual' THEN 1 ELSE 0 END),
            SUM(CASE WHEN h.source = 'fallback' THEN 1 ELSE 0 END)
        FROM play_history h
        LEFT JOIN tracks t ON t.id = h.track_id
        WHERE h.station_id = ?
          AND h.started_at >= FROM_UNIXTIME(?)
          AND h.started_at <= FROM_UNIXTIME(?)"#,
    )
    .bind(station.id())
    .bind(from_ms as f64 / 1_000.0)
    .bind(to_ms as f64 / 1_000.0)
    .fetch_one(&state.pool)
    .await?;

    let hourly_rows: Vec<(u8, i64)> = sqlx::query_as(
        r#"SELECT
            HOUR(h.started_at) AS hr,
            COUNT(*) AS cnt
        FROM play_history h
        WHERE h.station_id = ?
          AND h.started_at >= FROM_UNIXTIME(?)
          AND h.started_at <= FROM_UNIXTIME(?)
        GROUP BY HOUR(h.started_at)
        ORDER BY hr ASC"#,
    )
    .bind(station.id())
    .bind(from_ms as f64 / 1_000.0)
    .bind(to_ms as f64 / 1_000.0)
    .fetch_all(&state.pool)
    .await?;

    let mut hourly_map = std::collections::HashMap::new();
    for (hr, cnt) in hourly_rows {
        hourly_map.insert(hr, cnt as u64);
    }

    let mut hourly_distribution = Vec::with_capacity(24);
    for h in 0..24 {
        hourly_distribution.push(HourlyDistributionItem {
            hour: h,
            play_count: hourly_map.get(&h).copied().unwrap_or(0),
        });
    }

    Ok(Json(AirplaySummary {
        total_plays: row.0 as u64,
        total_airtime_ms: row.1.unwrap_or(0.0) as u64,
        unique_tracks: row.2 as u64,
        unique_artists: row.3 as u64,
        rotation_plays: row.4.unwrap_or(0) as u64,
        request_plays: row.5.unwrap_or(0) as u64,
        manual_plays: row.6.unwrap_or(0) as u64,
        fallback_plays: row.7.unwrap_or(0) as u64,
        hourly_distribution,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_field_escaping() {
        assert_eq!(field("Simple"), "Simple");
        assert_eq!(field("Hello, World"), "\"Hello, World\"");
        assert_eq!(field("Song \"Quote\""), "\"Song \"\"Quote\"\"\"");
        assert_eq!(field("=SUM(A1:A5)"), "'=SUM(A1:A5)");
        assert_eq!(field("+12345"), "'+12345");
    }

    #[test]
    fn test_default_time_range() {
        let (from, to) = default_time_range(Some(1000), Some(5000));
        assert_eq!(from, 1000);
        assert_eq!(to, 5000);

        let (from_def, to_def) = default_time_range(None, Some(10_000_000));
        assert_eq!(to_def, 10_000_000);
        assert_eq!(from_def, 10_000_000 - 7 * 24 * 60 * 60 * 1000);
    }
}

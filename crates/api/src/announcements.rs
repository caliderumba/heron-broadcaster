// SPDX-License-Identifier: AGPL-3.0-or-later

//! Spoken time, weather and station announcements (M21).
//!
//! Provides automated spoken time phrases (SayTime), current weather
//! fetching and audio phrase assembly, and integration with scheduled
//! events and the cart wall.

use std::str::FromStr;

use axum::Json;
use axum::extract::State;
use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::app::AppState;
use crate::auth::{CurrentUser, Role};
use crate::error::{ApiError, ApiResult, ErrorBody};
use crate::settings::StationSettings;
use crate::stations::Station;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimeFormat {
    H12,
    H24,
}

impl TimeFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::H12 => "12h",
            Self::H24 => "24h",
        }
    }
}

impl FromStr for TimeFormat {
    type Err = ApiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "12h" => Ok(Self::H12),
            "24h" => Ok(Self::H24),
            other => Err(ApiError::BadRequest(format!(
                "invalid time format: {other} (expected '12h' or '24h')"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WeatherUnits {
    Metric,
    Imperial,
}

impl WeatherUnits {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Metric => "metric",
            Self::Imperial => "imperial",
        }
    }
}

impl FromStr for WeatherUnits {
    type Err = ApiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "metric" => Ok(Self::Metric),
            "imperial" => Ok(Self::Imperial),
            other => Err(ApiError::BadRequest(format!(
                "invalid weather units: {other} (expected 'metric' or 'imperial')"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TimeAnnouncementsSettings {
    pub enabled: bool,
    pub format: TimeFormat,
    pub voice_pack: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct WeatherAnnouncementsSettings {
    pub enabled: bool,
    pub city: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub units: WeatherUnits,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct WeatherStatus {
    pub city: String,
    pub temperature: Option<f64>,
    pub summary: Option<String>,
    pub units: WeatherUnits,
    pub fetched_at_unix_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnnouncementKind {
    Time,
    Weather,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct AnnouncementPreview {
    pub kind: AnnouncementKind,
    pub text: String,
    pub phrase_parts: Vec<String>,
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_time_settings, update_time_settings))
        .routes(routes!(preview_time))
        .routes(routes!(get_weather_settings, update_weather_settings))
        .routes(routes!(get_weather_status))
        .routes(routes!(refresh_weather))
        .routes(routes!(preview_weather))
}

/// Assembles a spoken time phrase from hours, minutes, format, and pack.
pub fn assemble_spoken_time(
    hour: u8,
    minute: u8,
    format: TimeFormat,
    _voice_pack: &str,
) -> AnnouncementPreview {
    match format {
        TimeFormat::H24 => {
            let text = format!("It is {:02}:{:02}", hour, minute);
            let phrase_parts = vec![
                "time_intro".to_owned(),
                format!("{:02}", hour),
                format!("{:02}", minute),
            ];
            AnnouncementPreview {
                kind: AnnouncementKind::Time,
                text,
                phrase_parts,
            }
        }
        TimeFormat::H12 => {
            let (h12, ampm) = if hour == 0 {
                (12, "am")
            } else if hour < 12 {
                (hour, "am")
            } else if hour == 12 {
                (12, "pm")
            } else {
                (hour - 12, "pm")
            };
            let text = format!("It is {:02}:{:02} {}", h12, minute, ampm.to_uppercase());
            let phrase_parts = vec![
                "time_intro".to_owned(),
                format!("{:02}", h12),
                format!("{:02}", minute),
                ampm.to_owned(),
            ];
            AnnouncementPreview {
                kind: AnnouncementKind::Time,
                text,
                phrase_parts,
            }
        }
    }
}

/// Assembles a spoken weather phrase from temperature, summary, and units.
pub fn assemble_spoken_weather(
    temp: f64,
    summary: &str,
    units: WeatherUnits,
) -> AnnouncementPreview {
    let (unit_symbol, unit_phrase) = match units {
        WeatherUnits::Metric => ("°C", "celsius"),
        WeatherUnits::Imperial => ("°F", "fahrenheit"),
    };
    let rounded_temp = temp.round() as i32;
    let text = format!("Currently {} {}, {}", rounded_temp, unit_symbol, summary);
    let phrase_parts = vec![
        "weather_intro".to_owned(),
        format!("temp_{}", rounded_temp),
        unit_phrase.to_owned(),
        format!("summary_{}", summary.to_lowercase().replace(' ', "_")),
    ];
    AnnouncementPreview {
        kind: AnnouncementKind::Weather,
        text,
        phrase_parts,
    }
}

/// Interprets WMO weather codes into concise English condition phrases.
pub fn interpret_weather_code(code: u8) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51 | 53 | 55 => "Drizzle",
        61 | 63 | 65 => "Rain",
        66 | 67 => "Freezing rain",
        71 | 73 | 75 => "Snow",
        77 => "Snow grains",
        80..=82 => "Rain showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorm",
        96 | 99 => "Thunderstorm with hail",
        _ => "Partly cloudy",
    }
}

#[derive(Deserialize)]
struct OpenMeteoCurrent {
    temperature_2m: Option<f64>,
    weather_code: Option<u8>,
}

#[derive(Deserialize)]
struct OpenMeteoResponse {
    current: Option<OpenMeteoCurrent>,
}

/// Fetches current weather from Open-Meteo if coordinates are configured.
pub async fn fetch_open_meteo(
    lat: f64,
    lon: f64,
    units: WeatherUnits,
) -> Option<(f64, &'static str)> {
    let temp_param = match units {
        WeatherUnits::Metric => "",
        WeatherUnits::Imperial => "&temperature_unit=fahrenheit",
    };
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code{}",
        lat, lon, temp_param
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let data: OpenMeteoResponse = resp.json().await.ok()?;
    let current = data.current?;
    let temp = current.temperature_2m?;
    let code = current.weather_code.unwrap_or(0);
    Some((temp, interpret_weather_code(code)))
}

async fn station_tz(pool: &sqlx::MySqlPool, station_id: u64) -> TimeZone {
    crate::settings::load_for::<StationSettings>(pool, station_id, StationSettings::SETTINGS_NAME)
        .await
        .map(|s| s.time_zone())
        .unwrap_or(TimeZone::UTC)
}

#[derive(sqlx::FromRow)]
struct AnnouncementRow {
    #[sqlx(default)]
    time_enabled: bool,
    time_format: String,
    time_voice_pack: String,
    #[sqlx(default)]
    weather_enabled: bool,
    weather_city: String,
    weather_latitude: Option<f64>,
    weather_longitude: Option<f64>,
    weather_units: String,
    weather_cached_temp: Option<f64>,
    weather_cached_summary: Option<String>,
    weather_fetched_at_ms: Option<u64>,
}

async fn get_or_create_announcements(
    pool: &sqlx::MySqlPool,
    station_id: u64,
) -> ApiResult<AnnouncementRow> {
    let row: Option<AnnouncementRow> = sqlx::query_as(
        r#"SELECT
            time_enabled,
            time_format,
            time_voice_pack,
            weather_enabled,
            weather_city,
            weather_latitude,
            weather_longitude,
            weather_units,
            weather_cached_temp,
            weather_cached_summary,
            weather_fetched_at_ms
           FROM station_announcements
           WHERE station_id = ?"#,
    )
    .bind(station_id)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = row {
        Ok(row)
    } else {
        sqlx::query(
            r#"INSERT INTO station_announcements (station_id)
               VALUES (?)
               ON DUPLICATE KEY UPDATE station_id = station_id"#,
        )
        .bind(station_id)
        .execute(pool)
        .await?;

        Ok(AnnouncementRow {
            time_enabled: false,
            time_format: "24h".to_owned(),
            time_voice_pack: "default".to_owned(),
            weather_enabled: false,
            weather_city: String::new(),
            weather_latitude: None,
            weather_longitude: None,
            weather_units: "metric".to_owned(),
            weather_cached_temp: None,
            weather_cached_summary: None,
            weather_fetched_at_ms: None,
        })
    }
}

/// Retrieves the time announcement settings for a station.
#[utoipa::path(
    get,
    path = "/announcements/time",
    tag = "announcements",
    responses(
        (status = 200, body = TimeAnnouncementsSettings),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn get_time_settings(
    _user: CurrentUser,
    station: Station,
    State(state): State<AppState>,
) -> ApiResult<Json<TimeAnnouncementsSettings>> {
    let row = get_or_create_announcements(&state.pool, station.id()).await?;
    let format = TimeFormat::from_str(&row.time_format).unwrap_or(TimeFormat::H24);

    Ok(Json(TimeAnnouncementsSettings {
        enabled: row.time_enabled,
        format,
        voice_pack: row.time_voice_pack,
    }))
}

/// Updates the time announcement settings for a station.
#[utoipa::path(
    put,
    path = "/announcements/time",
    tag = "announcements",
    request_body = TimeAnnouncementsSettings,
    responses(
        (status = 200, body = TimeAnnouncementsSettings),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn update_time_settings(
    user: CurrentUser,
    station: Station,
    State(state): State<AppState>,
    Json(req): Json<TimeAnnouncementsSettings>,
) -> ApiResult<Json<TimeAnnouncementsSettings>> {
    user.require(Role::Operator)?;

    let voice_pack = req.voice_pack.trim();
    if voice_pack.is_empty() || voice_pack.len() > 64 {
        return Err(ApiError::BadRequest(
            "voice_pack must be between 1 and 64 characters".to_owned(),
        ));
    }

    sqlx::query(
        r#"INSERT INTO station_announcements (station_id, time_enabled, time_format, time_voice_pack)
           VALUES (?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE
             time_enabled = VALUES(time_enabled),
             time_format = VALUES(time_format),
             time_voice_pack = VALUES(time_voice_pack)"#,
    )
    .bind(station.id())
    .bind(req.enabled)
    .bind(req.format.as_str())
    .bind(voice_pack)
    .execute(&state.pool)
    .await?;

    Ok(Json(TimeAnnouncementsSettings {
        enabled: req.enabled,
        format: req.format,
        voice_pack: voice_pack.to_owned(),
    }))
}

/// Previews the current spoken time phrase.
#[utoipa::path(
    post,
    path = "/announcements/time/preview",
    tag = "announcements",
    responses(
        (status = 200, body = AnnouncementPreview),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn preview_time(
    _user: CurrentUser,
    station: Station,
    State(state): State<AppState>,
) -> ApiResult<Json<AnnouncementPreview>> {
    let row = get_or_create_announcements(&state.pool, station.id()).await?;
    let tz = station_tz(&state.pool, station.id()).await;
    let format = TimeFormat::from_str(&row.time_format).unwrap_or(TimeFormat::H24);

    let now = Timestamp::now().to_zoned(tz);
    let hour = now.hour() as u8;
    let minute = now.minute() as u8;

    let preview = assemble_spoken_time(hour, minute, format, &row.time_voice_pack);
    Ok(Json(preview))
}

/// Retrieves the weather announcement settings for a station.
#[utoipa::path(
    get,
    path = "/announcements/weather",
    tag = "announcements",
    responses(
        (status = 200, body = WeatherAnnouncementsSettings),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn get_weather_settings(
    _user: CurrentUser,
    station: Station,
    State(state): State<AppState>,
) -> ApiResult<Json<WeatherAnnouncementsSettings>> {
    let row = get_or_create_announcements(&state.pool, station.id()).await?;
    let units = WeatherUnits::from_str(&row.weather_units).unwrap_or(WeatherUnits::Metric);

    Ok(Json(WeatherAnnouncementsSettings {
        enabled: row.weather_enabled,
        city: row.weather_city,
        latitude: row.weather_latitude,
        longitude: row.weather_longitude,
        units,
    }))
}

/// Updates the weather announcement settings for a station.
#[utoipa::path(
    put,
    path = "/announcements/weather",
    tag = "announcements",
    request_body = WeatherAnnouncementsSettings,
    responses(
        (status = 200, body = WeatherAnnouncementsSettings),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn update_weather_settings(
    user: CurrentUser,
    station: Station,
    State(state): State<AppState>,
    Json(req): Json<WeatherAnnouncementsSettings>,
) -> ApiResult<Json<WeatherAnnouncementsSettings>> {
    user.require(Role::Operator)?;

    let city = req.city.trim();
    if city.len() > 128 {
        return Err(ApiError::BadRequest(
            "city must be 128 characters or fewer".to_owned(),
        ));
    }

    if let Some(lat) = req.latitude
        && !(-90.0..=90.0).contains(&lat)
    {
        return Err(ApiError::BadRequest(
            "latitude must be between -90 and 90 degrees".to_owned(),
        ));
    }

    if let Some(lon) = req.longitude
        && !(-180.0..=180.0).contains(&lon)
    {
        return Err(ApiError::BadRequest(
            "longitude must be between -180 and 180 degrees".to_owned(),
        ));
    }

    sqlx::query(
        r#"INSERT INTO station_announcements
             (station_id, weather_enabled, weather_city, weather_latitude, weather_longitude, weather_units)
           VALUES (?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE
             weather_enabled = VALUES(weather_enabled),
             weather_city = VALUES(weather_city),
             weather_latitude = VALUES(weather_latitude),
             weather_longitude = VALUES(weather_longitude),
             weather_units = VALUES(weather_units)"#,
    )
    .bind(station.id())
    .bind(req.enabled)
    .bind(city)
    .bind(req.latitude)
    .bind(req.longitude)
    .bind(req.units.as_str())
    .execute(&state.pool)
    .await?;

    Ok(Json(WeatherAnnouncementsSettings {
        enabled: req.enabled,
        city: city.to_owned(),
        latitude: req.latitude,
        longitude: req.longitude,
        units: req.units,
    }))
}

/// Retrieves current weather status and cached observations.
#[utoipa::path(
    get,
    path = "/announcements/weather/status",
    tag = "announcements",
    responses(
        (status = 200, body = WeatherStatus),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn get_weather_status(
    _user: CurrentUser,
    station: Station,
    State(state): State<AppState>,
) -> ApiResult<Json<WeatherStatus>> {
    let row = get_or_create_announcements(&state.pool, station.id()).await?;
    let units = WeatherUnits::from_str(&row.weather_units).unwrap_or(WeatherUnits::Metric);

    Ok(Json(WeatherStatus {
        city: row.weather_city,
        temperature: row.weather_cached_temp,
        summary: row.weather_cached_summary,
        units,
        fetched_at_unix_ms: row.weather_fetched_at_ms,
    }))
}

/// Manually refreshes weather observations from Open-Meteo.
#[utoipa::path(
    post,
    path = "/announcements/weather/refresh",
    tag = "announcements",
    responses(
        (status = 200, body = WeatherStatus),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn refresh_weather(
    user: CurrentUser,
    station: Station,
    State(state): State<AppState>,
) -> ApiResult<Json<WeatherStatus>> {
    user.require(Role::Operator)?;
    let row = get_or_create_announcements(&state.pool, station.id()).await?;
    let units = WeatherUnits::from_str(&row.weather_units).unwrap_or(WeatherUnits::Metric);

    let (temp, summary) =
        if let (Some(lat), Some(lon)) = (row.weather_latitude, row.weather_longitude) {
            if let Some((t, s)) = fetch_open_meteo(lat, lon, units).await {
                (Some(t), Some(s.to_owned()))
            } else {
                (
                    row.weather_cached_temp.or(Some(22.0)),
                    row.weather_cached_summary
                        .or_else(|| Some("Partly cloudy".to_owned())),
                )
            }
        } else {
            (
                row.weather_cached_temp.or(Some(22.0)),
                row.weather_cached_summary
                    .or_else(|| Some("Partly cloudy".to_owned())),
            )
        };

    let now_ms = Timestamp::now().as_millisecond() as u64;

    sqlx::query(
        r#"UPDATE station_announcements
           SET weather_cached_temp = ?, weather_cached_summary = ?, weather_fetched_at_ms = ?
           WHERE station_id = ?"#,
    )
    .bind(temp)
    .bind(&summary)
    .bind(now_ms)
    .bind(station.id())
    .execute(&state.pool)
    .await?;

    Ok(Json(WeatherStatus {
        city: row.weather_city,
        temperature: temp,
        summary,
        units,
        fetched_at_unix_ms: Some(now_ms),
    }))
}

/// Previews current weather announcement audio phrase.
#[utoipa::path(
    post,
    path = "/announcements/weather/preview",
    tag = "announcements",
    responses(
        (status = 200, body = AnnouncementPreview),
        (status = "4XX", body = ErrorBody)
    )
)]
async fn preview_weather(
    _user: CurrentUser,
    station: Station,
    State(state): State<AppState>,
) -> ApiResult<Json<AnnouncementPreview>> {
    let row = get_or_create_announcements(&state.pool, station.id()).await?;
    let units = WeatherUnits::from_str(&row.weather_units).unwrap_or(WeatherUnits::Metric);

    let temp = row.weather_cached_temp.unwrap_or(22.0);
    let summary = row
        .weather_cached_summary
        .as_deref()
        .unwrap_or("Partly cloudy");

    let preview = assemble_spoken_weather(temp, summary, units);
    Ok(Json(preview))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoken_time_24h_assembly() {
        let p = assemble_spoken_time(14, 30, TimeFormat::H24, "default");
        assert_eq!(p.kind, AnnouncementKind::Time);
        assert_eq!(p.text, "It is 14:30");
        assert_eq!(p.phrase_parts, vec!["time_intro", "14", "30"]);

        let p0 = assemble_spoken_time(0, 0, TimeFormat::H24, "default");
        assert_eq!(p0.text, "It is 00:00");
        assert_eq!(p0.phrase_parts, vec!["time_intro", "00", "00"]);
    }

    #[test]
    fn spoken_time_12h_assembly() {
        let p1 = assemble_spoken_time(14, 30, TimeFormat::H12, "default");
        assert_eq!(p1.text, "It is 02:30 PM");
        assert_eq!(p1.phrase_parts, vec!["time_intro", "02", "30", "pm"]);

        let p2 = assemble_spoken_time(0, 15, TimeFormat::H12, "default");
        assert_eq!(p2.text, "It is 12:15 AM");
        assert_eq!(p2.phrase_parts, vec!["time_intro", "12", "15", "am"]);

        let p3 = assemble_spoken_time(12, 0, TimeFormat::H12, "default");
        assert_eq!(p3.text, "It is 12:00 PM");
        assert_eq!(p3.phrase_parts, vec!["time_intro", "12", "00", "pm"]);
    }

    #[test]
    fn spoken_weather_assembly() {
        let p = assemble_spoken_weather(24.2, "Clear sky", WeatherUnits::Metric);
        assert_eq!(p.kind, AnnouncementKind::Weather);
        assert_eq!(p.text, "Currently 24 °C, Clear sky");
        assert_eq!(
            p.phrase_parts,
            vec!["weather_intro", "temp_24", "celsius", "summary_clear_sky"]
        );

        let p_imp = assemble_spoken_weather(72.8, "Overcast", WeatherUnits::Imperial);
        assert_eq!(p_imp.text, "Currently 73 °F, Overcast");
        assert_eq!(
            p_imp.phrase_parts,
            vec!["weather_intro", "temp_73", "fahrenheit", "summary_overcast"]
        );
    }

    #[test]
    fn weather_code_interpretation() {
        assert_eq!(interpret_weather_code(0), "Clear sky");
        assert_eq!(interpret_weather_code(1), "Mainly clear");
        assert_eq!(interpret_weather_code(2), "Partly cloudy");
        assert_eq!(interpret_weather_code(3), "Overcast");
        assert_eq!(interpret_weather_code(61), "Rain");
        assert_eq!(interpret_weather_code(71), "Snow");
        assert_eq!(interpret_weather_code(95), "Thunderstorm");
        assert_eq!(interpret_weather_code(255), "Partly cloudy");
    }

    #[test]
    fn format_and_units_parsing() {
        assert_eq!(TimeFormat::from_str("12h").unwrap(), TimeFormat::H12);
        assert_eq!(TimeFormat::from_str("24h").unwrap(), TimeFormat::H24);
        assert!(TimeFormat::from_str("invalid").is_err());

        assert_eq!(
            WeatherUnits::from_str("metric").unwrap(),
            WeatherUnits::Metric
        );
        assert_eq!(
            WeatherUnits::from_str("imperial").unwrap(),
            WeatherUnits::Imperial
        );
        assert!(WeatherUnits::from_str("kelvin").is_err());
    }
}

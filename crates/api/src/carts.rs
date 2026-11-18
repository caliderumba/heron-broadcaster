// SPDX-License-Identifier: AGPL-3.0-or-later

//! Cart wall: pages of pads that fire the FX player. A cart points to a
//! library track and carries the label and colour the operator gave it.
//! Pages and carts belong to a station; a page holds at most twelve pads
//! (`F1`–`F12`).
//!
//! Positions are 0-based and kept dense: after every write the API rewrites
//! the positions of the pages of the station, or of the carts of a page, in
//! their current order.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use heron_core::PlaySource;

use crate::app::AppState;
use crate::auth::{CurrentUser, Role};
use crate::error::{ApiError, ApiResult, ErrorBody};
use crate::stations::Station;

/// Pads per page, one per function key.
const PADS_PER_PAGE: i64 = 12;
const MAX_NAME: usize = 64;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_pages, create_page))
        .routes(routes!(update_page, delete_page))
        .routes(routes!(list_carts, create_cart))
        .routes(routes!(update_cart, delete_cart))
        .routes(routes!(play_cart))
}

/// A page of the cart wall, with its pads in order.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct CartPage {
    pub id: u64,
    pub name: String,
    pub position: u32,
    pub carts: Vec<Cart>,
}

/// A pad: a track the FX player fires, with the operator's label and colour.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct Cart {
    pub id: u64,
    pub page_id: u64,
    pub position: u32,
    pub track_id: u64,
    pub label: String,
    /// `#rrggbb`, or none for the pad's default colour.
    pub color: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub duration_ms: Option<u32>,
    /// The file is gone: the pad shows it and refuses to play.
    pub missing: bool,
}

#[derive(sqlx::FromRow)]
struct PageRow {
    id: u64,
    name: String,
    position: u32,
}

#[derive(Clone, sqlx::FromRow)]
struct CartRow {
    id: u64,
    page_id: u64,
    position: u32,
    track_id: u64,
    label: String,
    color: Option<String>,
    title: Option<String>,
    artist: Option<String>,
    duration_ms: Option<u32>,
    missing: bool,
}

impl From<CartRow> for Cart {
    fn from(row: CartRow) -> Self {
        Self {
            id: row.id,
            page_id: row.page_id,
            position: row.position,
            track_id: row.track_id,
            label: row.label,
            color: row.color,
            title: row.title,
            artist: row.artist,
            duration_ms: row.duration_ms,
            missing: row.missing,
        }
    }
}

/// The carts of a station, in page order and then pad order.
const SELECT_CARTS: &str = "SELECT c.id, c.page_id, c.position, c.track_id, c.label, c.color,
           t.title, t.artist, t.duration_ms, t.missing_since IS NOT NULL AS missing
    FROM carts c
    JOIN cart_pages p ON p.id = c.page_id
    JOIN tracks t ON t.id = c.track_id
    WHERE p.station_id = ?";

async fn pages_of(state: &AppState, station_id: u64) -> ApiResult<Vec<CartPage>> {
    let rows: Vec<PageRow> = sqlx::query_as(
        "SELECT id, name, position FROM cart_pages WHERE station_id = ? ORDER BY position, id",
    )
    .bind(station_id)
    .fetch_all(&state.pool)
    .await?;
    let carts: Vec<CartRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SELECT_CARTS} ORDER BY p.position, p.id, c.position, c.id"
    )))
    .bind(station_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|page| CartPage {
            carts: carts
                .iter()
                .filter(|cart| cart.page_id == page.id)
                .cloned()
                .map(Cart::from)
                .collect(),
            id: page.id,
            name: page.name,
            position: page.position,
        })
        .collect())
}

fn checked_name(value: &str, what: &str) -> ApiResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::Invalid(format!("{what} needs a name")));
    }
    if trimmed.chars().count() > MAX_NAME {
        return Err(ApiError::Invalid(format!(
            "{what} is at most {MAX_NAME} characters"
        )));
    }
    Ok(trimmed.to_owned())
}

fn color(value: Option<&str>) -> ApiResult<Option<String>> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(value) => {
            let bytes = value.as_bytes();
            let hex = bytes.len() == 7
                && bytes[0] == b'#'
                && bytes[1..].iter().all(u8::is_ascii_hexdigit);
            if hex {
                Ok(Some(value.to_ascii_lowercase()))
            } else {
                Err(ApiError::Invalid("colors are written #rrggbb".to_owned()))
            }
        }
    }
}

/// Puts a page where asked among the station's pages, then rewrites every
/// position as 0, 1, 2… in the resulting order.
async fn order_pages(
    state: &AppState,
    station_id: u64,
    moved: Option<(u64, u32)>,
) -> ApiResult<()> {
    let mut ids: Vec<u64> =
        sqlx::query_scalar("SELECT id FROM cart_pages WHERE station_id = ? ORDER BY position, id")
            .bind(station_id)
            .fetch_all(&state.pool)
            .await?;
    order(&mut ids, moved);
    let mut tx = state.pool.begin().await?;
    for (index, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE cart_pages SET position = ? WHERE id = ?")
            .bind(u32::try_from(index).unwrap_or(u32::MAX))
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Puts a cart where asked among its page's pads, then rewrites every
/// position as 0, 1, 2… in the resulting order.
async fn order_carts(state: &AppState, page_id: u64, moved: Option<(u64, u32)>) -> ApiResult<()> {
    let mut ids: Vec<u64> =
        sqlx::query_scalar("SELECT id FROM carts WHERE page_id = ? ORDER BY position, id")
            .bind(page_id)
            .fetch_all(&state.pool)
            .await?;
    order(&mut ids, moved);
    let mut tx = state.pool.begin().await?;
    for (index, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE carts SET position = ? WHERE id = ?")
            .bind(u32::try_from(index).unwrap_or(u32::MAX))
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Moves `moved` to `position` in `ids`, clamping to the ends.
fn order(ids: &mut Vec<u64>, moved: Option<(u64, u32)>) {
    if let Some((id, position)) = moved {
        ids.retain(|row| *row != id);
        let at = (position as usize).min(ids.len());
        ids.insert(at, id);
    }
}

/// The page, if it belongs to the station.
async fn page_of(state: &AppState, station_id: u64, page_id: u64) -> ApiResult<PageRow> {
    sqlx::query_as("SELECT id, name, position FROM cart_pages WHERE id = ? AND station_id = ?")
        .bind(page_id)
        .bind(station_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("cart page"))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub struct CartsQuery {
    /// Only the carts of this page.
    pub page_id: Option<u64>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PageInput {
    pub name: String,
    /// Where the page goes, 0 first. The end when missing.
    pub position: Option<u32>,
}

/// The cart wall: its pages and their pads. Operators and admins.
#[utoipa::path(get, path = "/cart-pages", tag = "cart wall",
    responses((status = 200, body = Vec<CartPage>), (status = "4XX", body = ErrorBody)))]
async fn list_pages(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
) -> ApiResult<Json<Vec<CartPage>>> {
    user.require(Role::Operator)?;
    Ok(Json(pages_of(&state, station.id()).await?))
}

/// The carts of a page, or of the whole station. Operators and admins.
#[utoipa::path(get, path = "/carts", tag = "cart wall", params(CartsQuery),
    responses((status = 200, body = Vec<Cart>), (status = "4XX", body = ErrorBody)))]
async fn list_carts(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Query(query): Query<CartsQuery>,
) -> ApiResult<Json<Vec<Cart>>> {
    user.require(Role::Operator)?;
    if let Some(page_id) = query.page_id {
        page_of(&state, station.id(), page_id).await?;
    }
    let carts: Vec<CartRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SELECT_CARTS} AND (? IS NULL OR c.page_id = ?)
         ORDER BY p.position, p.id, c.position, c.id"
    )))
    .bind(station.id())
    .bind(query.page_id)
    .bind(query.page_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(carts.into_iter().map(Cart::from).collect()))
}

/// Adds a page. Operators and admins.
#[utoipa::path(post, path = "/cart-pages", tag = "cart wall",
    request_body = PageInput,
    responses((status = 201, body = CartPage), (status = "4XX", body = ErrorBody)))]
async fn create_page(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Json(input): Json<PageInput>,
) -> ApiResult<(StatusCode, Json<CartPage>)> {
    user.require(Role::Operator)?;
    let name = checked_name(&input.name, "a page")?;
    let next: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cart_pages WHERE station_id = ?")
        .bind(station.id())
        .fetch_one(&state.pool)
        .await?;
    let position = input
        .position
        .unwrap_or(u32::try_from(next).unwrap_or(u32::MAX));
    let id = sqlx::query("INSERT INTO cart_pages (station_id, name, position) VALUES (?, ?, ?)")
        .bind(station.id())
        .bind(name)
        .bind(position)
        .execute(&state.pool)
        .await?
        .last_insert_id();
    order_pages(&state, station.id(), Some((id, position))).await?;
    let page = pages_of(&state, station.id())
        .await?
        .into_iter()
        .find(|page| page.id == id)
        .ok_or_else(|| ApiError::not_found("cart page"))?;
    Ok((StatusCode::CREATED, Json(page)))
}

/// Renames a page or moves it among the others. Operators and admins.
#[utoipa::path(put, path = "/cart-pages/{id}", tag = "cart wall",
    params(("id" = u64, Path)), request_body = PageInput,
    responses((status = 200, body = CartPage), (status = "4XX", body = ErrorBody)))]
async fn update_page(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Path(id): Path<u64>,
    Json(input): Json<PageInput>,
) -> ApiResult<Json<CartPage>> {
    user.require(Role::Operator)?;
    let _page = page_of(&state, station.id(), id).await?;
    let name = checked_name(&input.name, "a page")?;
    sqlx::query("UPDATE cart_pages SET name = ? WHERE id = ?")
        .bind(name)
        .bind(id)
        .execute(&state.pool)
        .await?;
    order_pages(
        &state,
        station.id(),
        input.position.map(|position| (id, position)),
    )
    .await?;
    pages_of(&state, station.id())
        .await?
        .into_iter()
        .find(|page| page.id == id)
        .map(Json)
        .ok_or_else(|| ApiError::not_found("cart page"))
}

/// Deletes a page and its carts. Operators and admins.
#[utoipa::path(delete, path = "/cart-pages/{id}", tag = "cart wall",
    params(("id" = u64, Path)),
    responses((status = 204), (status = "4XX", body = ErrorBody)))]
async fn delete_page(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Path(id): Path<u64>,
) -> ApiResult<StatusCode> {
    user.require(Role::Operator)?;
    page_of(&state, station.id(), id).await?;
    sqlx::query("DELETE FROM cart_pages WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await?;
    order_pages(&state, station.id(), None).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CartInput {
    pub page_id: u64,
    pub track_id: u64,
    /// What the pad says. The track's title when missing.
    pub label: Option<String>,
    /// `#rrggbb`, or none for the pad's default colour.
    pub color: Option<String>,
    /// Where the pad goes in its page, 0 first. The end when missing.
    pub position: Option<u32>,
}

/// Adds a pad to a page. Operators and admins.
#[utoipa::path(post, path = "/carts", tag = "cart wall",
    request_body = CartInput,
    responses((status = 201, body = Cart), (status = "4XX", body = ErrorBody)))]
async fn create_cart(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Json(input): Json<CartInput>,
) -> ApiResult<(StatusCode, Json<Cart>)> {
    user.require(Role::Operator)?;
    let page = page_of(&state, station.id(), input.page_id).await?;
    let carts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM carts WHERE page_id = ?")
        .bind(page.id)
        .fetch_one(&state.pool)
        .await?;
    if carts >= PADS_PER_PAGE {
        return Err(ApiError::Conflict(format!(
            "a page holds at most {PADS_PER_PAGE} carts"
        )));
    }
    let title: Option<String> = sqlx::query_scalar(
        "SELECT COALESCE(title, '') FROM tracks WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(input.track_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some(title) = title else {
        return Err(ApiError::not_found("track"));
    };
    let fallback = if title.is_empty() {
        format!("Track {}", input.track_id)
    } else {
        title
    };
    let label = match input.label.as_deref() {
        Some(text) if !text.trim().is_empty() => checked_name(text, "a cart")?,
        _ => fallback,
    };
    let color = color(input.color.as_deref())?;
    let position = input
        .position
        .unwrap_or(u32::try_from(carts).unwrap_or(u32::MAX));
    let id = sqlx::query(
        "INSERT INTO carts (page_id, track_id, label, color, position) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(page.id)
    .bind(input.track_id)
    .bind(label)
    .bind(color)
    .bind(position)
    .execute(&state.pool)
    .await?
    .last_insert_id();
    order_carts(&state, page.id, Some((id, position))).await?;
    cart_of(&state, station.id(), id)
        .await
        .map(|cart| (StatusCode::CREATED, Json(cart)))
}

/// Changes a pad: its track, its label, its colour, its page or its place.
/// Operators and admins.
#[utoipa::path(put, path = "/carts/{id}", tag = "cart wall",
    params(("id" = u64, Path)), request_body = CartInput,
    responses((status = 200, body = Cart), (status = "4XX", body = ErrorBody)))]
async fn update_cart(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Path(id): Path<u64>,
    Json(input): Json<CartInput>,
) -> ApiResult<Json<Cart>> {
    user.require(Role::Operator)?;
    let before = cart_of(&state, station.id(), id).await?;
    let page = page_of(&state, station.id(), input.page_id).await?;
    if page.id != before.page_id {
        let carts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM carts WHERE page_id = ?")
            .bind(page.id)
            .fetch_one(&state.pool)
            .await?;
        if carts >= PADS_PER_PAGE {
            return Err(ApiError::Conflict(format!(
                "a page holds at most {PADS_PER_PAGE} carts"
            )));
        }
    }
    let label = match input.label.as_deref() {
        Some(text) => checked_name(text, "a cart")?,
        None => before.label.clone(),
    };
    let color = color(input.color.as_deref().or(before.color.as_deref()))?;
    let position = input.position.unwrap_or(before.position);
    sqlx::query(
        "UPDATE carts SET page_id = ?, track_id = ?, label = ?, color = ?, position = ?
         WHERE id = ?",
    )
    .bind(page.id)
    .bind(input.track_id)
    .bind(label)
    .bind(color)
    .bind(position)
    .bind(id)
    .execute(&state.pool)
    .await?;
    order_carts(
        &state,
        page.id,
        input.position.map(|position| (id, position)),
    )
    .await?;
    if page.id != before.page_id {
        order_carts(&state, before.page_id, None).await?;
    }
    cart_of(&state, station.id(), id).await.map(Json)
}

/// Deletes a pad. Operators and admins.
#[utoipa::path(delete, path = "/carts/{id}", tag = "cart wall",
    params(("id" = u64, Path)),
    responses((status = 204), (status = "4XX", body = ErrorBody)))]
async fn delete_cart(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Path(id): Path<u64>,
) -> ApiResult<StatusCode> {
    user.require(Role::Operator)?;
    let cart = cart_of(&state, station.id(), id).await?;
    sqlx::query("DELETE FROM carts WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await?;
    order_carts(&state, cart.page_id, None).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The cart, if its page belongs to the station.
async fn cart_of(state: &AppState, station_id: u64, id: u64) -> ApiResult<Cart> {
    let cart: Option<CartRow> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!("{SELECT_CARTS} AND c.id = ?")))
            .bind(station_id)
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
    cart.map(Cart::from)
        .ok_or_else(|| ApiError::not_found("cart"))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PlayCartResponse {
    pub entry_id: u64,
}

/// Fires a cart on the FX player, replacing the one playing. Operators and
/// admins.
#[utoipa::path(post, path = "/carts/{id}/play", tag = "cart wall",
    params(("id" = u64, Path)),
    responses((status = 200, body = PlayCartResponse), (status = "4XX", body = ErrorBody),
              (status = 503, body = ErrorBody)))]
async fn play_cart(
    user: CurrentUser,
    State(state): State<AppState>,
    station: Station,
    Path(id): Path<u64>,
) -> ApiResult<Json<PlayCartResponse>> {
    user.require(Role::Operator)?;
    let cart = cart_of(&state, station.id(), id).await?;
    if cart.missing {
        return Err(ApiError::Conflict(format!(
            "the file of {} is missing",
            cart.label
        )));
    }
    let mut tracks = station
        .queue
        .tracks(&[cart.track_id], PlaySource::Manual)
        .await?;
    let track = tracks.pop().ok_or_else(|| ApiError::not_found("track"))?;
    let entry_id = station.engine.play_fx(track).await?;
    Ok(Json(PlayCartResponse { entry_id }))
}

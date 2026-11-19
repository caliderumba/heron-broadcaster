// SPDX-License-Identifier: AGPL-3.0-or-later

//! Converts, tags and files a recorded voice link take into the station's
//! "Voice track" category in the music library.

use std::process::Command;

use heron_core::library::VoiceLinkReport;
use sqlx::MySqlPool;

use crate::journal::Roots;
use crate::layout;

/// Converts `source_path` in staging with FFmpeg, tags it with the operator
/// and current date/time, files it into `roots.library/Voice Links`, registers
/// it in `tracks`, and links it to the station's "Voice track" category.
pub async fn import(
    pool: &MySqlPool,
    roots: &Roots,
    station_id: u64,
    source_path: &str,
    operator: &str,
) -> Result<VoiceLinkReport, String> {
    let source_full = roots.staging.join(source_path);
    if !source_full.exists() {
        return Err(format!("staged take not found: {source_path}"));
    }

    let now = jiff::Zoned::now();
    let title = format!("Voice link {}", now.strftime("%Y-%m-%d %H:%M:%S"));
    let safe_op = layout::safe_component(operator);
    let safe_op = if safe_op.is_empty() {
        "operator"
    } else {
        &safe_op
    };
    let filename_base = format!("{safe_op}_{}", now.strftime("%Y%m%d_%H%M%S"));

    let target_dir = roots.library.join("Voice Links");
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| format!("cannot create directory: {e}"))?;

    let mut dest_filename = format!("{filename_base}.flac");
    let mut dest_path = target_dir.join(&dest_filename);
    let mut suffix = 2;
    while dest_path.exists() {
        dest_filename = format!("{filename_base}_{suffix}.flac");
        dest_path = target_dir.join(&dest_filename);
        suffix += 1;
    }

    let input_for_ffmpeg = source_full.clone();
    let dest_for_ffmpeg = dest_path.clone();
    let title_for_ffmpeg = title.clone();
    let operator_for_ffmpeg = operator.to_owned();

    let output = tokio::task::spawn_blocking(move || {
        Command::new("ffmpeg")
            .arg("-y")
            .arg("-i")
            .arg(&input_for_ffmpeg)
            .arg("-metadata")
            .arg(format!("title={title_for_ffmpeg}"))
            .arg("-metadata")
            .arg(format!("artist={operator_for_ffmpeg}"))
            .arg("-metadata")
            .arg("album=Voice Links")
            .arg("-c:a")
            .arg("flac")
            .arg(&dest_for_ffmpeg)
            .output()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("cannot execute ffmpeg: {e}"))?;

    // Always remove the source file from staging (no orphan files)
    let _ = tokio::fs::remove_file(&source_full).await;

    if !output.status.success() {
        let _ = tokio::fs::remove_file(&dest_path).await;
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg conversion failed: {stderr}"));
    }

    // Read tags and properties from the converted FLAC file
    let file_tags = heron_library::tags::read(&dest_path)
        .map_err(|e| format!("cannot read converted file properties: {e}"))?;
    let duration_ms = file_tags.duration_ms.unwrap_or(0);
    let metadata =
        std::fs::metadata(&dest_path).map_err(|e| format!("cannot read file metadata: {e}"))?;
    let file_size = metadata.len();
    let file_mtime_us = metadata
        .modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0);

    let rel_path = format!("Voice Links/{dest_filename}");

    // Insert track into `tracks`
    let track_id = sqlx::query(
        "INSERT INTO tracks (path, title, artist, album, duration_ms, file_size, file_mtime_us, sample_rate, channels, bitrate_kbps)
         VALUES (?, ?, ?, 'Voice Links', ?, ?, ?, ?, ?, ?)",
    )
    .bind(&rel_path)
    .bind(&title)
    .bind(operator)
    .bind(duration_ms)
    .bind(file_size)
    .bind(file_mtime_us)
    .bind(file_tags.properties.sample_rate)
    .bind(file_tags.properties.channels)
    .bind(file_tags.properties.bitrate_kbps)
    .execute(pool)
    .await
    .map_err(|e| format!("cannot insert track: {e}"))?
    .last_insert_id();

    // Find or create "Voice track" category for this station
    let mut category_id: Option<u64> = sqlx::query_scalar(
        "SELECT c.id FROM categories c
         JOIN category_stations cs ON cs.category_id = c.id
         WHERE c.name = 'Voice track' AND cs.station_id = ?",
    )
    .bind(station_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("cannot check station category: {e}"))?;

    if category_id.is_none() {
        // Check if category exists globally
        let existing: Option<u64> =
            sqlx::query_scalar("SELECT id FROM categories WHERE name = 'Voice track'")
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("cannot check category: {e}"))?;

        let id = match existing {
            Some(id) => id,
            None => {
                let type_id: Option<u64> =
                    sqlx::query_scalar("SELECT id FROM category_types WHERE name = 'Jingle'")
                        .fetch_optional(pool)
                        .await
                        .map_err(|e| format!("cannot find Jingle category type: {e}"))?;

                sqlx::query(
                    "INSERT INTO categories (name, color, type_id, fx_player, ducks_music)
                     VALUES ('Voice track', '#4a90e2', ?, TRUE, TRUE)",
                )
                .bind(type_id)
                .execute(pool)
                .await
                .map_err(|e| format!("cannot create Voice track category: {e}"))?
                .last_insert_id()
            }
        };

        // Link category to this station
        sqlx::query("INSERT IGNORE INTO category_stations (category_id, station_id) VALUES (?, ?)")
            .bind(id)
            .bind(station_id)
            .execute(pool)
            .await
            .map_err(|e| format!("cannot link category to station: {e}"))?;

        category_id = Some(id);
    }

    if let Some(cat_id) = category_id {
        sqlx::query(
            "INSERT IGNORE INTO track_categories (track_id, category_id, weight) VALUES (?, ?, 1.0)",
        )
        .bind(track_id)
        .bind(cat_id)
        .execute(pool)
        .await
        .map_err(|e| format!("cannot link track to category: {e}"))?;
    }

    // Queue analysis for the new track
    if let Ok(analysis_settings) = heron_analysis::jobs::load_settings(pool).await {
        let _ = heron_analysis::jobs::enqueue_stale(pool, &analysis_settings).await;
    }

    tracing::info!(track_id, %rel_path, %title, duration_ms, "voice link imported");

    Ok(VoiceLinkReport {
        track_id,
        path: rel_path,
        title,
        duration_ms,
    })
}

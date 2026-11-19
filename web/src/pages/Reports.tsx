// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import {
  type AirplayItem,
  type TopArtistItem,
  type TopTrackItem,
  api,
  unwrap,
} from "../api";
import {
  Card,
  Empty,
  Icon,
  Loading,
  PageHead,
} from "../components/ui";
import { t } from "../i18n";
import { duration } from "../lib/format";
import { here, useStation } from "../lib/station";

type Tab = "log" | "top" | "summary";
type Preset = "today" | "yesterday" | "last7Days" | "last30Days" | "thisMonth";

function getPresetRange(preset: Preset): [number, number] {
  const now = Date.now();
  const todayStart = new Date();
  todayStart.setHours(0, 0, 0, 0);
  const startMs = todayStart.getTime();

  switch (preset) {
    case "today":
      return [startMs, now];
    case "yesterday":
      return [startMs - 86_400_000, startMs];
    case "last7Days":
      return [now - 7 * 86_400_000, now];
    case "last30Days":
      return [now - 30 * 86_400_000, now];
    case "thisMonth": {
      const d = new Date();
      d.setDate(1);
      d.setHours(0, 0, 0, 0);
      return [d.getTime(), now];
    }
  }
}

export function Reports() {
  const station = useStation();

  const [activeTab, setActiveTab] = useState<Tab>("log");
  const [preset, setPreset] = useState<Preset>("last7Days");
  const [[fromMs, toMs], setDateRange] = useState<[number, number]>(() =>
    getPresetRange("last7Days"),
  );

  const [sourceFilter, setSourceFilter] = useState<string>("all");
  const [searchTerm, setSearchTerm] = useState<string>("");
  const [page, setPage] = useState<number>(0);
  const PAGE_SIZE = 50;

  const handlePresetChange = (newPreset: Preset) => {
    setPreset(newPreset);
    setDateRange(getPresetRange(newPreset));
    setPage(0);
  };

  // Queries
  const airplayQuery = useQuery({
    queryKey: ["reports-airplay", station, fromMs, toMs, sourceFilter, searchTerm, page],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/reports/airplay", {
          params: {
            path: here(),
            query: {
              from: fromMs,
              to: toMs,
              limit: PAGE_SIZE,
              offset: page * PAGE_SIZE,
              ...(sourceFilter !== "all" ? { source: sourceFilter } : {}),
              ...(searchTerm.trim() ? { search: searchTerm.trim() } : {}),
            },
          },
        }),
      ),
  });

  const topTracksQuery = useQuery({
    queryKey: ["reports-top-tracks", station, fromMs, toMs],
    enabled: activeTab === "top",
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/reports/top-tracks", {
          params: {
            path: here(),
            query: { from: fromMs, to: toMs, limit: 25 },
          },
        }),
      ),
  });

  const topArtistsQuery = useQuery({
    queryKey: ["reports-top-artists", station, fromMs, toMs],
    enabled: activeTab === "top",
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/reports/top-artists", {
          params: {
            path: here(),
            query: { from: fromMs, to: toMs, limit: 25 },
          },
        }),
      ),
  });

  const summaryQuery = useQuery({
    queryKey: ["reports-summary", station, fromMs, toMs],
    enabled: activeTab === "summary",
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/reports/summary", {
          params: {
            path: here(),
            query: { from: fromMs, to: toMs },
          },
        }),
      ),
  });

  const handleExport = (format: "csv" | "json") => {
    const src = sourceFilter !== "all" ? `&source=${encodeURIComponent(sourceFilter)}` : "";
    const search = searchTerm.trim() ? `&search=${encodeURIComponent(searchTerm.trim())}` : "";
    const url = `/api/v1/stations/${station}/reports/airplay/export?from=${fromMs}&to=${toMs}&format=${format}${src}${search}`;
    window.open(url, "_blank");
  };

  return (
    <>
      <PageHead
        title={t("reports.title")}
        description={t("reports.description")}
        actions={
          <div className="row wrap" style={{ gap: "0.5rem" }}>
            <button
              type="button"
              className="btn"
              onClick={() => handleExport("csv")}
            >
              <Icon name="download" size={16} /> {t("reports.exportCsv")}
            </button>
            <button
              type="button"
              className="btn"
              onClick={() => handleExport("json")}
            >
              <Icon name="download" size={16} /> {t("reports.exportJson")}
            </button>
          </div>
        }
      />

      {/* Preset date buttons */}
      <div
        className="row wrap space-between"
        style={{ gap: "1rem", marginBottom: "1.5rem", alignItems: "center" }}
      >
        <div className="segmented">
          {[
            { key: "today" as const, label: t("reports.presets.today") },
            { key: "yesterday" as const, label: t("reports.presets.yesterday") },
            { key: "last7Days" as const, label: t("reports.presets.last7Days") },
            { key: "last30Days" as const, label: t("reports.presets.last30Days") },
            { key: "thisMonth" as const, label: t("reports.presets.thisMonth") },
          ].map((item) => (
            <button
              key={item.key}
              type="button"
              className={preset === item.key ? "active" : ""}
              onClick={() => handlePresetChange(item.key)}
            >
              {item.label}
            </button>
          ))}
        </div>

        {/* Tab switcher */}
        <div className="segmented">
          <button
            type="button"
            className={activeTab === "log" ? "active" : ""}
            onClick={() => setActiveTab("log")}
          >
            {t("reports.tabs.log")}
          </button>
          <button
            type="button"
            className={activeTab === "top" ? "active" : ""}
            onClick={() => setActiveTab("top")}
          >
            {t("reports.tabs.top")}
          </button>
          <button
            type="button"
            className={activeTab === "summary" ? "active" : ""}
            onClick={() => setActiveTab("summary")}
          >
            {t("reports.tabs.summary")}
          </button>
        </div>
      </div>

      {/* Tab contents */}
      {activeTab === "log" && (
        <AirplayLogTab
          data={airplayQuery.data}
          isPending={airplayQuery.isPending}
          sourceFilter={sourceFilter}
          setSourceFilter={(s) => {
            setSourceFilter(s);
            setPage(0);
          }}
          searchTerm={searchTerm}
          setSearchTerm={(term) => {
            setSearchTerm(term);
            setPage(0);
          }}
          page={page}
          setPage={setPage}
          pageSize={PAGE_SIZE}
        />
      )}

      {activeTab === "top" && (
        <TopChartsTab
          tracks={topTracksQuery.data}
          artists={topArtistsQuery.data}
          isPending={topTracksQuery.isPending || topArtistsQuery.isPending}
        />
      )}

      {activeTab === "summary" && (
        <SummaryTab
          summary={summaryQuery.data}
          isPending={summaryQuery.isPending}
        />
      )}
    </>
  );
}

function AirplayLogTab({
  data,
  isPending,
  sourceFilter,
  setSourceFilter,
  searchTerm,
  setSearchTerm,
  page,
  setPage,
  pageSize,
}: Readonly<{
  data: { items: AirplayItem[]; total: number } | undefined;
  isPending: boolean;
  sourceFilter: string;
  setSourceFilter: (s: string) => void;
  searchTerm: string;
  setSearchTerm: (term: string) => void;
  page: number;
  setPage: (p: number) => void;
  pageSize: number;
}>) {
  const items = data?.items ?? [];
  const total = data?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  return (
    <div className="stack" style={{ gap: "1rem" }}>
      {/* Filters row */}
      <div className="row wrap space-between" style={{ gap: "0.5rem" }}>
        <input
          type="text"
          className="input"
          style={{ flex: "1 1 250px" }}
          placeholder={t("reports.searchPlaceholder")}
          value={searchTerm}
          onChange={(e) => setSearchTerm(e.target.value)}
        />
        <select
          className="input"
          style={{ width: "auto" }}
          value={sourceFilter}
          onChange={(e) => setSourceFilter(e.target.value)}
        >
          <option value="all">{t("reports.filter.allSources")}</option>
          <option value="rotation">{t("reports.filter.rotation")}</option>
          <option value="request">{t("reports.filter.request")}</option>
          <option value="manual">{t("reports.filter.manual")}</option>
          <option value="fallback">{t("reports.filter.fallback")}</option>
        </select>
      </div>

      {isPending && <Loading />}

      {!isPending && items.length === 0 && (
        <Empty>{t("reports.empty")}</Empty>
      )}

      {!isPending && items.length > 0 && (
        <Card flush>
          <div style={{ overflowX: "auto" }}>
            <table className="table" style={{ width: "100%", borderCollapse: "collapse" }}>
              <thead>
                <tr style={{ borderBottom: "1px solid var(--border)", textAlign: "left" }}>
                  <th style={{ padding: "0.75rem 1rem" }}>{t("reports.col.time")}</th>
                  <th style={{ padding: "0.75rem 1rem" }}>{t("reports.col.title")}</th>
                  <th style={{ padding: "0.75rem 1rem" }}>{t("reports.col.artist")}</th>
                  <th style={{ padding: "0.75rem 1rem" }}>{t("reports.col.album")}</th>
                  <th style={{ padding: "0.75rem 1rem" }}>{t("reports.col.isrc")}</th>
                  <th style={{ padding: "0.75rem 1rem" }}>{t("reports.col.duration")}</th>
                  <th style={{ padding: "0.75rem 1rem" }}>{t("reports.col.source")}</th>
                </tr>
              </thead>
              <tbody>
                {items.map((item) => (
                  <tr
                    key={item.id}
                    style={{ borderBottom: "1px solid var(--surface-2)" }}
                  >
                    <td style={{ padding: "0.6rem 1rem", whiteSpace: "nowrap" }} className="muted small">
                      {new Date(item.started_at_unix_ms).toLocaleString([], {
                        month: "numeric",
                        day: "numeric",
                        hour: "2-digit",
                        minute: "2-digit",
                        second: "2-digit",
                      })}
                    </td>
                    <td style={{ padding: "0.6rem 1rem", fontWeight: 500 }}>{item.title}</td>
                    <td style={{ padding: "0.6rem 1rem" }}>{item.artist}</td>
                    <td style={{ padding: "0.6rem 1rem" }} className="muted">{item.album || "—"}</td>
                    <td style={{ padding: "0.6rem 1rem" }} className="muted small font-mono">{item.isrc || "—"}</td>
                    <td style={{ padding: "0.6rem 1rem", whiteSpace: "nowrap" }} className="muted small">
                      {item.played_ms ? duration(item.played_ms) : duration(item.duration_ms ?? 0)}
                    </td>
                    <td style={{ padding: "0.6rem 1rem" }}>
                      <span
                        className="badge"
                        style={{
                          fontSize: "0.7rem",
                          textTransform: "uppercase",
                          fontWeight: 600,
                        }}
                      >
                        {item.source}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Pagination */}
          {totalPages > 1 && (
            <div
              className="row wrap space-between"
              style={{ padding: "0.75rem 1rem", alignItems: "center", borderTop: "1px solid var(--border)" }}
            >
              <button
                type="button"
                className="btn"
                disabled={page === 0}
                onClick={() => setPage(page - 1)}
              >
                &larr;
              </button>
              <span className="muted small">
                {t("reports.page", { page: page + 1, pages: totalPages })}
              </span>
              <button
                type="button"
                className="btn"
                disabled={page >= totalPages - 1}
                onClick={() => setPage(page + 1)}
              >
                &rarr;
              </button>
            </div>
          )}
        </Card>
      )}
    </div>
  );
}

function TopChartsTab({
  tracks,
  artists,
  isPending,
}: Readonly<{
  tracks: TopTrackItem[] | undefined;
  artists: TopArtistItem[] | undefined;
  isPending: boolean;
}>) {
  if (isPending) {
    return <Loading />;
  }

  return (
    <div className="row wrap" style={{ gap: "1.5rem", alignItems: "flex-start" }}>
      {/* Top Songs */}
      <div style={{ flex: "1 1 360px" }}>
        <Card title={t("reports.topTracks")}>
          {!tracks || tracks.length === 0 ? (
            <Empty>{t("reports.empty")}</Empty>
          ) : (
            <div className="stack" style={{ gap: "0.75rem" }}>
              {tracks.map((track, index) => (
                <div
                  key={`${track.track_id}-${index}`}
                  className="row space-between"
                  style={{
                    alignItems: "center",
                    padding: "0.4rem 0",
                    borderBottom: "1px solid var(--surface-2)",
                  }}
                >
                  <div className="row" style={{ gap: "0.75rem", alignItems: "center" }}>
                    <span
                      style={{
                        width: "1.5rem",
                        textAlign: "center",
                        fontWeight: 700,
                        color: index < 3 ? "var(--good)" : "var(--muted)",
                      }}
                    >
                      {index + 1}
                    </span>
                    <div>
                      <div style={{ fontWeight: 600 }}>{track.title}</div>
                      <div className="muted small">{track.artist}</div>
                    </div>
                  </div>
                  <div style={{ textAlign: "right" }}>
                    <div style={{ fontWeight: 600 }}>
                      {t("reports.playsCount", { count: track.play_count })}
                    </div>
                    <div className="muted small">{duration(track.total_played_ms)}</div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>

      {/* Top Artists */}
      <div style={{ flex: "1 1 360px" }}>
        <Card title={t("reports.topArtists")}>
          {!artists || artists.length === 0 ? (
            <Empty>{t("reports.empty")}</Empty>
          ) : (
            <div className="stack" style={{ gap: "0.75rem" }}>
              {artists.map((artist, index) => (
                <div
                  key={`${artist.artist}-${index}`}
                  className="row space-between"
                  style={{
                    alignItems: "center",
                    padding: "0.4rem 0",
                    borderBottom: "1px solid var(--surface-2)",
                  }}
                >
                  <div className="row" style={{ gap: "0.75rem", alignItems: "center" }}>
                    <span
                      style={{
                        width: "1.5rem",
                        textAlign: "center",
                        fontWeight: 700,
                        color: index < 3 ? "var(--good)" : "var(--muted)",
                      }}
                    >
                      {index + 1}
                    </span>
                    <div>
                      <div style={{ fontWeight: 600 }}>{artist.artist}</div>
                      <div className="muted small">
                        {t("reports.distinctTracks", { count: artist.distinct_tracks })}
                      </div>
                    </div>
                  </div>
                  <div style={{ textAlign: "right" }}>
                    <div style={{ fontWeight: 600 }}>
                      {t("reports.playsCount", { count: artist.play_count })}
                    </div>
                    <div className="muted small">{duration(artist.total_played_ms)}</div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>
    </div>
  );
}

function SummaryTab({
  summary,
  isPending,
}: Readonly<{
  summary:
    | {
        total_plays: number;
        total_airtime_ms: number;
        unique_tracks: number;
        unique_artists: number;
        rotation_plays: number;
        request_plays: number;
        manual_plays: number;
        fallback_plays: number;
        hourly_distribution: { hour: number; play_count: number }[];
      }
    | undefined;
  isPending: boolean;
}>) {
  if (isPending) {
    return <Loading />;
  }

  if (!summary) {
    return <Empty>{t("reports.empty")}</Empty>;
  }

  const maxHourPlays = Math.max(
    1,
    ...summary.hourly_distribution.map((h) => h.play_count),
  );

  return (
    <div className="stack" style={{ gap: "1.5rem" }}>
      {/* 4 KPI cards */}
      <div
        className="row wrap"
        style={{ gap: "1rem", display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))" }}
      >
        <Card>
          <div className="muted small">{t("reports.summary.totalPlays")}</div>
          <div style={{ fontSize: "1.75rem", fontWeight: 700, margin: "0.25rem 0" }}>
            {summary.total_plays.toLocaleString()}
          </div>
        </Card>

        <Card>
          <div className="muted small">{t("reports.summary.totalAirtime")}</div>
          <div style={{ fontSize: "1.75rem", fontWeight: 700, margin: "0.25rem 0" }}>
            {duration(summary.total_airtime_ms)}
          </div>
        </Card>

        <Card>
          <div className="muted small">{t("reports.summary.uniqueTracks")}</div>
          <div style={{ fontSize: "1.75rem", fontWeight: 700, margin: "0.25rem 0" }}>
            {summary.unique_tracks.toLocaleString()}
          </div>
        </Card>

        <Card>
          <div className="muted small">{t("reports.summary.uniqueArtists")}</div>
          <div style={{ fontSize: "1.75rem", fontWeight: 700, margin: "0.25rem 0" }}>
            {summary.unique_artists.toLocaleString()}
          </div>
        </Card>
      </div>

      {/* Source breakdown */}
      <Card title={t("reports.summary.sourceBreakdown")}>
        <div className="stack" style={{ gap: "0.75rem" }}>
          {[
            { label: t("reports.filter.rotation"), count: summary.rotation_plays, color: "var(--accent)" },
            { label: t("reports.filter.request"), count: summary.request_plays, color: "var(--good)" },
            { label: t("reports.filter.manual"), count: summary.manual_plays, color: "var(--warn)" },
            { label: t("reports.filter.fallback"), count: summary.fallback_plays, color: "var(--bad)" },
          ].map((item) => {
            const pct = summary.total_plays > 0
              ? Math.round((item.count / summary.total_plays) * 100)
              : 0;
            return (
              <div key={item.label} className="stack" style={{ gap: "0.25rem" }}>
                <div className="row space-between small">
                  <span>{item.label}</span>
                  <span>
                    {item.count.toLocaleString()} ({pct}%)
                  </span>
                </div>
                <div
                  style={{
                    height: "8px",
                    background: "var(--surface-2)",
                    borderRadius: "4px",
                    overflow: "hidden",
                  }}
                >
                  <div
                    style={{
                      height: "100%",
                      width: `${pct}%`,
                      background: item.color,
                      borderRadius: "4px",
                    }}
                  />
                </div>
              </div>
            );
          })}
        </div>
      </Card>

      {/* 24-Hour Distribution */}
      <Card title={t("reports.summary.hourlyDistribution")}>
        <div
          style={{
            display: "flex",
            alignItems: "flex-end",
            height: "140px",
            gap: "4px",
            paddingTop: "1rem",
          }}
        >
          {summary.hourly_distribution.map((h) => {
            const heightPct = Math.round((h.play_count / maxHourPlays) * 100);
            return (
              <div
                key={h.hour}
                style={{
                  flex: 1,
                  display: "flex",
                  flexDirection: "column",
                  alignItems: "center",
                  height: "100%",
                  justifyContent: "flex-end",
                }}
                title={`${h.hour.toString().padStart(2, "0")}:00 – ${h.play_count} plays`}
              >
                <div
                  style={{
                    width: "100%",
                    height: `${heightPct}%`,
                    minHeight: h.play_count > 0 ? "4px" : "1px",
                    background: "var(--accent)",
                    borderRadius: "2px 2px 0 0",
                  }}
                />
                <span
                  style={{
                    fontSize: "0.65rem",
                    color: "var(--muted)",
                    marginTop: "4px",
                  }}
                >
                  {h.hour % 6 === 0 ? `${h.hour}h` : ""}
                </span>
              </div>
            );
          })}
        </div>
      </Card>
    </div>
  );
}

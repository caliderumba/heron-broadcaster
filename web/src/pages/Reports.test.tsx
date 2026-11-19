// @vitest-environment jsdom
// SPDX-License-Identifier: AGPL-3.0-or-later

import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { chooseStation } from "../lib/station";
import { answer, browserStubs, renderApp } from "../test/render";
import { Reports } from "./Reports";

browserStubs();

const MOCK_AIRPLAY_RESPONSE = {
  items: [
    {
      id: 1,
      track_id: 101,
      started_at_unix_ms: Date.now() - 3600000,
      ended_at_unix_ms: Date.now() - 3360000,
      title: "Rebelión",
      artist: "Joe Arroyo",
      album: "Fuego en mi Mente",
      isrc: "COA018600001",
      duration_ms: 240000,
      played_ms: 240000,
      deck: "a",
      source: "rotation",
      end_reason: "finished",
    },
    {
      id: 2,
      track_id: 102,
      started_at_unix_ms: Date.now() - 7200000,
      ended_at_unix_ms: Date.now() - 6900000,
      title: "La Murga",
      artist: "Willie Colón & Héctor Lavoe",
      album: "Asalto Navideño",
      isrc: "USF017100002",
      duration_ms: 300000,
      played_ms: 300000,
      deck: "b",
      source: "request",
      end_reason: "finished",
    },
  ],
  total: 2,
  from_unix_ms: Date.now() - 7 * 86400000,
  to_unix_ms: Date.now(),
};

const MOCK_TOP_TRACKS = [
  {
    track_id: 101,
    title: "Rebelión",
    artist: "Joe Arroyo",
    album: "Fuego en mi Mente",
    play_count: 14,
    total_played_ms: 3360000,
    last_played_at_unix_ms: Date.now() - 3600000,
  },
  {
    track_id: 102,
    title: "La Murga",
    artist: "Willie Colón & Héctor Lavoe",
    album: "Asalto Navideño",
    play_count: 9,
    total_played_ms: 2700000,
    last_played_at_unix_ms: Date.now() - 7200000,
  },
];

const MOCK_TOP_ARTISTS = [
  {
    artist: "Joe Arroyo",
    play_count: 22,
    total_played_ms: 5280000,
    distinct_tracks: 3,
    last_played_at_unix_ms: Date.now() - 3600000,
  },
  {
    artist: "Héctor Lavoe",
    play_count: 18,
    total_played_ms: 4800000,
    distinct_tracks: 5,
    last_played_at_unix_ms: Date.now() - 7200000,
  },
];

const MOCK_SUMMARY = {
  total_plays: 120,
  total_airtime_ms: 28800000, // 8 hours
  unique_tracks: 45,
  unique_artists: 28,
  rotation_plays: 95,
  request_plays: 15,
  manual_plays: 8,
  fallback_plays: 2,
  hourly_distribution: Array.from({ length: 24 }, (_, i) => ({
    hour: i,
    play_count: i >= 6 && i <= 22 ? 6 : 2,
  })),
};

describe("Reports page", () => {
  it("renders airplay log with tracks, artists, ISRCs and sources", async () => {
    chooseStation(1);
    answer({
      "GET /api/v1/stations/{station_id}/reports/airplay": MOCK_AIRPLAY_RESPONSE,
    });

    renderApp(<Reports />);

    await waitFor(() => {
      expect(screen.getByText("Rebelión")).toBeTruthy();
      expect(screen.getByText("Joe Arroyo")).toBeTruthy();
      expect(screen.getByText("COA018600001")).toBeTruthy();
      expect(screen.getByText("La Murga")).toBeTruthy();
      expect(screen.getByText("Willie Colón & Héctor Lavoe")).toBeTruthy();
    });
  });

  it("changes date preset when clicking preset buttons", async () => {
    chooseStation(1);
    answer({
      "GET /api/v1/stations/{station_id}/reports/airplay": MOCK_AIRPLAY_RESPONSE,
    });

    renderApp(<Reports />);

    await waitFor(() => {
      expect(screen.getByText("Rebelión")).toBeTruthy();
    });

    const todayBtn = screen.getByRole("button", { name: "Today" });
    fireEvent.click(todayBtn);

    expect(todayBtn.className).toContain("active");
  });

  it("switches to Top Charts tab and renders leaderboard", async () => {
    chooseStation(1);
    answer({
      "GET /api/v1/stations/{station_id}/reports/airplay": MOCK_AIRPLAY_RESPONSE,
      "GET /api/v1/stations/{station_id}/reports/top-tracks": MOCK_TOP_TRACKS,
      "GET /api/v1/stations/{station_id}/reports/top-artists": MOCK_TOP_ARTISTS,
    });

    renderApp(<Reports />);

    await waitFor(() => {
      expect(screen.getByText("Top Charts")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Top Charts"));

    await waitFor(() => {
      expect(screen.getByText("Top Played Songs")).toBeTruthy();
      expect(screen.getByText("Top Played Artists")).toBeTruthy();
      expect(screen.getAllByText("Joe Arroyo").length).toBeGreaterThan(0);
      expect(screen.getByText("14 plays")).toBeTruthy();
      expect(screen.getByText("22 plays")).toBeTruthy();
    });
  });

  it("switches to Performance Summary tab and renders KPIs", async () => {
    chooseStation(1);
    answer({
      "GET /api/v1/stations/{station_id}/reports/airplay": MOCK_AIRPLAY_RESPONSE,
      "GET /api/v1/stations/{station_id}/reports/summary": MOCK_SUMMARY,
    });

    renderApp(<Reports />);

    await waitFor(() => {
      expect(screen.getByText("Performance Summary")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Performance Summary"));

    await waitFor(() => {
      expect(screen.getByText("Total Playouts")).toBeTruthy();
      expect(screen.getByText("120")).toBeTruthy();
      expect(screen.getByText("Unique Songs")).toBeTruthy();
      expect(screen.getByText("45")).toBeTruthy();
      expect(screen.getByText("Unique Artists")).toBeTruthy();
      expect(screen.getByText("28")).toBeTruthy();
      expect(screen.getByText("Playout Source Breakdown")).toBeTruthy();
      expect(screen.getByText("24-Hour Daypart Distribution")).toBeTruthy();
    });
  });
});

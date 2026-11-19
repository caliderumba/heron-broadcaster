// @vitest-environment jsdom
// SPDX-License-Identifier: AGPL-3.0-or-later

import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { answer, browserStubs, renderApp } from "../test/render";
import { chooseStation } from "../lib/station";
import { Announcements } from "./Announcements";

browserStubs();

const MOCK_TIME_SETTINGS = {
  enabled: true,
  format: "h24" as const,
  voice_pack: "default",
};

const MOCK_WEATHER_SETTINGS = {
  enabled: true,
  city: "Cali",
  latitude: 3.4516,
  longitude: -76.532,
  units: "metric" as const,
};

const MOCK_WEATHER_STATUS = {
  city: "Cali",
  temperature: 24.5,
  summary: "Clear sky",
  units: "metric" as const,
  fetched_at_unix_ms: Date.now() - 300000,
};

describe("Announcements page", () => {
  it("renders time and weather settings and current status", async () => {
    chooseStation(1);
    answer({
      "GET /api/v1/stations/{station_id}/announcements/time": MOCK_TIME_SETTINGS,
      "GET /api/v1/stations/{station_id}/announcements/weather": MOCK_WEATHER_SETTINGS,
      "GET /api/v1/stations/{station_id}/announcements/weather/status": MOCK_WEATHER_STATUS,
    });

    renderApp(<Announcements />);

    await waitFor(() => {
      expect(screen.getByText("Spoken time")).toBeTruthy();
      expect(screen.getByText("Weather reports")).toBeTruthy();
    });

    expect(screen.getByDisplayValue("Cali")).toBeTruthy();
    expect(screen.getByDisplayValue("default")).toBeTruthy();
    expect(screen.getByText("25 °C")).toBeTruthy();
  });

  it("previews spoken time phrase", async () => {
    chooseStation(1);
    answer({
      "GET /api/v1/stations/{station_id}/announcements/time": MOCK_TIME_SETTINGS,
      "GET /api/v1/stations/{station_id}/announcements/weather": MOCK_WEATHER_SETTINGS,
      "GET /api/v1/stations/{station_id}/announcements/weather/status": MOCK_WEATHER_STATUS,
      "POST /api/v1/stations/{station_id}/announcements/time/preview": {
        kind: "time",
        text: "It is 14:30",
        phrase_parts: ["time_intro", "14", "30"],
      },
    });

    renderApp(<Announcements />);

    await waitFor(() => {
      expect(screen.getByText("Preview time")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Preview time"));

    await waitFor(() => {
      expect(screen.getByText("It is 14:30")).toBeTruthy();
      expect(screen.getByText("time_intro")).toBeTruthy();
    });
  });

  it("previews weather phrase and triggers refresh", async () => {
    chooseStation(1);
    let refreshed = false;
    answer({
      "GET /api/v1/stations/{station_id}/announcements/time": MOCK_TIME_SETTINGS,
      "GET /api/v1/stations/{station_id}/announcements/weather": MOCK_WEATHER_SETTINGS,
      "GET /api/v1/stations/{station_id}/announcements/weather/status": MOCK_WEATHER_STATUS,
      "POST /api/v1/stations/{station_id}/announcements/weather/preview": {
        kind: "weather",
        text: "Currently 25 °C, Clear sky",
        phrase_parts: ["weather_intro", "temp_25", "celsius", "summary_clear_sky"],
      },
      "POST /api/v1/stations/{station_id}/announcements/weather/refresh": () => {
        refreshed = true;
        return MOCK_WEATHER_STATUS;
      },
    });

    renderApp(<Announcements />);

    await waitFor(() => {
      expect(screen.getByText("Preview weather")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Preview weather"));

    await waitFor(() => {
      expect(screen.getByText("Currently 25 °C, Clear sky")).toBeTruthy();
      expect(screen.getByText("weather_intro")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Refresh weather"));

    await waitFor(() => {
      expect(refreshed).toBe(true);
    });
  });
});

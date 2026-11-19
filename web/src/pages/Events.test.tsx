// @vitest-environment jsdom
// SPDX-License-Identifier: AGPL-3.0-or-later

import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { answer, browserStubs, renderApp } from "../test/render";
import { chooseStation } from "../lib/station";
import {
  Events,
  describeDays,
  formatNextRun,
  formatTimeOfDay,
  formatTrigger,
  parseTimeOfDay,
  type ScheduledEvent,
} from "./Events";

browserStubs();

const MOCK_EVENT: ScheduledEvent = {
  id: 1,
  station_id: 1,
  name: "Top of Hour News",
  enabled: true,
  trigger_type: "recurring",
  days_mask: 31,
  time_of_day_s: 43200, // 12:00
  interval_s: null,
  scheduled_at_unix_ms: null,
  timing_mode: "exact",
  action_type: "play_track",
  action_data: { track_id: 10 },
  last_run_at_unix_ms: 1700000000000,
  next_run_at_unix_ms: Date.now() + 1800000, // in 30 min
};

describe("Events formatting helpers", () => {
  it("formats and parses time of day", () => {
    expect(formatTimeOfDay(0)).toBe("00:00");
    expect(formatTimeOfDay(43200)).toBe("12:00");
    expect(formatTimeOfDay(43260)).toBe("12:01");
    expect(formatTimeOfDay(86399)).toBe("23:59");

    expect(parseTimeOfDay("00:00")).toBe(0);
    expect(parseTimeOfDay("12:00")).toBe(43200);
    expect(parseTimeOfDay("12:30")).toBe(45000);
  });

  it("describes days masks", () => {
    expect(describeDays(127)).toContain("Every day");
    expect(describeDays(31)).toContain("Weekdays");
    expect(describeDays(96)).toContain("Weekends");
    expect(describeDays(0)).toContain("Never");
  });

  it("formats trigger descriptions", () => {
    expect(formatTrigger(MOCK_EVENT)).toContain("12:00");

    const intervalEvent: ScheduledEvent = {
      ...MOCK_EVENT,
      trigger_type: "interval",
      interval_s: 1800,
    };
    expect(formatTrigger(intervalEvent)).toContain("30");

    const onceEvent: ScheduledEvent = {
      ...MOCK_EVENT,
      trigger_type: "once",
      scheduled_at_unix_ms: 1700000000000,
    };
    expect(formatTrigger(onceEvent)).toBeTruthy();
  });

  it("formats next run countdown", () => {
    expect(formatNextRun(null)).toContain("None pending");
    expect(formatNextRun(Date.now() - 1000)).toContain("Due now");
    expect(formatNextRun(Date.now() + 1800000)).toContain("30 min");
    expect(formatNextRun(Date.now() + 7200000)).toContain("2h");
    expect(formatNextRun(Date.now() + 90000000)).toContain("1d");
  });
});

describe("Events page", () => {
  it("renders empty state", async () => {
    chooseStation(1);
    answer({
      "GET /api/v1/stations/{station_id}/events": [],
    });

    renderApp(<Events />);

    await waitFor(() => {
      expect(screen.getByText("No scheduled events configured for this station.")).toBeTruthy();
    });
  });

  it("renders event list and allows running now", async () => {
    chooseStation(1);
    let runNowCalled = false;
    answer({
      "GET /api/v1/stations/{station_id}/events": [MOCK_EVENT],
      "POST /api/v1/stations/{station_id}/events/{id}/run-now": () => {
        runNowCalled = true;
        return { success: true, message: "executed" };
      },
    });

    renderApp(<Events />);

    await waitFor(() => {
      expect(screen.getByText("Top of Hour News")).toBeTruthy();
    });

    expect(screen.getByText("Exact")).toBeTruthy();
    expect(screen.getByText("Play track")).toBeTruthy();
    expect(screen.getByText("Active")).toBeTruthy();

    const runBtn = screen.getByText("Run now");
    fireEvent.click(runBtn);

    await waitFor(() => {
      expect(runNowCalled).toBe(true);
    });
  });
});

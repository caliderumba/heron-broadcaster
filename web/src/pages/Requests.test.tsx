// @vitest-environment jsdom
// SPDX-License-Identifier: AGPL-3.0-or-later

import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { chooseStation } from "../lib/station";
import { answer, browserStubs, renderApp } from "../test/render";
import { Requests } from "./Requests";

browserStubs();

const MOCK_SETTINGS = {
  enabled: true,
  cooldown_minutes: 120,
  max_pending: 50,
  daily_cap: 5,
  allow_dedication: true,
  max_dedication_len: 200,
  title: "Song Requests",
  subtitle: "Send your favorite track to our on-air host",
};

const MOCK_REQUESTS = [
  {
    id: 1,
    station_id: 1,
    track_id: 101,
    title: "El Cantante",
    artist: "Héctor Lavoe",
    album: "Comedia",
    duration_ms: 360000,
    listener_name: "Carlos",
    dedication: "Para toda la familia salsera",
    status: "pending" as const,
    client_token: "token123",
    requested_at_unix_ms: Date.now() - 300000,
    reviewed_at_unix_ms: null,
    reviewed_by: null,
    reviewed_by_name: null,
  },
  {
    id: 2,
    station_id: 1,
    track_id: 102,
    title: "Pedro Navaja",
    artist: "Rubén Blades",
    album: "Siembra",
    duration_ms: 420000,
    listener_name: "Lucia",
    dedication: null,
    status: "approved" as const,
    client_token: "token456",
    requested_at_unix_ms: Date.now() - 600000,
    reviewed_at_unix_ms: Date.now() - 100000,
    reviewed_by: 1,
    reviewed_by_name: "admin",
  },
];

describe("Requests page", () => {
  it("renders requests list with tracks, listener names, and dedications", async () => {
    chooseStation(1);
    answer({
      "GET /api/v1/stations/{station_id}/requests": MOCK_REQUESTS,
      "GET /api/v1/stations/{station_id}/requests/settings": MOCK_SETTINGS,
    });

    renderApp(<Requests />);

    await waitFor(() => {
      expect(screen.getByText("El Cantante")).toBeTruthy();
      expect(screen.getByText("· Héctor Lavoe")).toBeTruthy();
      expect(screen.getByText("Carlos")).toBeTruthy();
      expect(screen.getByText("«Para toda la familia salsera»")).toBeTruthy();
      expect(screen.getByText("Pedro Navaja")).toBeTruthy();
      expect(screen.getByText("Lucia")).toBeTruthy();
    });
  });

  it("filters requests when status tab is selected", async () => {
    chooseStation(1);
    answer({
      "GET /api/v1/stations/{station_id}/requests": MOCK_REQUESTS,
      "GET /api/v1/stations/{station_id}/requests/settings": MOCK_SETTINGS,
    });

    renderApp(<Requests />);

    await waitFor(() => {
      expect(screen.getByText("El Cantante")).toBeTruthy();
    });

    const pendingTab = screen.getByRole("button", { name: "Pending" });
    fireEvent.click(pendingTab);

    expect(pendingTab.className).toContain("active");
  });

  it("approves a pending request", async () => {
    chooseStation(1);
    let approved = false;
    answer({
      "GET /api/v1/stations/{station_id}/requests": MOCK_REQUESTS,
      "GET /api/v1/stations/{station_id}/requests/settings": MOCK_SETTINGS,
      "POST /api/v1/stations/{station_id}/requests/{id}/approve": () => {
        approved = true;
        return {
          ...MOCK_REQUESTS[0],
          status: "approved" as const,
        };
      },
    });

    renderApp(<Requests />);

    await waitFor(() => {
      expect(screen.getByText("Approve")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Approve"));

    await waitFor(() => {
      expect(approved).toBe(true);
    });
  });

  it("rejects a pending request", async () => {
    chooseStation(1);
    let rejected = false;
    answer({
      "GET /api/v1/stations/{station_id}/requests": MOCK_REQUESTS,
      "GET /api/v1/stations/{station_id}/requests/settings": MOCK_SETTINGS,
      "POST /api/v1/stations/{station_id}/requests/{id}/reject": () => {
        rejected = true;
        return {
          ...MOCK_REQUESTS[0],
          status: "rejected" as const,
        };
      },
    });

    renderApp(<Requests />);

    await waitFor(() => {
      expect(screen.getByText("Reject")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Reject"));

    await waitFor(() => {
      expect(rejected).toBe(true);
    });
  });

  it("opens settings modal on settings button click", async () => {
    chooseStation(1);
    answer({
      "GET /api/v1/stations/{station_id}/requests": [],
      "GET /api/v1/stations/{station_id}/requests/settings": MOCK_SETTINGS,
    });

    renderApp(<Requests />);

    await waitFor(() => {
      expect(screen.getByText("Request settings")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Request settings"));

    await waitFor(() => {
      expect(screen.getByText("Track cooldown (minutes)")).toBeTruthy();
      expect(screen.getByText("Maximum pending queue size")).toBeTruthy();
    });
  });
});

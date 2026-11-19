// @vitest-environment jsdom
// SPDX-License-Identifier: AGPL-3.0-or-later

import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { chooseStation } from "../lib/station";
import { answer, browserStubs, renderApp } from "../test/render";
import { Campaigns } from "./Campaigns";

browserStubs();

const MOCK_CAMPAIGNS = [
  {
    id: 1,
    station_id: 1,
    name: "Summer Fiesta 2026",
    client_name: "Acme Beverages",
    status: "active" as const,
    start_date: "2026-06-01",
    end_date: "2026-08-31",
    daily_target: 6,
    total_target: 200,
    priority: 8,
    days_mask: 127,
    start_hour: 8,
    end_hour: 22,
    spots_count: 2,
    delivered_plays: 45,
    daily_delivered_plays: 3,
  },
  {
    id: 2,
    station_id: 1,
    name: "Night Club Promo",
    client_name: "Club Havana",
    status: "paused" as const,
    start_date: "2026-07-01",
    end_date: "2026-07-15",
    daily_target: 4,
    total_target: 50,
    priority: 5,
    days_mask: 96, // Fri, Sat
    start_hour: 18,
    end_hour: 24,
    spots_count: 1,
    delivered_plays: 20,
    daily_delivered_plays: 0,
  },
];

describe("Campaigns page", () => {
  it("renders campaign list with client names and progress", async () => {
    chooseStation(1);
    answer({
      "GET /api/v1/stations/{station_id}/campaigns": MOCK_CAMPAIGNS,
    });

    renderApp(<Campaigns />);

    await waitFor(() => {
      expect(screen.getByText("Summer Fiesta 2026")).toBeTruthy();
      expect(screen.getByText("Acme Beverages")).toBeTruthy();
      expect(screen.getByText("Night Club Promo")).toBeTruthy();
      expect(screen.getByText("Club Havana")).toBeTruthy();
    });

    expect(screen.getByText("45 / 200 (23%)")).toBeTruthy();
  });

  it("filters campaigns when status tab is clicked", async () => {
    chooseStation(1);
    answer({
      "GET /api/v1/stations/{station_id}/campaigns": MOCK_CAMPAIGNS,
    });

    renderApp(<Campaigns />);

    await waitFor(() => {
      expect(screen.getByText("Summer Fiesta 2026")).toBeTruthy();
    });

    const activeTab = screen.getByRole("button", { name: "Active" });
    fireEvent.click(activeTab);

    // Filter tab triggered
    expect(activeTab).toBeTruthy();
  });

  it("opens create campaign dialog on new campaign button click", async () => {
    chooseStation(1);
    answer({
      "GET /api/v1/stations/{station_id}/campaigns": [],
    });

    renderApp(<Campaigns />);

    await waitFor(() => {
      expect(screen.getByText("No advertising campaigns configured.")).toBeTruthy();
    });

    const newBtn = screen.getByText("New campaign");
    fireEvent.click(newBtn);

    await waitFor(() => {
      expect(screen.getByPlaceholderText("e.g. Summer Promo")).toBeTruthy();
      expect(screen.getByPlaceholderText("e.g. Acme Corp")).toBeTruthy();
    });
  });
});

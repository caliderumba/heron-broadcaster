// @vitest-environment jsdom
// SPDX-License-Identifier: AGPL-3.0-or-later

// Each page renders for an admin on a station that has nothing yet, and says
// so; what a page does with data has its own tests.

import { screen } from "@testing-library/react";
import type { ReactElement } from "react";
import { describe, expect, it } from "vitest";
import { chooseStation } from "../lib/station";
import { answer, browserStubs, lenient, renderApp } from "../test/render";
import { AGC_DEFAULTS, EQ_DEFAULTS, PLAYOUT_DEFAULTS } from "../test/settings";
import { Agc } from "./Agc";
import { Announcements } from "./Announcements";
import { Artists } from "./Artists";
import { Buffer } from "./Buffer";
import { Campaigns } from "./Campaigns";
import { Categories } from "./Categories";
import { Clocks } from "./Clocks";
import { Dashboard } from "./Dashboard";
import { Eq } from "./Eq";
import { Events } from "./Events";
import { History } from "./History";
import { Import } from "./Import";
import { Labels } from "./Labels";
import { Library } from "./Library";
import { Medleys } from "./Medleys";
import { Outputs } from "./Outputs";
import { Playout } from "./Playout";
import { QueuePage } from "./Queue";
import { Requests } from "./Requests";
import { Restrictions } from "./Restrictions";
import { Review } from "./Review";
import { Rotation } from "./Rotation";
import { Settings } from "./Settings";
import { Statistics } from "./Statistics";
import { Stations } from "./Stations";
import { Users } from "./Users";
import { Webhooks } from "./Webhooks";

browserStubs();

const PAGES: [string, () => ReactElement][] = [
  ["Agc", () => <Agc />],
  ["Announcements", () => <Announcements />],
  ["Artists", () => <Artists />],
  ["Buffer", () => <Buffer />],
  ["Campaigns", () => <Campaigns />],
  ["Categories", () => <Categories />],
  ["Clocks", () => <Clocks />],
  ["Dashboard", () => <Dashboard />],
  ["Eq", () => <Eq />],
  ["Events", () => <Events />],
  ["History", () => <History />],
  ["Import", () => <Import />],
  ["Labels", () => <Labels />],
  ["Library", () => <Library />],
  ["Medleys", () => <Medleys />],
  ["Outputs", () => <Outputs />],
  ["Playout", () => <Playout />],
  ["Queue", () => <QueuePage />],
  ["Requests", () => <Requests />],
  ["Restrictions", () => <Restrictions />],
  ["Review", () => <Review />],
  ["Rotation", () => <Rotation />],
  ["Settings", () => <Settings />],
  ["Statistics", () => <Statistics />],
  ["Stations", () => <Stations />],
  ["Users", () => <Users />],
  ["Webhooks", () => <Webhooks />],
];

describe("pages without data", () => {
  it.each(PAGES)("%s renders", async (_name, page) => {
    answer(
      lenient({
        "GET /api/v1/stations/{station_id}/settings/agc": AGC_DEFAULTS,
        "GET /api/v1/stations/{station_id}/settings/eq": EQ_DEFAULTS,
        "GET /api/v1/stations/{station_id}/settings/playout": PLAYOUT_DEFAULTS,
        "GET /api/v1/stations/{station_id}/announcements/time": {
          enabled: false,
          format: "h24",
          voice_pack: "default",
        },
        "GET /api/v1/stations/{station_id}/announcements/weather": {
          enabled: false,
          city: "",
          latitude: null,
          longitude: null,
          units: "metric",
        },
        "GET /api/v1/stations/{station_id}/announcements/weather/status": {
          city: "",
          temperature: null,
          summary: null,
          units: "metric",
          fetched_at_unix_ms: null,
        },
      }),
    );
    chooseStation(1);
    renderApp(page());
    await screen.findAllByRole("heading");
    expect(document.body.textContent?.length).toBeGreaterThan(0);
  });
});

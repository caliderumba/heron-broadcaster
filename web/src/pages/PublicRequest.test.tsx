// @vitest-environment jsdom
// SPDX-License-Identifier: AGPL-3.0-or-later

import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { answer, browserStubs, renderApp } from "../test/render";
import { PublicRequest } from "./PublicRequest";

browserStubs();

const MOCK_PUBLIC_SETTINGS = {
  enabled: true,
  allow_dedication: true,
  max_dedication_len: 200,
  title: "Cali Rumba Requests",
  subtitle: "Request your salsa classic for our live DJ",
};

const MOCK_SEARCH_RESULTS = [
  {
    id: 201,
    title: "Periodico de Ayer",
    artist: "Héctor Lavoe",
    album: "De Ti Depende",
    duration_ms: 280000,
    can_request: true,
    cooldown_until_unix_ms: null,
  },
  {
    id: 202,
    title: "Lloraras",
    artist: "Oscar D'León",
    album: "Dimension Latina",
    duration_ms: 240000,
    can_request: false,
    cooldown_until_unix_ms: Date.now() + 3600000,
  },
];

describe("PublicRequest page", () => {
  it("renders station branding and search box", async () => {
    answer({
      "GET /api/v1/public/requests/settings": MOCK_PUBLIC_SETTINGS,
    });

    renderApp(<PublicRequest />);

    await waitFor(() => {
      expect(screen.getByText("Cali Rumba Requests")).toBeTruthy();
      expect(screen.getByText("Request your salsa classic for our live DJ")).toBeTruthy();
      expect(screen.getByPlaceholderText("Search by song title, artist, or album…")).toBeTruthy();
    });
  });

  it("renders disabled message when requests are disabled", async () => {
    answer({
      "GET /api/v1/public/requests/settings": {
        ...MOCK_PUBLIC_SETTINGS,
        enabled: false,
      },
    });

    renderApp(<PublicRequest />);

    await waitFor(() => {
      expect(screen.getByText("Song requests are currently closed. Please check back later!")).toBeTruthy();
    });
  });

  it("searches tracks and renders playable and cooldown tracks", async () => {
    answer({
      "GET /api/v1/public/requests/settings": MOCK_PUBLIC_SETTINGS,
      "GET /api/v1/public/requests/search": MOCK_SEARCH_RESULTS,
    });

    renderApp(<PublicRequest />);

    await waitFor(() => {
      expect(screen.getByPlaceholderText("Search by song title, artist, or album…")).toBeTruthy();
    });

    const searchInput = screen.getByPlaceholderText("Search by song title, artist, or album…");
    fireEvent.change(searchInput, { target: { value: "Lavoe" } });

    const searchBtn = screen.getByRole("button", { name: "Search" });
    fireEvent.click(searchBtn);

    await waitFor(() => {
      expect(screen.getByText("Periodico de Ayer")).toBeTruthy();
      expect(screen.getByText("· Héctor Lavoe")).toBeTruthy();
      expect(screen.getByText("Request song")).toBeTruthy();
      expect(screen.getByText("Lloraras")).toBeTruthy();
      expect(screen.getByText("· Oscar D'León")).toBeTruthy();
    });
  });

  it("opens request modal and submits a song request", async () => {
    let submitted = false;
    answer({
      "GET /api/v1/public/requests/settings": MOCK_PUBLIC_SETTINGS,
      "GET /api/v1/public/requests/search": MOCK_SEARCH_RESULTS,
      "POST /api/v1/public/requests": () => {
        submitted = true;
        return {
          request_id: 1,
          status: "pending" as const,
          client_token: "mock-token-xyz",
          message: "Request submitted",
        };
      },
    });

    renderApp(<PublicRequest />);

    await waitFor(() => {
      expect(screen.getByPlaceholderText("Search by song title, artist, or album…")).toBeTruthy();
    });

    const searchInput = screen.getByPlaceholderText("Search by song title, artist, or album…");
    fireEvent.change(searchInput, { target: { value: "Lavoe" } });
    fireEvent.click(screen.getByRole("button", { name: "Search" }));

    await waitFor(() => {
      expect(screen.getByText("Request song")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Request song"));

    await waitFor(() => {
      expect(screen.getByPlaceholderText("e.g. Alex")).toBeTruthy();
    });

    const nameInput = screen.getByPlaceholderText("e.g. Alex");
    fireEvent.change(nameInput, { target: { value: "Maria" } });

    const dedicationInput = screen.getByPlaceholderText("Dedicate this song to someone special or leave a message for the DJ…");
    fireEvent.change(dedicationInput, { target: { value: "Con cariño para Cali" } });

    const submitBtn = screen.getByRole("button", { name: "Send request" });
    fireEvent.click(submitBtn);

    await waitFor(() => {
      expect(submitted).toBe(true);
      expect(screen.getByText("Request received!")).toBeTruthy();
    });
  });
});

// @vitest-environment jsdom
// SPDX-License-Identifier: AGPL-3.0-or-later

import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { t } from "../i18n";
import { chooseStation } from "../lib/station";
import { answer, lenient, renderApp, sent, user } from "../test/render";
import { CartWall, midiBinding } from "./CartWall";

function cart(id: number, label: string, extra: Record<string, unknown> = {}) {
  return {
    id,
    page_id: id === 3 ? 2 : 1,
    position: id - 1,
    track_id: 100 + id,
    label,
    color: null,
    title: label,
    artist: "New Order",
    duration_ms: 60_000,
    missing: false,
    ...extra,
  };
}

const pages = [
  { id: 1, name: "Jingles", position: 0, carts: [cart(1, "Liner"), cart(2, "Station ID")] },
  { id: 2, name: "Ads", position: 1, carts: [cart(3, "Ad")] },
];

function open() {
  answer(
    lenient({
      "GET /api/v1/stations/{station_id}/cart-pages": pages,
      "POST /api/v1/stations/{station_id}/carts/{id}/play": { entry_id: 7 },
      "DELETE /api/v1/stations/{station_id}/player/fx": undefined,
      "POST /api/v1/stations/{station_id}/cart-pages": pages[0],
      "PUT /api/v1/stations/{station_id}/cart-pages/{id}": pages[1],
      "DELETE /api/v1/stations/{station_id}/cart-pages/{id}": undefined,
      "POST /api/v1/stations/{station_id}/carts": pages[0]?.carts[0],
      "PUT /api/v1/stations/{station_id}/carts/{id}": pages[0]?.carts[0],
      "DELETE /api/v1/stations/{station_id}/carts/{id}": undefined,
      "GET /api/v1/tracks": { items: [], total: 0 },
    }),
  );
  chooseStation(1);
}

const playCalls = () => sent("POST /api/v1/stations/{station_id}/carts/{id}/play");

afterEach(() => {
  localStorage.clear();
  vi.unstubAllGlobals();
});

describe("the cart wall", () => {
  it("shows the pages and fires a pad", async () => {
    open();
    renderApp(<CartWall fx={undefined} paused={false} />, user("operator"));
    expect(await screen.findByText("Liner")).toBeTruthy();
    expect(screen.getByRole("tab", { name: /Jingles/ }).getAttribute("aria-selected")).toBe("true");
    fireEvent.click(screen.getByText("Station ID"));
    await waitFor(() => expect(playCalls()).toHaveLength(1));
    expect((playCalls()[0]?.options as { params: { path: { id: number } } }).params.path.id).toBe(2);
  });

  it("fires the pads with F1–F12 and changes page with Shift and PageDown", async () => {
    open();
    renderApp(<CartWall fx={undefined} paused={false} />, user("operator"));
    await screen.findByText("Liner");

    fireEvent.keyDown(window, { key: "F1" });
    await waitFor(() => expect(playCalls()).toHaveLength(1));
    expect((playCalls()[0]?.options as { params: { path: { id: number } } }).params.path.id).toBe(1);

    // Holding the key fires once.
    fireEvent.keyDown(window, { key: "F1", repeat: true });
    expect(playCalls()).toHaveLength(1);

    fireEvent.keyDown(window, { key: "F2", shiftKey: true });
    expect(screen.getByRole("tab", { name: /Ads/ }).getAttribute("aria-selected")).toBe("true");
    fireEvent.keyDown(window, { key: "F1" });
    await waitFor(() => expect(playCalls()).toHaveLength(2));
    expect((playCalls()[1]?.options as { params: { path: { id: number } } }).params.path.id).toBe(3);

    fireEvent.keyDown(window, { key: "PageUp" });
    expect(screen.getByRole("tab", { name: /Jingles/ }).getAttribute("aria-selected")).toBe("true");
  });

  it("leaves keys alone while a field has focus", async () => {
    open();
    renderApp(<CartWall fx={undefined} paused={false} />, user("operator"));
    await screen.findByText("Liner");
    const input = document.createElement("input");
    document.body.append(input);
    fireEvent.keyDown(input, { key: "F1" });
    expect(playCalls()).toHaveLength(0);
    input.remove();
  });

  it("refuses a pad whose file is gone", async () => {
    answer(
      lenient({
        "GET /api/v1/stations/{station_id}/cart-pages": [
          { id: 1, name: "Jingles", position: 0, carts: [cart(1, "Liner", { missing: true })] },
        ],
      }),
    );
    chooseStation(1);
    renderApp(<CartWall fx={undefined} paused={false} />, user("operator"));
    fireEvent.click(await screen.findByText("Liner"));
    expect(playCalls()).toHaveLength(0);
    expect(await screen.findByText(t("carts.missingHint"))).toBeTruthy();
  });

  it("stops the FX from the panel", async () => {
    open();
    renderApp(
      <CartWall
        fx={{
          deck: "fx",
          entry: { entry_id: 7, track: { track_id: 101 } as never },
          length_ms: 60_000,
          on_air: true,
          position_ms: 0,
        }}
        paused={false}
      />,
      user("operator"),
    );
    fireEvent.click(await screen.findByText(t("dashboard.stop")));
    await waitFor(() =>
      expect(sent("DELETE /api/v1/stations/{station_id}/player/fx")).toHaveLength(1),
    );
  });

  it("edits a pad from the panel", async () => {
    open();
    renderApp(<CartWall fx={undefined} paused={false} />, user("operator"));
    await screen.findByText("Liner");
    fireEvent.click(screen.getByText(t("carts.edit")));
    fireEvent.click(screen.getByRole("button", { name: t("carts.addToSlot", { key: "F3" }) }));
    expect(await screen.findByText(t("carts.addTitle"))).toBeTruthy();
  });

  it("shows nothing to a viewer", async () => {
    open();
    renderApp(<CartWall fx={undefined} paused={false} />, user("viewer"));
    await waitFor(() => expect(sent("GET /api/v1/stations/{station_id}/cart-pages")).toHaveLength(0));
    expect(screen.queryByText(t("carts.title"))).toBeNull();
  });

  it("fires a pad from its MIDI control", async () => {
    localStorage.setItem("heron.carts.midi", JSON.stringify({ 1: "note:60:0" }));
    const handlers: ((event: { data: Uint8Array }) => void)[] = [];
    const input = {
      addEventListener: (_type: string, handler: (event: { data: Uint8Array }) => void) => handlers.push(handler),
      removeEventListener: () => {},
    };
    vi.stubGlobal("navigator", {
      ...navigator,
      requestMIDIAccess: async () => ({ inputs: new Map([["in", input]]), onstatechange: null }),
    });
    open();
    renderApp(<CartWall fx={undefined} paused={false} />, user("operator"));
    await screen.findByText("Liner");
    await waitFor(() => expect(handlers).toHaveLength(1));
    act(() => handlers[0]?.({ data: new Uint8Array([0x90, 60, 100]) }));
    await waitFor(() => expect(playCalls()).toHaveLength(1));
    expect((playCalls()[0]?.options as { params: { path: { id: number } } }).params.path.id).toBe(1);
  });
});

describe("midiBinding", () => {
  it("reads note-ons and control changes, and ignores the rest", () => {
    expect(midiBinding([0x90, 60, 100])).toBe("note:60:0");
    expect(midiBinding([0x93, 40, 1])).toBe("note:40:3");
    expect(midiBinding([0xb2, 7, 64])).toBe("cc:7:2");
    expect(midiBinding([0x90, 60, 0])).toBeNull();
    expect(midiBinding([0x80, 60, 0])).toBeNull();
  });
});

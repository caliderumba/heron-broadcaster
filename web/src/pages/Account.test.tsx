// @vitest-environment jsdom
// SPDX-License-Identifier: AGPL-3.0-or-later

import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { answer, browserStubs, renderApp, user } from "../test/render";
import { Account } from "./Account";

browserStubs();

describe("Account page", () => {
  it("renders theme options including studio and high contrast", async () => {
    answer({
      "GET /api/v1/auth/tokens": [],
    });

    renderApp(<Account />, {
      ...user("admin"),
      theme: "system",
    });

    expect(await screen.findByText("Display")).toBeDefined();
    expect(screen.getByText("System")).toBeDefined();
    expect(screen.getByRole("button", { name: /Light/i })).toBeDefined();
    expect(screen.getByRole("button", { name: /Dark/i })).toBeDefined();
    expect(screen.getByRole("button", { name: /Studio/i })).toBeDefined();
    expect(screen.getByRole("button", { name: /High contrast/i })).toBeDefined();
  });

  it("applies studio theme when selected", async () => {
    let savedTheme: string | undefined;
    answer({
      "GET /api/v1/auth/tokens": [],
      "PUT /api/v1/auth/preferences": (req: unknown) => {
        const body = (req as { body?: { theme?: string } }).body;
        savedTheme = body?.theme;
        return { ...user("admin"), theme: (savedTheme as any) ?? "studio" };
      },
    });

    renderApp(<Account />, {
      ...user("admin"),
      theme: "system",
    });

    const studioBtn = await screen.findByRole("button", { name: /Studio/i });
    fireEvent.click(studioBtn);

    await waitFor(() => {
      expect(document.documentElement.dataset.theme).toBe("studio");
    });
  });

  it("applies high contrast theme when selected", async () => {
    let savedTheme: string | undefined;
    answer({
      "GET /api/v1/auth/tokens": [],
      "PUT /api/v1/auth/preferences": (req: unknown) => {
        const body = (req as { body?: { theme?: string } }).body;
        savedTheme = body?.theme;
        return { ...user("admin"), theme: (savedTheme as any) ?? "high_contrast" };
      },
    });

    renderApp(<Account />, {
      ...user("admin"),
      theme: "system",
    });

    const hcBtn = await screen.findByRole("button", { name: /High contrast/i });
    fireEvent.click(hcBtn);

    await waitFor(() => {
      expect(document.documentElement.dataset.theme).toBe("high_contrast");
    });
  });
});

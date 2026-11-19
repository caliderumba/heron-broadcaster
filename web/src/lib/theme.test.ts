// @vitest-environment jsdom
// SPDX-License-Identifier: AGPL-3.0-or-later

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { applyFullWidth, applyTheme, storedFullWidth, storedTheme } from "./theme";

describe("theme library", () => {
  beforeEach(() => {
    window.localStorage.clear();
    delete document.documentElement.dataset.theme;
    delete document.documentElement.dataset.width;
  });

  afterEach(() => {
    window.localStorage.clear();
    delete document.documentElement.dataset.theme;
    delete document.documentElement.dataset.width;
  });

  it("defaults to system when no theme is stored", () => {
    expect(storedTheme()).toBe("system");
  });

  it("stores and retrieves light and dark themes", () => {
    applyTheme("light");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(storedTheme()).toBe("light");

    applyTheme("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(storedTheme()).toBe("dark");
  });

  it("stores and retrieves studio and high contrast themes", () => {
    applyTheme("studio");
    expect(document.documentElement.dataset.theme).toBe("studio");
    expect(storedTheme()).toBe("studio");

    applyTheme("high_contrast");
    expect(document.documentElement.dataset.theme).toBe("high_contrast");
    expect(storedTheme()).toBe("high_contrast");
  });

  it("removes data-theme on system theme", () => {
    applyTheme("studio");
    expect(document.documentElement.dataset.theme).toBe("studio");

    applyTheme("system");
    expect(document.documentElement.dataset.theme).toBeUndefined();
    expect(storedTheme()).toBe("system");
  });

  it("manages full width preference", () => {
    expect(storedFullWidth()).toBe(false);

    applyFullWidth(true);
    expect(document.documentElement.dataset.width).toBe("full");
    expect(storedFullWidth()).toBe(true);

    applyFullWidth(false);
    expect(document.documentElement.dataset.width).toBeUndefined();
    expect(storedFullWidth()).toBe(false);
  });
});

// @vitest-environment jsdom
// SPDX-License-Identifier: AGPL-3.0-or-later

import { fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../i18n";
import { chooseStation } from "../lib/station";
import { answer, lenient, renderApp, user } from "../test/render";
import { VoiceRecorder } from "./VoiceRecorder";

class MockMediaStreamTrack {
  stop = vi.fn();
}

class MockMediaStream {
  tracks: MockMediaStreamTrack[] = [new MockMediaStreamTrack()];
  getTracks() {
    return this.tracks;
  }
}

class MockMediaRecorder {
  state: "inactive" | "recording" | "paused" = "inactive";
  ondataavailable: ((e: { data: Blob }) => void) | null = null;
  onstop: (() => void) | null = null;

  static isTypeSupported(_type: string) {
    return true;
  }

  constructor(public stream: MockMediaStream, public options?: { mimeType?: string }) {}

  start() {
    this.state = "recording";
  }

  stop() {
    this.state = "inactive";
    if (this.ondataavailable) {
      this.ondataavailable({ data: new Blob(["audio-bytes"], { type: "audio/webm" }) });
    }
    if (this.onstop) {
      this.onstop();
    }
  }
}

describe("VoiceRecorder", () => {
  beforeEach(() => {
    chooseStation(1);
    vi.stubGlobal("MediaRecorder", MockMediaRecorder);
    vi.stubGlobal("navigator", {
      ...navigator,
      mediaDevices: {
        getUserMedia: vi.fn().mockResolvedValue(new MockMediaStream()),
      },
    });
    vi.stubGlobal("URL", {
      ...URL,
      createObjectURL: vi.fn().mockReturnValue("blob:http://localhost/test-take"),
      revokeObjectURL: vi.fn(),
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders permission explainer and starts recording on click", async () => {
    const onClose = vi.fn();
    renderApp(<VoiceRecorder open onClose={onClose} />, user("operator"));

    expect(screen.getByText(t("voiceLinks.dialogTitle"))).toBeTruthy();
    expect(screen.getByText(t("voiceLinks.permissionExplainer"))).toBeTruthy();
    expect(screen.getByText(t("voiceLinks.startRecording"))).toBeTruthy();

    fireEvent.click(screen.getByText(t("voiceLinks.startRecording")));

    await waitFor(() => {
      expect(screen.getByText(t("voiceLinks.stopRecording"))).toBeTruthy();
    });
  });

  it("stops recording and allows uploading the take, then playing over intro", async () => {
    const taskReport = {
      id: 42,
      status: "done",
      voice_link: {
        track_id: 101,
        path: "music/voice-track.flac",
        title: "Voice link 2026-11-28",
        duration_ms: 4500,
      },
    };

    answer(
      lenient({
        "POST /api/v1/stations/{station_id}/voice-links": { id: 42, status: "pending" },
        "GET /api/v1/library/tasks/{id}": taskReport,
        "POST /api/v1/stations/{station_id}/player/fx": { entry_id: 9 },
      }),
    );

    const onPlay = vi.fn();
    renderApp(<VoiceRecorder open onClose={vi.fn()} onPlayOverIntro={onPlay} />, user("operator"));

    // 1. Start recording
    fireEvent.click(screen.getByText(t("voiceLinks.startRecording")));
    await waitFor(() => expect(screen.getByText(t("voiceLinks.stopRecording"))).toBeTruthy());

    // 2. Stop recording -> recorded state
    fireEvent.click(screen.getByText(t("voiceLinks.stopRecording")));
    await waitFor(() => expect(screen.getByText(t("voiceLinks.listenBack"))).toBeTruthy());
    expect(screen.getByText(t("voiceLinks.uploadTake"))).toBeTruthy();

    // 3. Upload take -> task poll
    fireEvent.click(screen.getByText(t("voiceLinks.uploadTake")));

    // 4. Task completes -> done state
    await waitFor(() => {
      expect(screen.getByText(t("voiceLinks.playOverIntro"))).toBeTruthy();
    });
    expect(screen.getByText(t("voiceLinks.uploaded", { title: "Voice link 2026-11-28" }))).toBeTruthy();

    // 5. Play over intro
    fireEvent.click(screen.getByText(t("voiceLinks.playOverIntro")));
    expect(onPlay).toHaveBeenCalledWith(101);
  });

  it("handles microphone permission rejection gracefully", async () => {
    vi.stubGlobal("navigator", {
      ...navigator,
      mediaDevices: {
        getUserMedia: vi.fn().mockRejectedValue(new Error("Permission denied")),
      },
    });

    renderApp(<VoiceRecorder open onClose={vi.fn()} />, user("operator"));

    fireEvent.click(screen.getByText(t("voiceLinks.startRecording")));

    await waitFor(() => {
      expect(screen.getByText(t("voiceLinks.micDenied"))).toBeTruthy();
    });
  });
});

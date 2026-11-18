// SPDX-License-Identifier: AGPL-3.0-or-later

// Voice recorder (M19): records a take from the browser's microphone with
// optional headphone monitoring of the on-air track from cue-in, uploads it
// to heron-librarian via heron-api, and offers to play it over the intro.

import { useEffect, useRef, useState } from "react";
import { api, unwrap } from "../api";
import { useTask } from "./Task";
import { Field, Icon, Modal } from "./ui";
import { t } from "../i18n";
import { duration, trackName } from "../lib/format";
import { useLive } from "../lib/live";
import { here } from "../lib/station";

export interface VoiceRecorderProps {
  open: boolean;
  onClose: () => void;
  onPlayOverIntro?: (trackId: number) => void;
}

type RecordState = "idle" | "recording" | "recorded" | "uploading" | "done";

export function VoiceRecorder({ open, onClose, onPlayOverIntro }: Readonly<VoiceRecorderProps>) {
  const player = useLive((live) => live.player);
  const onAirTrack = player?.decks.find((deck) => deck.on_air && deck.deck !== "fx")?.entry?.track;

  const [state, setState] = useState<RecordState>("idle");
  const [error, setError] = useState<string | null>(null);
  const [useMonitor, setUseMonitor] = useState(true);
  const [elapsedMs, setElapsedMs] = useState(0);

  const [recordedBlob, setRecordedBlob] = useState<Blob | null>(null);
  const [recordedUrl, setRecordedUrl] = useState<string | null>(null);

  const [taskId, setTaskId] = useState<number | null>(null);
  const taskQuery = useTask(taskId);

  const streamRef = useRef<MediaStream | null>(null);
  const recorderRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const monitorAudioRef = useRef<HTMLAudioElement | null>(null);
  const timerRef = useRef<number | null>(null);
  const startTimeRef = useRef<number>(0);

  // Stop everything when closing.
  useEffect(() => {
    if (!open) {
      cleanup();
    }
  }, [open]);

  // Clean up object URL when changed or unmounted.
  useEffect(() => {
    return () => {
      if (recordedUrl) {
        URL.revokeObjectURL(recordedUrl);
      }
    };
  }, [recordedUrl]);

  // Watch for task completion.
  useEffect(() => {
    if (state === "uploading" && taskQuery.data?.status === "done") {
      setState("done");
    } else if (state === "uploading" && taskQuery.data?.status === "failed") {
      setError(taskQuery.data.error ?? "Conversion failed");
      setState("recorded");
    }
  }, [state, taskQuery.data]);

  const cleanup = () => {
    if (timerRef.current !== null) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
    if (recorderRef.current && recorderRef.current.state !== "inactive") {
      try {
        recorderRef.current.stop();
      } catch {
        // ignore
      }
    }
    if (streamRef.current) {
      for (const track of streamRef.current.getTracks()) {
        track.stop();
      }
      streamRef.current = null;
    }
    if (monitorAudioRef.current) {
      monitorAudioRef.current.pause();
    }
  };

  const startRecording = async () => {
    setError(null);
    chunksRef.current = [];
    if (!navigator.mediaDevices?.getUserMedia) {
      setError(t("voiceLinks.micUnsupported"));
      return;
    }

    let stream: MediaStream;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      streamRef.current = stream;
    } catch {
      setError(t("voiceLinks.micDenied"));
      return;
    }

    try {
      const mimeType = [
        "audio/webm;codecs=opus",
        "audio/ogg;codecs=opus",
        "audio/webm",
        "audio/ogg",
        "audio/mp4",
        "",
      ].find((type) => !type || (typeof MediaRecorder !== "undefined" && MediaRecorder.isTypeSupported?.(type))) ?? "";

      const recorder = mimeType ? new MediaRecorder(stream, { mimeType }) : new MediaRecorder(stream);
      recorderRef.current = recorder;

      recorder.ondataavailable = (event) => {
        if (event.data && event.data.size > 0) {
          chunksRef.current.push(event.data);
        }
      };

      recorder.onstop = () => {
        const mime = mimeType || "audio/webm";
        const blob = new Blob(chunksRef.current, { type: mime });
        setRecordedBlob(blob);
        if (recordedUrl) {
          URL.revokeObjectURL(recordedUrl);
        }
        setRecordedUrl(URL.createObjectURL(blob));
        setState("recorded");
      };

      recorder.start(100);
      startTimeRef.current = performance.now();
      setElapsedMs(0);
      timerRef.current = window.setInterval(() => {
        setElapsedMs(Math.round(performance.now() - startTimeRef.current));
      }, 100);

      setState("recording");

      // Headphone monitoring: play on-air track from cue-in in local headphones.
      if (useMonitor && onAirTrack?.track_id && monitorAudioRef.current) {
        const cueInSec = (onAirTrack.cue_in_ms ?? 0) / 1000;
        monitorAudioRef.current.src = `/api/v1/tracks/${onAirTrack.track_id}/audio#t=${cueInSec}`;
        monitorAudioRef.current.currentTime = cueInSec;
        monitorAudioRef.current.play().catch(() => {
          // Autoplay policy or no audio element in test.
        });
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      cleanup();
      setState("idle");
    }
  };

  const stopRecording = () => {
    if (timerRef.current !== null) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
    if (monitorAudioRef.current) {
      monitorAudioRef.current.pause();
    }
    if (recorderRef.current && recorderRef.current.state !== "inactive") {
      recorderRef.current.stop();
    }
    if (streamRef.current) {
      for (const track of streamRef.current.getTracks()) {
        track.stop();
      }
      streamRef.current = null;
    }
  };

  const recordAgain = () => {
    cleanup();
    if (recordedUrl) {
      URL.revokeObjectURL(recordedUrl);
      setRecordedUrl(null);
    }
    setRecordedBlob(null);
    setTaskId(null);
    setError(null);
    setElapsedMs(0);
    setState("idle");
  };

  const uploadTake = async () => {
    if (!recordedBlob) return;
    setState("uploading");
    setError(null);
    try {
      const task = await unwrap(
        api.POST("/api/v1/stations/{station_id}/voice-links", {
          params: { path: here() },
          body: recordedBlob as unknown as number[],
          headers: { "Content-Type": "application/octet-stream" },
        }),
      );
      setTaskId(task.id);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setState("recorded");
    }
  };

  const handlePlayOverIntro = (trackId: number) => {
    if (onPlayOverIntro) {
      onPlayOverIntro(trackId);
    } else {
      void unwrap(
        api.POST("/api/v1/stations/{station_id}/player/fx", {
          params: { path: here() },
          body: { track_id: trackId },
        }),
      );
      onClose();
    }
  };

  const voiceLinkReport = taskQuery.data?.voice_link;

  return (
    <Modal title={t("voiceLinks.dialogTitle")} open={open} onClose={onClose}>
      <div className="stack">
        <p className="muted small" style={{ margin: 0 }}>
          {t("voiceLinks.permissionExplainer")}
        </p>

        {error && <output className="banner bad">{error}</output>}

        <audio ref={monitorAudioRef} preload="none" style={{ display: "none" }} />

        {state === "idle" && (
          <>
            <Field label={t("voiceLinks.monitorTrack")} hint={t("voiceLinks.monitorTrackHint")}>
              <label className="check">
                <input
                  type="checkbox"
                  checked={useMonitor && Boolean(onAirTrack)}
                  disabled={!onAirTrack}
                  onChange={(e) => setUseMonitor(e.target.checked)}
                />
                {onAirTrack
                  ? t("voiceLinks.onAirTrack", { title: trackName(onAirTrack) })
                  : t("voiceLinks.noMonitor")}
              </label>
            </Field>

            <div style={{ textAlign: "center", padding: "1.5rem 0" }}>
              <button
                type="button"
                className="btn primary big"
                onClick={startRecording}
                style={{ padding: "0.8rem 2rem", fontSize: "1.1rem" }}
              >
                <Icon name="mic" size={22} /> {t("voiceLinks.startRecording")}
              </button>
            </div>
          </>
        )}

        {state === "recording" && (
          <div style={{ textAlign: "center", padding: "2rem 0" }}>
            <div className="row" style={{ justifyContent: "center", alignItems: "center", marginBottom: "1.5rem" }}>
              <span
                style={{
                  display: "inline-block",
                  width: "14px",
                  height: "14px",
                  borderRadius: "50%",
                  background: "var(--danger, #e03e3e)",
                }}
              />
              <strong style={{ fontSize: "1.2rem" }}>{t("voiceLinks.recording")}</strong>
              <span className="mono" style={{ fontSize: "1.2rem", marginLeft: "0.5rem" }}>
                {duration(elapsedMs)}
              </span>
            </div>
            <button
              type="button"
              className="btn danger big"
              onClick={stopRecording}
              style={{ padding: "0.8rem 2rem", fontSize: "1.1rem" }}
            >
              <Icon name="stop" size={22} /> {t("voiceLinks.stopRecording")}
            </button>
          </div>
        )}

        {state === "recorded" && (
          <>
            <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
              <strong>{t("voiceLinks.listenBack")}</strong>
              <span className="muted small">{duration(elapsedMs)}</span>
            </div>
            {recordedUrl && (
              <audio controls src={recordedUrl} style={{ width: "100%", margin: "0.5rem 0" }}>
                <track kind="captions" />
              </audio>
            )}
            <div className="row" style={{ justifyContent: "flex-end", gap: "0.5rem", marginTop: "1rem" }}>
              <button type="button" className="btn" onClick={recordAgain}>
                <Icon name="undo" size={16} /> {t("voiceLinks.recordAgain")}
              </button>
              <button type="button" className="btn primary" onClick={uploadTake}>
                <Icon name="upload" size={16} /> {t("voiceLinks.uploadTake")}
              </button>
            </div>
          </>
        )}

        {state === "uploading" && (
          <div style={{ textAlign: "center", padding: "2rem 0" }}>
            <span className="spinner" style={{ marginBottom: "0.5rem" }} />
            <p className="muted">{t("voiceLinks.uploading")}</p>
          </div>
        )}

        {state === "done" && voiceLinkReport && (
          <>
            <output className="banner good">
              <Icon name="check" /> {t("voiceLinks.uploaded", { title: voiceLinkReport.title })}
            </output>
            <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
              <strong>{voiceLinkReport.title}</strong>
              <span className="muted small">{duration(voiceLinkReport.duration_ms)}</span>
            </div>
            <audio
              controls
              src={`/api/v1/tracks/${voiceLinkReport.track_id}/audio`}
              style={{ width: "100%", margin: "0.5rem 0" }}
            >
              <track kind="captions" />
            </audio>
            <div className="row" style={{ justifyContent: "space-between", marginTop: "1rem" }}>
              <button type="button" className="btn" onClick={recordAgain}>
                <Icon name="plus" size={16} /> {t("voiceLinks.recordAgain")}
              </button>
              <button
                type="button"
                className="btn primary"
                title={t("voiceLinks.playOverIntroHint")}
                onClick={() => handlePlayOverIntro(voiceLinkReport.track_id)}
              >
                <Icon name="bolt" size={16} /> {t("voiceLinks.playOverIntro")}
              </button>
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}

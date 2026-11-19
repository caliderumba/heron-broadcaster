// SPDX-License-Identifier: AGPL-3.0-or-later

// Scheduled events: timed actions that fire on schedule.
// Shows events, timing mode (exact/next), countdowns, and execution controls.

import { useQuery } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import {
  type ActionType,
  type ScheduledEvent,
  type TimingMode,
  type TriggerType,
  api,
  done,
  unwrap,
} from "../api";
import {
  Card,
  ConfirmButton,
  Empty,
  ErrorView,
  Field,
  Icon,
  Loading,
  Modal,
  PageHead,
} from "../components/ui";
import { type PlainKey, rich, t } from "../i18n";
import { here, useStation } from "../lib/station";
import { useAction, useCan } from "../lib/session";
import { useCategories } from "./Clocks";

export type { ActionType, ScheduledEvent, TimingMode, TriggerType };

const ACTION_TYPE_LABELS: Record<ActionType, PlainKey> = {
  play_track: "events.actionTypes.play_track",
  play_category: "events.actionTypes.play_category",
  relay: "events.actionTypes.relay",
  load_clock: "events.actionTypes.load_clock",
};

const DAYS = [
  { bit: 1, labelKey: "events.days.mon" as const },
  { bit: 2, labelKey: "events.days.tue" as const },
  { bit: 4, labelKey: "events.days.wed" as const },
  { bit: 8, labelKey: "events.days.thu" as const },
  { bit: 16, labelKey: "events.days.fri" as const },
  { bit: 32, labelKey: "events.days.sat" as const },
  { bit: 64, labelKey: "events.days.sun" as const },
];

export function formatTimeOfDay(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return `${h.toString().padStart(2, "0")}:${m.toString().padStart(2, "0")}`;
}

export function parseTimeOfDay(hhmm: string): number {
  const [h, m] = hhmm.split(":").map(Number);
  return (h || 0) * 3600 + (m || 0) * 60;
}

export function describeDays(mask: number): string {
  if (mask === 127) return t("events.days.everyDay");
  if (mask === 31) return t("events.days.weekdays");
  if (mask === 96) return t("events.days.weekends");
  const active = DAYS.filter((d) => (mask & d.bit) !== 0).map((d) => t(d.labelKey));
  return active.length > 0 ? active.join(", ") : t("events.days.never");
}

export function formatTrigger(event: ScheduledEvent): string {
  switch (event.trigger_type) {
    case "recurring":
      return `${describeDays(event.days_mask)} @ ${formatTimeOfDay(event.time_of_day_s)}`;
    case "interval": {
      const minutes = Math.round((event.interval_s ?? 0) / 60);
      return t("events.everyMinutes", { count: minutes });
    }
    case "once":
      if (!event.scheduled_at_unix_ms) return t("events.notScheduled");
      return new Date(event.scheduled_at_unix_ms).toLocaleString();
  }
}

export function formatNextRun(nextMs: number | null | undefined): string {
  if (!nextMs) return t("events.nonePending");
  const diff = nextMs - Date.now();
  if (diff <= 0) return t("events.dueNow");
  const totalSeconds = Math.floor(diff / 1000);
  const days = Math.floor(totalSeconds / 86400);
  const hours = Math.floor((totalSeconds % 86400) / 3600);
  const mins = Math.floor((totalSeconds % 3600) / 60);
  if (days > 0) return t("events.inDaysHours", { days, hours });
  if (hours > 0) return t("events.inHoursMinutes", { hours, mins });
  return t("events.inMinutes", { mins: Math.max(1, mins) });
}

export function Events() {
  const canEdit = useCan("operator");
  const station = useStation();

  const events = useQuery({
    queryKey: ["events", station],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/events", {
          params: { path: here() },
        }),
      ),
  });

  const [editing, setEditing] = useState<ScheduledEvent | "new" | null>(null);

  const toggleEnabled = useAction(
    (event: ScheduledEvent) =>
      unwrap(
        api.PUT("/api/v1/stations/{station_id}/events/{id}", {
          params: { path: here({ id: event.id }) },
          body: {
            name: event.name,
            enabled: !event.enabled,
            trigger_type: event.trigger_type,
            days_mask: event.days_mask,
            time_of_day_s: event.time_of_day_s,
            interval_s: event.interval_s ?? null,
            scheduled_at_unix_ms: event.scheduled_at_unix_ms ?? null,
            timing_mode: event.timing_mode,
            action_type: event.action_type,
            action_data: event.action_data,
          },
        }),
      ),
    {
      success: t("events.saved"),
      invalidate: [["events", station]],
    },
  );

  const runNow = useAction(
    (id: number) =>
      unwrap(
        api.POST("/api/v1/stations/{station_id}/events/{id}/run-now", {
          params: { path: here({ id }) },
        }),
      ),
    {
      success: t("events.executed"),
      invalidate: [["events", station]],
    },
  );

  const remove = useAction(
    (id: number) =>
      done(
        api.DELETE("/api/v1/stations/{station_id}/events/{id}", {
          params: { path: here({ id }) },
        }),
      ),
    {
      success: t("events.deleted"),
      invalidate: [["events", station]],
    },
  );

  return (
    <>
      <PageHead
        title={t("nav.events")}
        description={t("events.description")}
        actions={
          canEdit && (
            <button
              type="button"
              className="btn primary"
              onClick={() => setEditing("new")}
            >
              <Icon name="plus" /> {t("events.new")}
            </button>
          )
        }
      />
      <Card flush>
        {events.isPending && <Loading />}
        {events.isError && <ErrorView error={events.error} />}
        {events.data?.length === 0 && <Empty>{t("events.none")}</Empty>}
        {events.data && events.data.length > 0 && (
          <div className="table-wrap">
            <table className="table">
              <thead>
                <tr>
                  <th>{t("events.name")}</th>
                  <th>{t("events.timing")}</th>
                  <th>{t("events.trigger")}</th>
                  <th>{t("events.action")}</th>
                  <th>{t("events.nextRun")}</th>
                  <th>{t("events.status")}</th>
                  <th>
                    <span className="visually-hidden">{t("library.actions")}</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {events.data.map((event) => (
                  <tr key={event.id}>
                    <td>
                      <span className="row">
                        <strong>{event.name}</strong>
                      </span>
                    </td>
                    <td>
                      <span
                        className={`badge ${
                          event.timing_mode === "exact" ? "primary" : "neutral"
                        }`}
                        title={
                          event.timing_mode === "exact"
                            ? t("events.timingExactHint")
                            : t("events.timingNextHint")
                        }
                      >
                        {event.timing_mode === "exact"
                          ? t("events.timingExact")
                          : t("events.timingNext")}
                      </span>
                    </td>
                    <td>{formatTrigger(event)}</td>
                    <td>
                      <span className="badge small">
                        {t(ACTION_TYPE_LABELS[event.action_type])}
                      </span>
                    </td>
                    <td>
                      <span
                        className="small"
                        title={
                          event.next_run_at_unix_ms
                            ? new Date(event.next_run_at_unix_ms).toLocaleString()
                            : undefined
                        }
                      >
                        {formatNextRun(event.next_run_at_unix_ms)}
                      </span>
                    </td>
                    <td>
                      {canEdit ? (
                        <button
                          type="button"
                          className={`btn small ${
                            event.enabled ? "secondary" : "ghost"
                          }`}
                          onClick={() => toggleEnabled.mutate(event)}
                          title={
                            event.enabled
                              ? t("events.disableHelp")
                              : t("events.enableHelp")
                          }
                        >
                          {event.enabled ? t("events.active") : t("events.inactive")}
                        </button>
                      ) : (
                        <span className="small muted">
                          {event.enabled ? t("events.active") : t("events.inactive")}
                        </span>
                      )}
                    </td>
                    <td className="actions">
                      {canEdit && (
                        <>
                          <button
                            type="button"
                            className="btn ghost small"
                            title={t("events.runNowHelp")}
                            onClick={() => runNow.mutate(event.id)}
                          >
                            <Icon name="play" /> {t("events.runNow")}
                          </button>
                          <button
                            type="button"
                            className="btn ghost small icon"
                            aria-label={t("events.edit", { name: event.name })}
                            onClick={() => setEditing(event)}
                          >
                            <Icon name="edit" />
                          </button>
                          <ConfirmButton
                            className="btn ghost small icon danger"
                            title={t("events.delete", { name: event.name })}
                            confirm={t("events.deleteButton")}
                            question={
                              <p>
                                {rich("events.deleteQuestion", {
                                  name: <strong>{event.name}</strong>,
                                })}
                              </p>
                            }
                            onConfirm={() => remove.mutate(event.id)}
                          >
                            <Icon name="trash" />
                          </ConfirmButton>
                        </>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>
      {editing && (
        <EventDialog
          event={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
        />
      )}
    </>
  );
}

function EventDialog({
  event,
  onClose,
}: Readonly<{
  event: ScheduledEvent | null;
  onClose: () => void;
}>) {
  const station = useStation();
  const [name, setName] = useState(event?.name ?? "");
  const [enabled, setEnabled] = useState(event?.enabled ?? true);
  const [triggerType, setTriggerType] = useState<TriggerType>(
    event?.trigger_type ?? "recurring",
  );
  const [daysMask, setDaysMask] = useState(event?.days_mask ?? 31);
  const [timeOfDay, setTimeOfDay] = useState(
    formatTimeOfDay(event?.time_of_day_s ?? 43200),
  );
  const [intervalMinutes, setIntervalMinutes] = useState(
    Math.round((event?.interval_s ?? 1800) / 60),
  );
  const [scheduledDate, setScheduledDate] = useState(
    event?.scheduled_at_unix_ms
      ? new Date(event.scheduled_at_unix_ms).toISOString().slice(0, 16)
      : new Date(Date.now() + 3600000).toISOString().slice(0, 16),
  );
  const [timingMode, setTimingMode] = useState<TimingMode>(
    event?.timing_mode ?? "exact",
  );
  const [actionType, setActionType] = useState<ActionType>(
    event?.action_type ?? "play_track",
  );

  const actionData = (event?.action_data ?? {}) as Record<string, unknown>;
  const [trackId, setTrackId] = useState<number>(
    (actionData.track_id as number) ?? 1,
  );
  const [categoryId, setCategoryId] = useState<number>(
    (actionData.category_id as number) ?? 1,
  );
  const [relayUrl, setRelayUrl] = useState<string>(
    (actionData.url as string) ?? "https://",
  );
  const [clockId, setClockId] = useState<number>(
    (actionData.clock_id as number) ?? 1,
  );

  const categories = useCategories();
  const clocks = useQuery({
    queryKey: ["clocks", station],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/clocks", {
          params: { path: here() },
        }),
      ),
  });

  const toggleDay = (bit: number) => {
    setDaysMask((prev) => (prev ^ bit));
  };

  const save = useAction(
    () => {
      let action_data: Record<string, unknown> = {};
      if (actionType === "play_track") {
        action_data = { track_id: Number(trackId) };
      } else if (actionType === "play_category") {
        action_data = { category_id: Number(categoryId) };
      } else if (actionType === "relay") {
        action_data = { url: relayUrl };
      } else if (actionType === "load_clock") {
        action_data = { clock_id: Number(clockId) };
      }

      const body = {
        name,
        enabled,
        trigger_type: triggerType,
        days_mask: daysMask,
        time_of_day_s: parseTimeOfDay(timeOfDay),
        interval_s: triggerType === "interval" ? intervalMinutes * 60 : null,
        scheduled_at_unix_ms:
          triggerType === "once" ? new Date(scheduledDate).getTime() : null,
        timing_mode: timingMode,
        action_type: actionType,
        action_data,
      };

      return event
        ? unwrap(
            api.PUT("/api/v1/stations/{station_id}/events/{id}", {
              params: { path: here({ id: event.id }) },
              body,
            }),
          )
        : unwrap(
            api.POST("/api/v1/stations/{station_id}/events", {
              params: { path: here() },
              body,
            }),
          );
    },
    {
      success: t("events.saved"),
      invalidate: [["events", station]],
      onSuccess: onClose,
    },
  );

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    save.mutate();
  };

  return (
    <Modal
      open={true}
      title={event ? t("events.editTitle") : t("events.newTitle")}
      onClose={onClose}
    >
      <form onSubmit={submit} className="stack">
        <Field label={t("events.name")}>
          <input
            type="text"
            className="input"
            required
            maxLength={64}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t("events.namePlaceholder")}
          />
        </Field>

        <label className="row">
          <input
            type="checkbox"
            checked={enabled}
            onChange={(e) => setEnabled(e.target.checked)}
          />
          {t("events.enabledLabel")}
        </label>

        <Field label={t("events.timingMode")}>
          <div className="row">
            <label className="row small">
              <input
                type="radio"
                name="timing_mode"
                value="exact"
                checked={timingMode === "exact"}
                onChange={() => setTimingMode("exact")}
              />
              {t("events.timingExact")}
            </label>
            <label className="row small">
              <input
                type="radio"
                name="timing_mode"
                value="next"
                checked={timingMode === "next"}
                onChange={() => setTimingMode("next")}
              />
              {t("events.timingNext")}
            </label>
          </div>
          <span className="small muted">
            {timingMode === "exact"
              ? t("events.timingExactHint")
              : t("events.timingNextHint")}
          </span>
        </Field>

        <Field label={t("events.triggerType")}>
          <select
            className="input"
            value={triggerType}
            onChange={(e) => setTriggerType(e.target.value as TriggerType)}
          >
            <option value="recurring">{t("events.triggers.recurring")}</option>
            <option value="interval">{t("events.triggers.interval")}</option>
            <option value="once">{t("events.triggers.once")}</option>
          </select>
        </Field>

        {triggerType === "recurring" && (
          <>
            <Field label={t("events.daysOfWeek")}>
              <div className="row wrap">
                {DAYS.map((d) => (
                  <label key={d.bit} className="row small">
                    <input
                      type="checkbox"
                      checked={(daysMask & d.bit) !== 0}
                      onChange={() => toggleDay(d.bit)}
                    />
                    {t(d.labelKey)}
                  </label>
                ))}
              </div>
              <div className="row small" style={{ marginTop: "0.25rem" }}>
                <button
                  type="button"
                  className="btn ghost small"
                  onClick={() => setDaysMask(127)}
                >
                  {t("events.days.all")}
                </button>
                <button
                  type="button"
                  className="btn ghost small"
                  onClick={() => setDaysMask(31)}
                >
                  {t("events.days.weekdays")}
                </button>
                <button
                  type="button"
                  className="btn ghost small"
                  onClick={() => setDaysMask(96)}
                >
                  {t("events.days.weekends")}
                </button>
              </div>
            </Field>

            <Field label={t("events.timeOfDay")}>
              <input
                type="time"
                className="input"
                required
                value={timeOfDay}
                onChange={(e) => setTimeOfDay(e.target.value)}
              />
            </Field>
          </>
        )}

        {triggerType === "interval" && (
          <Field label={t("events.intervalMinutes")}>
            <input
              type="number"
              className="input"
              min={1}
              max={1440}
              required
              value={intervalMinutes}
              onChange={(e) => setIntervalMinutes(Number(e.target.value))}
            />
          </Field>
        )}

        {triggerType === "once" && (
          <Field label={t("events.dateTime")}>
            <input
              type="datetime-local"
              className="input"
              required
              value={scheduledDate}
              onChange={(e) => setScheduledDate(e.target.value)}
            />
          </Field>
        )}

        <Field label={t("events.actionType")}>
          <select
            className="input"
            value={actionType}
            onChange={(e) => setActionType(e.target.value as ActionType)}
          >
            <option value="play_track">{t("events.actionTypes.play_track")}</option>
            <option value="play_category">{t("events.actionTypes.play_category")}</option>
            <option value="relay">{t("events.actionTypes.relay")}</option>
            <option value="load_clock">{t("events.actionTypes.load_clock")}</option>
          </select>
        </Field>

        {actionType === "play_track" && (
          <Field label={t("events.trackId")}>
            <input
              type="number"
              className="input"
              min={1}
              required
              value={trackId}
              onChange={(e) => setTrackId(Number(e.target.value))}
              placeholder="Track ID"
            />
          </Field>
        )}

        {actionType === "play_category" && (
          <Field label={t("events.category")}>
            <select
              className="input"
              value={categoryId}
              onChange={(e) => setCategoryId(Number(e.target.value))}
            >
              {categories.data?.map((cat) => (
                <option key={cat.id} value={cat.id}>
                  {cat.name}
                </option>
              ))}
            </select>
          </Field>
        )}

        {actionType === "relay" && (
          <Field label={t("events.relayUrl")}>
            <input
              type="url"
              className="input"
              required
              value={relayUrl}
              onChange={(e) => setRelayUrl(e.target.value)}
              placeholder="https://stream.example.com/live"
            />
          </Field>
        )}

        {actionType === "load_clock" && (
          <Field label={t("events.clock")}>
            <select
              className="input"
              value={clockId}
              onChange={(e) => setClockId(Number(e.target.value))}
            >
              {clocks.data?.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </Field>
        )}

        {save.isError && <ErrorView error={save.error} />}

        <div className="modal-actions">
          <button type="button" className="btn ghost" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button type="submit" className="btn primary" disabled={save.isPending}>
            {event ? t("events.updateButton") : t("events.createButton")}
          </button>
        </div>
      </form>
    </Modal>
  );
}

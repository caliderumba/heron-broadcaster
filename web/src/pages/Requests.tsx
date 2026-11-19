// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import {
  type ListenerRequestView,
  type RequestStatus,
  type StationRequestsSettings,
  api,
  unwrap,
} from "../api";
import {
  Card,
  ConfirmButton,
  Empty,
  Field,
  Icon,
  Modal,
  PageHead,
} from "../components/ui";
import { type PlainKey, t } from "../i18n";
import { ago, duration } from "../lib/format";
import { useAction } from "../lib/session";
import { here, useStation } from "../lib/station";

const STATUS_LABELS: Record<RequestStatus, PlainKey> = {
  pending: "requests.pending",
  approved: "requests.approved",
  rejected: "requests.rejected",
  played: "requests.played",
  expired: "requests.expired",
};

export function Requests() {
  const station = useStation();
  const client = useQueryClient();

  const [statusFilter, setStatusFilter] = useState<string>("all");
  const [settingsOpen, setSettingsOpen] = useState(false);

  // Queries
  const requestsQuery = useQuery({
    queryKey: ["station-requests", station, statusFilter],
    refetchInterval: 10_000,
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/requests", {
          params: {
            path: here(),
            ...(statusFilter !== "all"
              ? { query: { status: statusFilter as RequestStatus } }
              : {}),
          },
        }),
      ),
  });

  const settingsQuery = useQuery({
    queryKey: ["station-requests-settings", station],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/requests/settings", {
          params: { path: here() },
        }),
      ),
  });

  // Actions
  const approveAction = useAction(
    async (id: number) => {
      await unwrap(
        api.POST("/api/v1/stations/{station_id}/requests/{id}/approve", {
          params: { path: { ...here(), id } },
          body: { position: 0 },
        }),
      );
    },
    {
      onSuccess: () => {
        void client.invalidateQueries({ queryKey: ["station-requests", station] });
        void client.invalidateQueries({ queryKey: ["queue", station] });
      },
    },
  );

  const rejectAction = useAction(
    async (id: number) => {
      await unwrap(
        api.POST("/api/v1/stations/{station_id}/requests/{id}/reject", {
          params: { path: { ...here(), id } },
        }),
      );
    },
    {
      onSuccess: () => {
        void client.invalidateQueries({ queryKey: ["station-requests", station] });
      },
    },
  );

  const deleteAction = useAction(
    async (id: number) => {
      await unwrap(
        api.DELETE("/api/v1/stations/{station_id}/requests/{id}", {
          params: { path: { ...here(), id } },
        }),
      );
    },
    {
      onSuccess: () => {
        void client.invalidateQueries({ queryKey: ["station-requests", station] });
      },
    },
  );

  const requests = requestsQuery.data ?? [];
  const settings = settingsQuery.data;

  const publicPortalUrl = `${window.location.origin}/request`;

  return (
    <>
      <PageHead
        title={t("requests.title")}
        description={t("requests.description")}
        actions={
          <div className="row" style={{ gap: "0.5rem" }}>
            <a
              href={publicPortalUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="btn"
              style={{ textDecoration: "none" }}
            >
              <Icon name="link" size={16} /> {t("requests.openPortal")}
            </a>
            <button
              type="button"
              className="btn"
              onClick={() => setSettingsOpen(true)}
            >
              <Icon name="settings" size={16} /> {t("requests.settings")}
            </button>
          </div>
        }
      />

      {/* Status filter tabs */}
      <div className="segmented" style={{ marginBottom: "1.5rem" }}>
        {[
          { key: "all", label: t("requests.all") },
          { key: "pending", label: t("requests.pending") },
          { key: "approved", label: t("requests.approved") },
          { key: "rejected", label: t("requests.rejected") },
          { key: "played", label: t("requests.played") },
          { key: "expired", label: t("requests.expired") },
        ].map((tab) => (
          <button
            key={tab.key}
            type="button"
            className={statusFilter === tab.key ? "active" : ""}
            onClick={() => setStatusFilter(tab.key)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {requests.length === 0 ? (
        <Empty>{t("requests.empty")}</Empty>
      ) : (
        <div className="stack" style={{ gap: "1rem" }}>
          {requests.map((req) => (
            <RequestCard
              key={req.id}
              request={req}
              onApprove={() => approveAction.mutate(req.id)}
              onReject={() => rejectAction.mutate(req.id)}
              onDelete={() => deleteAction.mutate(req.id)}
              busy={
                approveAction.isPending ||
                rejectAction.isPending ||
                deleteAction.isPending
              }
            />
          ))}
        </div>
      )}

      {/* Settings Modal */}
      {settingsOpen && settings && (
        <SettingsModal
          settings={settings}
          onClose={() => setSettingsOpen(false)}
          onSaved={() => {
            setSettingsOpen(false);
            void client.invalidateQueries({
              queryKey: ["station-requests-settings", station],
            });
          }}
        />
      )}
    </>
  );
}

function RequestCard({
  request,
  onApprove,
  onReject,
  onDelete,
  busy,
}: Readonly<{
  request: ListenerRequestView;
  onApprove: () => void;
  onReject: () => void;
  onDelete: () => void;
  busy: boolean;
}>) {
  const statusColor: Record<RequestStatus, string> = {
    pending: "var(--warn)",
    approved: "var(--good)",
    rejected: "var(--bad)",
    played: "var(--muted)",
    expired: "var(--muted)",
  };

  return (
    <Card>
      <div
        className="row wrap space-between"
        style={{ alignItems: "flex-start", gap: "1rem" }}
      >
        <div style={{ flex: "1 1 300px" }}>
          <div className="row" style={{ gap: "0.5rem", alignItems: "center" }}>
            <span
              className="badge"
              style={{
                borderColor: statusColor[request.status],
                color: statusColor[request.status],
                fontSize: "0.75rem",
                textTransform: "uppercase",
                fontWeight: 600,
              }}
            >
              {t(STATUS_LABELS[request.status])}
            </span>
            <span className="muted small">
              {ago(request.requested_at_unix_ms)}
            </span>
          </div>

          <h3 style={{ margin: "0.4rem 0 0.2rem 0" }}>
            {request.title}{" "}
            <span className="muted" style={{ fontWeight: 400 }}>
              · {request.artist}
            </span>
          </h3>

          <div className="muted small" style={{ marginBottom: "0.5rem" }}>
            {duration(request.duration_ms)}
            {request.album ? ` · ${request.album}` : ""}
          </div>

          <div
            style={{
              padding: "0.5rem 0.75rem",
              background: "var(--surface-2)",
              borderRadius: "4px",
              marginTop: "0.5rem",
            }}
          >
            <div style={{ fontSize: "0.85rem" }}>
              <strong>{t("requests.listener")}:</strong> {request.listener_name}
            </div>
            {request.dedication && (
              <div
                style={{
                  fontSize: "0.85rem",
                  marginTop: "0.25rem",
                  fontStyle: "italic",
                }}
              >
                «{request.dedication}»
              </div>
            )}
          </div>
        </div>

        <div className="row" style={{ gap: "0.5rem", alignSelf: "center" }}>
          {request.status === "pending" && (
            <>
              <button
                type="button"
                className="btn primary"
                disabled={busy}
                onClick={onApprove}
              >
                <Icon name="check" size={16} /> {t("requests.approve")}
              </button>
              <button
                type="button"
                className="btn"
                disabled={busy}
                onClick={onReject}
              >
                <Icon name="close" size={16} /> {t("requests.reject")}
              </button>
            </>
          )}

          <ConfirmButton
            question={t("requests.deleteConfirm")}
            confirm="Delete"
            disabled={busy}
            onConfirm={onDelete}
          >
            <Icon name="trash" size={16} />
          </ConfirmButton>
        </div>
      </div>
    </Card>
  );
}

function SettingsModal({
  settings,
  onClose,
  onSaved,
}: Readonly<{
  settings: StationRequestsSettings;
  onClose: () => void;
  onSaved: () => void;
}>) {
  const [enabled, setEnabled] = useState(settings.enabled);
  const [cooldown, setCooldown] = useState(settings.cooldown_minutes);
  const [maxPending, setMaxPending] = useState(settings.max_pending);
  const [dailyCap, setDailyCap] = useState(settings.daily_cap);
  const [allowDedication, setAllowDedication] = useState(
    settings.allow_dedication,
  );
  const [maxDedicationLen, setMaxDedicationLen] = useState(
    settings.max_dedication_len,
  );
  const [title, setTitle] = useState(settings.title);
  const [subtitle, setSubtitle] = useState(settings.subtitle);

  const saveAction = useAction(
    async () => {
      await unwrap(
        api.PUT("/api/v1/stations/{station_id}/requests/settings", {
          params: { path: here() },
          body: {
            enabled,
            cooldown_minutes: cooldown,
            max_pending: maxPending,
            daily_cap: dailyCap,
            allow_dedication: allowDedication,
            max_dedication_len: maxDedicationLen,
            title,
            subtitle,
          },
        }),
      );
    },
    { onSuccess: onSaved },
  );

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    saveAction.mutate();
  };

  return (
    <Modal open={true} title={t("requests.settings")} onClose={onClose}>
      <form onSubmit={onSubmit} className="stack" style={{ gap: "1rem" }}>
        <Field label={t("requests.enabled")}>
          <input
            type="checkbox"
            checked={enabled}
            onChange={(e) => setEnabled(e.target.checked)}
          />
        </Field>

        <div className="row wrap" style={{ gap: "1rem" }}>
          <Field
            label={t("requests.cooldown")}
            hint={t("requests.cooldownHelp")}
          >
            <input
              type="number"
              className="input"
              min={0}
              value={cooldown}
              onChange={(e) => setCooldown(Number(e.target.value))}
            />
          </Field>

          <Field label={t("requests.maxPending")}>
            <input
              type="number"
              className="input"
              min={1}
              value={maxPending}
              onChange={(e) => setMaxPending(Number(e.target.value))}
            />
          </Field>
        </div>

        <div className="row wrap" style={{ gap: "1rem" }}>
          <Field label={t("requests.dailyCap")}>
            <input
              type="number"
              className="input"
              min={1}
              value={dailyCap}
              onChange={(e) => setDailyCap(Number(e.target.value))}
            />
          </Field>

          <Field label={t("requests.maxDedicationLen")}>
            <input
              type="number"
              className="input"
              min={10}
              max={1000}
              value={maxDedicationLen}
              onChange={(e) => setMaxDedicationLen(Number(e.target.value))}
            />
          </Field>
        </div>

        <Field label={t("requests.allowDedication")}>
          <input
            type="checkbox"
            checked={allowDedication}
            onChange={(e) => setAllowDedication(e.target.checked)}
          />
        </Field>

        <Field label={t("requests.portalTitle")}>
          <input
            type="text"
            className="input"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
        </Field>

        <Field label={t("requests.portalSubtitle")}>
          <input
            type="text"
            className="input"
            value={subtitle}
            onChange={(e) => setSubtitle(e.target.value)}
          />
        </Field>

        <div className="row end" style={{ gap: "0.5rem", marginTop: "1rem" }}>
          <button type="button" className="btn" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button
            type="submit"
            className="btn primary"
            disabled={saveAction.isPending}
          >
            {t("common.save")}
          </button>
        </div>
      </form>
    </Modal>
  );
}

// SPDX-License-Identifier: AGPL-3.0-or-later

// Advertising campaigns and commercial spots (M24).
// Manages commercial sponsor campaigns, flight dates, play quotas,
// spot rotation with weights, and audit-proof Proof of Play reporting.

import { useQuery } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import {
  type Campaign,
  type CampaignSpotDetail,
  type CampaignStatus,
  type CampaignWithSpots,
  type NextBreakResponse,
  type ProofOfPlayReport,
  api,
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
} from "../components/ui";
import { duration } from "../lib/format";
import { type PlainKey, t } from "../i18n";
import { useAction, useCan } from "../lib/session";
import { here, useStation } from "../lib/station";

const DAYS_OF_WEEK = [
  { bit: 1, label: "Mon" },
  { bit: 2, label: "Tue" },
  { bit: 4, label: "Wed" },
  { bit: 8, label: "Thu" },
  { bit: 16, label: "Fri" },
  { bit: 32, label: "Sat" },
  { bit: 64, label: "Sun" },
];

const FILTER_KEYS: Record<string, PlainKey> = {
  all: "campaigns.filter.all",
  active: "campaigns.filter.active",
  paused: "campaigns.filter.paused",
  completed: "campaigns.filter.completed",
  expired: "campaigns.filter.expired",
};

const STATUS_KEYS: Record<CampaignStatus, PlainKey> = {
  active: "campaigns.status.active",
  paused: "campaigns.status.paused",
  completed: "campaigns.status.completed",
  expired: "campaigns.status.expired",
};

export function Campaigns() {
  const canEdit = useCan("operator");
  const station = useStation();

  // State
  const [filterStatus, setFilterStatus] = useState<string>("all");
  const [editingCampaign, setEditingCampaign] = useState<Campaign | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [spotsCampaignId, setSpotsCampaignId] = useState<number | null>(null);
  const [proofCampaignId, setProofCampaignId] = useState<number | null>(null);
  const [showNextBreak, setShowNextBreak] = useState(false);

  // Queries
  const campaignsQuery = useQuery({
    queryKey: ["campaigns", station, filterStatus],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/campaigns", {
          params: {
            path: here(),
            ...(filterStatus !== "all" ? { query: { status: filterStatus as CampaignStatus } } : {}),
          },
        }),
      ),
  });

  const spotsQuery = useQuery({
    queryKey: ["campaign-spots", station, spotsCampaignId],
    enabled: spotsCampaignId !== null,
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/campaigns/{id}", {
          params: {
            path: { ...here(), id: spotsCampaignId! },
          },
        }),
      ),
  });

  const proofQuery = useQuery({
    queryKey: ["campaign-proof", station, proofCampaignId],
    enabled: proofCampaignId !== null,
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/campaigns/{id}/proof-of-play", {
          params: {
            path: { ...here(), id: proofCampaignId! },
            query: {},
          },
        }),
      ),
  });

  const nextBreakQuery = useQuery({
    queryKey: ["campaign-next-break", station],
    enabled: showNextBreak,
    queryFn: () =>
      unwrap(
        api.POST("/api/v1/stations/{station_id}/campaigns/next-break", {
          params: { path: here() },
          body: { max_spots: 4 },
        }),
      ),
  });

  // Mutations
  const createMutation = useAction(
    (body: any) =>
      unwrap(
        api.POST("/api/v1/stations/{station_id}/campaigns", {
          params: { path: here() },
          body,
        }),
      ),
    {
      onSuccess: () => {
        setIsCreating(false);
        campaignsQuery.refetch();
      },
    },
  );

  const updateMutation = useAction(
    ({ id, body }: { id: number; body: any }) =>
      unwrap(
        api.PUT("/api/v1/stations/{station_id}/campaigns/{id}", {
          params: { path: { ...here(), id } },
          body,
        }),
      ),
    {
      onSuccess: () => {
        setEditingCampaign(null);
        campaignsQuery.refetch();
      },
    },
  );

  const deleteMutation = useAction(
    (id: number) =>
      unwrap(
        api.DELETE("/api/v1/stations/{station_id}/campaigns/{id}", {
          params: { path: { ...here(), id } },
        }),
      ),
    {
      onSuccess: () => {
        campaignsQuery.refetch();
      },
    },
  );

  const addSpotMutation = useAction(
    ({ campaignId, trackId, weight }: { campaignId: number; trackId: number; weight: number }) =>
      unwrap(
        api.POST("/api/v1/stations/{station_id}/campaigns/{id}/spots", {
          params: { path: { ...here(), id: campaignId } },
          body: { track_id: trackId, weight, enabled: true },
        }),
      ),
    {
      onSuccess: () => {
        spotsQuery.refetch();
        campaignsQuery.refetch();
      },
    },
  );

  const deleteSpotMutation = useAction(
    ({ campaignId, spotId }: { campaignId: number; spotId: number }) =>
      unwrap(
        api.DELETE("/api/v1/stations/{station_id}/campaigns/{id}/spots/{spot_id}", {
          params: { path: { ...here(), id: campaignId, spot_id: spotId } },
        }),
      ),
    {
      onSuccess: () => {
        spotsQuery.refetch();
        campaignsQuery.refetch();
      },
    },
  );

  return (
    <div className="stack gap-large">
      <div className="row split wrap">
        <div>
          <h2>{t("campaigns.title")}</h2>
          <p className="muted small">{t("campaigns.subtitle")}</p>
        </div>
        <div className="row gap-small">
          <button
            type="button"
            className="subtle"
            onClick={() => {
              setShowNextBreak(true);
              nextBreakQuery.refetch();
            }}
          >
            <Icon name="play" size={16} />
            {t("campaigns.actions.previewBreak")}
          </button>
          {canEdit && (
            <button
              type="button"
              className="primary"
              onClick={() => setIsCreating(true)}
            >
              <Icon name="plus" size={16} />
              {t("campaigns.actions.newCampaign")}
            </button>
          )}
        </div>
      </div>

      {/* Status Filter Tabs */}
      <div className="row gap-small border-b pb-2">
        {["all", "active", "paused", "completed", "expired"].map((status) => (
          <button
            key={status}
            type="button"
            onClick={() => setFilterStatus(status)}
            className={`subtle small ${filterStatus === status ? "active font-bold" : ""}`}
          >
            {t(FILTER_KEYS[status] ?? "campaigns.filter.all")}
          </button>
        ))}
      </div>

      {/* Campaigns Listing */}
      {campaignsQuery.isLoading ? (
        <Loading />
      ) : campaignsQuery.isError ? (
        <ErrorView error={campaignsQuery.error} />
      ) : campaignsQuery.data?.length === 0 ? (
        <Empty>{t("campaigns.empty")}</Empty>
      ) : (
        <div className="grid">
          {campaignsQuery.data?.map((camp) => {
            const totalTarget = camp.total_target;
            const delivered = camp.delivered_plays;
            const progressPct =
              totalTarget > 0 ? Math.min(100, Math.round((delivered / totalTarget) * 100)) : 100;

            return (
              <Card key={camp.id}>
                <div className="stack gap-medium">
                  <div className="row split items-start">
                    <div>
                      <h3 className="font-bold">{camp.name}</h3>
                      <p className="small muted font-medium">{camp.client_name}</p>
                    </div>
                    <span className="badge small">
                      {t(STATUS_KEYS[camp.status] ?? "campaigns.status.active")}
                    </span>
                  </div>

                  <div className="stack gap-small small muted">
                    <div className="row split">
                      <span>{t("campaigns.fields.dates")}:</span>
                      <span className="font-mono text-foreground">
                        {camp.start_date} → {camp.end_date}
                      </span>
                    </div>
                    <div className="row split">
                      <span>{t("campaigns.fields.priority")}:</span>
                      <span>{camp.priority} / 10</span>
                    </div>
                    <div className="row split">
                      <span>{t("campaigns.fields.spots")}:</span>
                      <span>{camp.spots_count}</span>
                    </div>

                    {/* Progress Bar */}
                    <div className="stack gap-tiny pt-1">
                      <div className="row split small">
                        <span>{t("campaigns.fields.delivered")}:</span>
                        <span className="font-mono">
                          {delivered} / {totalTarget > 0 ? totalTarget : "∞"} ({progressPct}%)
                        </span>
                      </div>
                      <div className="progress-bar">
                        <div
                          className="progress-fill"
                          style={{ width: `${progressPct}%` }}
                        />
                      </div>
                    </div>
                  </div>

                  <div className="row split pt-2 border-t">
                    <div className="row gap-small">
                      <button
                        type="button"
                        className="subtle small"
                        onClick={() => setSpotsCampaignId(camp.id)}
                      >
                        <Icon name="tag" size={14} />
                        {t("campaigns.actions.spots")}
                      </button>
                      <button
                        type="button"
                        className="subtle small"
                        onClick={() => setProofCampaignId(camp.id)}
                      >
                        <Icon name="check" size={14} />
                        {t("campaigns.actions.proof")}
                      </button>
                    </div>
                    {canEdit && (
                      <div className="row gap-tiny">
                        <button
                          type="button"
                          className="subtle icon-only"
                          onClick={() => setEditingCampaign(camp)}
                          title={t("campaigns.actions.edit")}
                        >
                          <Icon name="edit" size={14} />
                        </button>
                        <ConfirmButton
                          question={t("campaigns.confirmDelete")}
                          confirm="Delete"
                          onConfirm={() => deleteMutation.mutate(camp.id)}
                        >
                          <Icon name="trash" size={14} />
                        </ConfirmButton>
                      </div>
                    )}
                  </div>
                </div>
              </Card>
            );
          })}
        </div>
      )}

      {/* New / Edit Campaign Modal */}
      {(isCreating || editingCampaign) && (
        <CampaignFormModal
          campaign={editingCampaign}
          onClose={() => {
            setIsCreating(false);
            setEditingCampaign(null);
          }}
          onSubmit={(values) => {
            if (editingCampaign) {
              updateMutation.mutate({ id: editingCampaign.id, body: values });
            } else {
              createMutation.mutate(values);
            }
          }}
          isSubmitting={createMutation.isPending || updateMutation.isPending}
        />
      )}

      {/* Spots Management Modal */}
      {spotsCampaignId !== null && (
        <SpotsModal
          campaignWithSpots={spotsQuery.data}
          isLoading={spotsQuery.isLoading}
          onClose={() => setSpotsCampaignId(null)}
          onAddSpot={(trackId, weight) =>
            addSpotMutation.mutate({ campaignId: spotsCampaignId, trackId, weight })
          }
          onDeleteSpot={(spotId) =>
            deleteSpotMutation.mutate({ campaignId: spotsCampaignId, spotId })
          }
          canEdit={canEdit}
        />
      )}

      {/* Proof of Play Modal */}
      {proofCampaignId !== null && (
        <ProofOfPlayModal
          report={proofQuery.data}
          isLoading={proofQuery.isLoading}
          onClose={() => setProofCampaignId(null)}
        />
      )}

      {/* Next Break Preview Modal */}
      {showNextBreak && (
        <NextBreakModal
          nextBreak={nextBreakQuery.data}
          isLoading={nextBreakQuery.isLoading}
          onClose={() => setShowNextBreak(false)}
        />
      )}
    </div>
  );
}

function CampaignFormModal({
  campaign,
  onClose,
  onSubmit,
  isSubmitting,
}: {
  campaign: Campaign | null;
  onClose: () => void;
  onSubmit: (values: any) => void;
  isSubmitting: boolean;
}) {
  const [name, setName] = useState(campaign?.name ?? "");
  const [clientName, setClientName] = useState(campaign?.client_name ?? "");
  const [status, setStatus] = useState<CampaignStatus>(campaign?.status ?? "active");
  const [startDate, setStartDate] = useState(
    campaign?.start_date ?? new Date().toISOString().slice(0, 10),
  );
  const [endDate, setEndDate] = useState(
    campaign?.end_date ??
      new Date(Date.now() + 30 * 86400000).toISOString().slice(0, 10),
  );
  const [dailyTarget, setDailyTarget] = useState(campaign?.daily_target ?? 5);
  const [totalTarget, setTotalTarget] = useState(campaign?.total_target ?? 100);
  const [priority, setPriority] = useState(campaign?.priority ?? 5);
  const [daysMask, setDaysMask] = useState(campaign?.days_mask ?? 127);
  const [startHour, setStartHour] = useState(campaign?.start_hour ?? 6);
  const [endHour, setEndHour] = useState(campaign?.end_hour ?? 22);

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    onSubmit({
      name,
      client_name: clientName,
      status,
      start_date: startDate,
      end_date: endDate,
      daily_target: Number(dailyTarget),
      total_target: Number(totalTarget),
      priority: Number(priority),
      days_mask: Number(daysMask),
      start_hour: Number(startHour),
      end_hour: Number(endHour),
    });
  };

  const toggleDay = (bit: number) => {
    setDaysMask((prev) => (prev & bit ? prev & ~bit : prev | bit));
  };

  return (
    <Modal
      open={true}
      title={campaign ? t("campaigns.actions.edit") : t("campaigns.actions.newCampaign")}
      onClose={onClose}
    >
      <form onSubmit={handleSubmit} className="stack gap-medium">
        <div className="row gap-medium">
          <Field label={t("campaigns.fields.name")}>
            <input
              type="text"
              className="input"
              required
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Summer Promo"
            />
          </Field>
          <Field label={t("campaigns.fields.clientName")}>
            <input
              type="text"
              className="input"
              required
              value={clientName}
              onChange={(e) => setClientName(e.target.value)}
              placeholder="e.g. Acme Corp"
            />
          </Field>
        </div>

        <div className="row gap-medium">
          <Field label={t("campaigns.fields.status")}>
            <select
              className="input"
              value={status}
              onChange={(e) => setStatus(e.target.value as CampaignStatus)}
            >
              <option value="active">{t("campaigns.status.active")}</option>
              <option value="paused">{t("campaigns.status.paused")}</option>
              <option value="completed">{t("campaigns.status.completed")}</option>
              <option value="expired">{t("campaigns.status.expired")}</option>
            </select>
          </Field>
          <Field label={t("campaigns.fields.startDate")}>
            <input
              type="date"
              className="input"
              required
              value={startDate}
              onChange={(e) => setStartDate(e.target.value)}
            />
          </Field>
          <Field label={t("campaigns.fields.endDate")}>
            <input
              type="date"
              className="input"
              required
              value={endDate}
              onChange={(e) => setEndDate(e.target.value)}
            />
          </Field>
        </div>

        <div className="row gap-medium">
          <Field label={t("campaigns.fields.dailyTarget")}>
            <input
              type="number"
              className="input"
              min="0"
              value={dailyTarget}
              onChange={(e) => setDailyTarget(Number(e.target.value))}
            />
          </Field>
          <Field label={t("campaigns.fields.totalTarget")}>
            <input
              type="number"
              className="input"
              min="0"
              value={totalTarget}
              onChange={(e) => setTotalTarget(Number(e.target.value))}
            />
          </Field>
          <Field label={t("campaigns.fields.priority")}>
            <input
              type="number"
              className="input"
              min="1"
              max="10"
              value={priority}
              onChange={(e) => setPriority(Number(e.target.value))}
            />
          </Field>
        </div>

        {/* Day of Week Filter */}
        <Field label={t("campaigns.fields.activeDays")}>
          <div className="row gap-tiny wrap">
            {DAYS_OF_WEEK.map((day) => {
              const active = (daysMask & day.bit) !== 0;
              return (
                <button
                  type="button"
                  key={day.bit}
                  onClick={() => toggleDay(day.bit)}
                  className={`subtle small ${active ? "active font-bold" : ""}`}
                >
                  {day.label}
                </button>
              );
            })}
          </div>
        </Field>

        {/* Hourly Window */}
        <div className="row gap-medium">
          <Field label={t("campaigns.fields.startHour")}>
            <input
              type="number"
              className="input"
              min="0"
              max="23"
              value={startHour}
              onChange={(e) => setStartHour(Number(e.target.value))}
            />
          </Field>
          <Field label={t("campaigns.fields.endHour")}>
            <input
              type="number"
              className="input"
              min="1"
              max="24"
              value={endHour}
              onChange={(e) => setEndHour(Number(e.target.value))}
            />
          </Field>
        </div>

        <div className="row split pt-2 border-t">
          <button type="button" className="subtle" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button type="submit" className="primary" disabled={isSubmitting}>
            {t("common.save")}
          </button>
        </div>
      </form>
    </Modal>
  );
}

function SpotsModal({
  campaignWithSpots,
  isLoading,
  onClose,
  onAddSpot,
  onDeleteSpot,
  canEdit,
}: {
  campaignWithSpots: CampaignWithSpots | undefined;
  isLoading: boolean;
  onClose: () => void;
  onAddSpot: (trackId: number, weight: number) => void;
  onDeleteSpot: (spotId: number) => void;
  canEdit: boolean;
}) {
  const [trackId, setTrackId] = useState("");
  const [weight, setWeight] = useState(100);

  const handleAdd = (e: FormEvent) => {
    e.preventDefault();
    if (!trackId) return;
    onAddSpot(Number(trackId), Number(weight));
    setTrackId("");
    setWeight(100);
  };

  return (
    <Modal
      open={true}
      title={`${t("campaigns.spots.title")}: ${campaignWithSpots?.campaign.name ?? ""}`}
      onClose={onClose}
    >
      <div className="stack gap-medium">
        {isLoading ? (
          <Loading />
        ) : (
          <>
            {/* Spots list */}
            <div className="stack gap-small">
              <h4>{t("campaigns.spots.assignedSpots")}</h4>
              {campaignWithSpots?.spots.length === 0 ? (
                <p className="small muted text-center p-3">
                  {t("campaigns.spots.empty")}
                </p>
              ) : (
                <div className="stack divide-y border rounded">
                  {campaignWithSpots?.spots.map((spot: CampaignSpotDetail) => (
                    <div key={spot.id} className="row split p-2 small">
                      <div>
                        <div className="font-bold">{spot.title}</div>
                        <div className="muted">
                          {spot.artist} • {duration(spot.duration_ms)} • {t("campaigns.fields.weight")}: {spot.weight}
                        </div>
                      </div>
                      {canEdit && (
                        <button
                          type="button"
                          className="subtle icon-only"
                          onClick={() => onDeleteSpot(spot.id)}
                        >
                          <Icon name="trash" size={14} />
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Add spot form */}
            {canEdit && (
              <form onSubmit={handleAdd} className="stack gap-small p-3 border rounded bg-muted/20">
                <h4>{t("campaigns.spots.addTitle")}</h4>
                <div className="row gap-medium">
                  <Field label={t("campaigns.spots.trackId")}>
                    <input
                      required
                      type="number"
                      className="input"
                      min="1"
                      placeholder="e.g. 104"
                      value={trackId}
                      onChange={(e) => setTrackId(e.target.value)}
                    />
                  </Field>
                  <Field label={t("campaigns.fields.weight")}>
                    <input
                      type="number"
                      className="input"
                      min="1"
                      max="100"
                      value={weight}
                      onChange={(e) => setWeight(Number(e.target.value))}
                    />
                  </Field>
                </div>
                <div className="row split pt-1">
                  <span />
                  <button type="submit" className="primary small">
                    <Icon name="plus" size={14} />
                    {t("campaigns.spots.add")}
                  </button>
                </div>
              </form>
            )}
          </>
        )}

        <div className="row split pt-2 border-t">
          <span />
          <button type="button" className="subtle" onClick={onClose}>
            {t("common.cancel")}
          </button>
        </div>
      </div>
    </Modal>
  );
}

function ProofOfPlayModal({
  report,
  isLoading,
  onClose,
}: {
  report: ProofOfPlayReport | undefined;
  isLoading: boolean;
  onClose: () => void;
}) {
  return (
    <Modal open={true} title={t("campaigns.proof.title")} onClose={onClose}>
      <div className="stack gap-medium">
        {isLoading || !report ? (
          <Loading />
        ) : (
          <>
            <div className="p-3 border rounded stack gap-small">
              <div className="row split">
                <span className="font-bold">{report.campaign_name}</span>
                <span className="small muted">{report.client_name}</span>
              </div>
              <div className="row split pt-2 border-t small">
                <div>
                  <div className="muted">{t("campaigns.fields.delivered")}</div>
                  <div className="font-bold text-lg">{report.delivered_plays}</div>
                </div>
                <div>
                  <div className="muted">{t("campaigns.proof.fulfillment")}</div>
                  <div className="font-bold text-lg">
                    {Math.round(report.fulfillment_percent)}%
                  </div>
                </div>
                <div>
                  <div className="muted">{t("campaigns.proof.totalAirtime")}</div>
                  <div className="font-bold text-lg font-mono">
                    {duration(report.total_broadcast_ms)}
                  </div>
                </div>
              </div>
            </div>

            {/* Itemized plays table */}
            <div className="stack gap-small">
              <h4>{t("campaigns.proof.playLog")}</h4>
              {report.plays.length === 0 ? (
                <p className="small muted text-center p-3">
                  {t("campaigns.proof.noPlays")}
                </p>
              ) : (
                <div className="stack divide-y border rounded max-h-60 overflow-y-auto">
                  {report.plays.map((play) => (
                    <div key={play.id} className="row split p-2 small">
                      <div>
                        <div className="font-bold">{play.title}</div>
                        <div className="muted">{play.artist}</div>
                      </div>
                      <div className="text-right font-mono muted">
                        <div>{new Date(play.played_at_unix_ms).toLocaleString()}</div>
                        <div>{duration(play.duration_ms)}</div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </>
        )}

        <div className="row split pt-2 border-t">
          <span />
          <button type="button" className="subtle" onClick={onClose}>
            {t("common.cancel")}
          </button>
        </div>
      </div>
    </Modal>
  );
}

function NextBreakModal({
  nextBreak,
  isLoading,
  onClose,
}: {
  nextBreak: NextBreakResponse | undefined;
  isLoading: boolean;
  onClose: () => void;
}) {
  return (
    <Modal open={true} title={t("campaigns.break.title")} onClose={onClose}>
      <div className="stack gap-medium">
        {isLoading || !nextBreak ? (
          <Loading />
        ) : (
          <>
            <div className="row split p-3 border rounded small">
              <span>{t("campaigns.break.totalDuration")}:</span>
              <span className="font-mono font-bold">
                {duration(nextBreak.total_duration_ms)}
              </span>
            </div>

            <div className="stack gap-small">
              <h4>{t("campaigns.break.spotsInBlock")}</h4>
              {nextBreak.spots.length === 0 ? (
                <p className="small muted text-center p-3">
                  {t("campaigns.break.empty")}
                </p>
              ) : (
                <div className="stack divide-y border rounded">
                  {nextBreak.spots.map((spot, idx) => (
                    <div key={idx} className="row split p-2.5 small">
                      <div>
                        <div className="font-bold">{spot.title}</div>
                        <div className="muted">
                          {spot.artist} • <span className="font-medium">{spot.campaign_name}</span>
                        </div>
                      </div>
                      <div className="font-mono muted">
                        {duration(spot.duration_ms)}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </>
        )}

        <div className="row split pt-2 border-t">
          <span />
          <button type="button" className="subtle" onClick={onClose}>
            {t("common.cancel")}
          </button>
        </div>
      </div>
    </Modal>
  );
}

// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import {
  type PublicSearchTrack,
  type SubmitRequestPayload,
  type SubmitRequestResponse,
  api,
  errorMessage,
  unwrap,
} from "../api";
import {
  Banner,
  Card,
  Empty,
  Field,
  Icon,
  Loading,
  Logo,
  Modal,
} from "../components/ui";
import { t } from "../i18n";
import { duration } from "../lib/format";

const CLIENT_TOKEN_KEY = "heron_listener_token";

function getClientToken(): string {
  try {
    let token = localStorage.getItem(CLIENT_TOKEN_KEY);
    if (!token) {
      const bytes = new Uint8Array(16);
      crypto.getRandomValues(bytes);
      token = Array.from(bytes)
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");
      localStorage.setItem(CLIENT_TOKEN_KEY, token);
    }
    return token;
  } catch {
    return "anon-listener";
  }
}

export function PublicRequest({
  stationCode,
}: Readonly<{ stationCode?: string | undefined }>) {
  const [searchTerm, setSearchTerm] = useState("");
  const [submittedQuery, setSubmittedQuery] = useState("");

  const [selectedTrack, setSelectedTrack] = useState<PublicSearchTrack | null>(
    null,
  );
  const [listenerName, setListenerName] = useState("");
  const [dedication, setDedication] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [successInfo, setSuccessInfo] = useState<{
    title: string;
    artist: string;
  } | null>(null);

  // Queries
  const settingsQuery = useQuery({
    queryKey: ["public-requests-settings", stationCode],
    queryFn: () => {
      if (stationCode) {
        return unwrap(
          api.GET("/api/v1/public/stations/{code}/requests/settings", {
            params: { path: { code: stationCode } },
          }),
        );
      }
      return unwrap(api.GET("/api/v1/public/requests/settings"));
    },
  });

  const searchQuery = useQuery({
    queryKey: ["public-requests-search", stationCode, submittedQuery],
    enabled: submittedQuery.trim().length > 0,
    queryFn: () => {
      if (stationCode) {
        return unwrap(
          api.GET("/api/v1/public/stations/{code}/requests/search", {
            params: {
              path: { code: stationCode },
              query: { q: submittedQuery.trim() },
            },
          }),
        );
      }
      return unwrap(
        api.GET("/api/v1/public/requests/search", {
          params: {
            query: { q: submittedQuery.trim() },
          },
        }),
      );
    },
  });

  const handleSearchSubmit = (e: FormEvent) => {
    e.preventDefault();
    setSubmittedQuery(searchTerm);
  };

  const handleRequestSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!selectedTrack || !listenerName.trim()) return;

    setIsSubmitting(true);
    setSubmitError(null);

    try {
      const payload: SubmitRequestPayload = {
        track_id: selectedTrack.id,
        listener_name: listenerName.trim(),
        dedication: dedication.trim() || null,
        client_token: getClientToken(),
      };

      let res: SubmitRequestResponse;
      if (stationCode) {
        res = await unwrap(
          api.POST("/api/v1/public/stations/{code}/requests", {
            params: { path: { code: stationCode } },
            body: payload,
          }),
        );
      } else {
        res = await unwrap(
          api.POST("/api/v1/public/requests", {
            body: payload,
          }),
        );
      }

      try {
        localStorage.setItem(CLIENT_TOKEN_KEY, res.client_token);
      } catch {
        // ignore
      }

      setSuccessInfo({
        title: selectedTrack.title,
        artist: selectedTrack.artist,
      });
      setSelectedTrack(null);
      setDedication("");
      void searchQuery.refetch();
    } catch (err) {
      setSubmitError(errorMessage(err));
    } finally {
      setIsSubmitting(false);
    }
  };

  if (settingsQuery.isPending) {
    return <Loading label={t("ui.loading")} />;
  }

  if (settingsQuery.isError) {
    return (
      <main
        style={{ maxWidth: "700px", margin: "3rem auto", padding: "0 1rem" }}
      >
        <Banner kind="bad">{errorMessage(settingsQuery.error)}</Banner>
      </main>
    );
  }

  const settings = settingsQuery.data;

  return (
    <main
      style={{ maxWidth: "760px", margin: "2rem auto", padding: "0 1rem" }}
    >
      <header
        className="row"
        style={{
          alignItems: "center",
          gap: "1rem",
          marginBottom: "2rem",
          paddingBottom: "1rem",
          borderBottom: "1px solid var(--border)",
        }}
      >
        <Logo size={40} />
        <div>
          <h1 style={{ margin: 0, fontSize: "1.5rem" }}>
            {settings.title || t("publicRequest.title")}
          </h1>
          <p className="muted" style={{ margin: "0.25rem 0 0 0" }}>
            {settings.subtitle || t("publicRequest.subtitle")}
          </p>
        </div>
      </header>

      {!settings.enabled ? (
        <Banner kind="warn">{t("publicRequest.disabled")}</Banner>
      ) : (
        <>
          {successInfo && (
            <div
              className="card"
              style={{
                borderLeft: "4px solid var(--good)",
                background: "var(--surface-2)",
                marginBottom: "1.5rem",
                padding: "1rem 1.25rem",
              }}
            >
              <h3
                style={{
                  margin: "0 0 0.5rem 0",
                  color: "var(--good)",
                  display: "flex",
                  alignItems: "center",
                  gap: "0.5rem",
                }}
              >
                <Icon name="check" size={20} />
                {t("publicRequest.successTitle")}
              </h3>
              <p style={{ margin: "0 0 0.75rem 0" }}>
                {t("publicRequest.successMessage", {
                  title: successInfo.title,
                  artist: successInfo.artist,
                })}
              </p>
              <button
                type="button"
                className="btn"
                onClick={() => setSuccessInfo(null)}
              >
                {t("publicRequest.anotherRequest")}
              </button>
            </div>
          )}

          {/* Search box */}
          <form
            onSubmit={handleSearchSubmit}
            className="row"
            style={{ gap: "0.5rem", marginBottom: "1.5rem" }}
          >
            <input
              type="text"
              className="input"
              style={{ flex: "1 1 auto" }}
              placeholder={t("publicRequest.searchPlaceholder")}
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
            />
            <button
              type="submit"
              className="btn primary"
              disabled={searchQuery.isFetching || !searchTerm.trim()}
            >
              <Icon name="search" size={16} />
              {t("publicRequest.searchButton")}
            </button>
          </form>

          {/* Search results */}
          {searchQuery.isFetching && <Loading label={t("publicRequest.searching")} />}

          {searchQuery.data && searchQuery.data.length === 0 && (
            <Empty>{t("publicRequest.noResults")}</Empty>
          )}

          {searchQuery.data && searchQuery.data.length > 0 && (
            <div className="stack" style={{ gap: "0.75rem" }}>
              {searchQuery.data.map((track) => (
                <Card key={track.id}>
                  <div
                    className="row wrap space-between"
                    style={{ alignItems: "center", gap: "1rem" }}
                  >
                    <div style={{ flex: "1 1 300px" }}>
                      <h3 style={{ margin: "0 0 0.2rem 0" }}>
                        {track.title}{" "}
                        <span className="muted" style={{ fontWeight: 400 }}>
                          · {track.artist}
                        </span>
                      </h3>
                      <div className="muted small">
                        {duration(track.duration_ms)}
                        {track.album ? ` · ${track.album}` : ""}
                      </div>
                    </div>

                    <div>
                      {track.can_request ? (
                        <button
                          type="button"
                          className="btn primary"
                          onClick={() => {
                            setSelectedTrack(track);
                            setSubmitError(null);
                          }}
                        >
                          <Icon name="heart" size={16} />
                          {t("publicRequest.requestButton")}
                        </button>
                      ) : (
                        <span
                          className="badge"
                          style={{
                            borderColor: "var(--muted)",
                            color: "var(--muted)",
                            fontSize: "0.75rem",
                          }}
                        >
                          {t("publicRequest.cooldown", {
                            time: track.cooldown_until_unix_ms
                              ? new Date(
                                  track.cooldown_until_unix_ms,
                                ).toLocaleTimeString([], {
                                  hour: "2-digit",
                                  minute: "2-digit",
                                })
                              : "later",
                          })}
                        </span>
                      )}
                    </div>
                  </div>
                </Card>
              ))}
            </div>
          )}

          {/* Request Modal */}
          {selectedTrack && (
            <Modal
              open={Boolean(selectedTrack)}
              title={t("publicRequest.modalTitle", {
                title: selectedTrack.title,
                artist: selectedTrack.artist,
              })}
              onClose={() => setSelectedTrack(null)}
            >
              <form
                onSubmit={handleRequestSubmit}
                className="stack"
                style={{ gap: "1rem" }}
              >
                {submitError && <Banner kind="bad">{submitError}</Banner>}

                <Field label={t("publicRequest.yourName")}>
                  <input
                    type="text"
                    required
                    maxLength={100}
                    className="input"
                    placeholder={t("publicRequest.namePlaceholder")}
                    value={listenerName}
                    onChange={(e) => setListenerName(e.target.value)}
                    autoFocus
                  />
                </Field>

                {settings.allow_dedication && (
                  <Field label={t("publicRequest.dedication")}>
                    <textarea
                      className="input"
                      rows={3}
                      maxLength={settings.max_dedication_len}
                      placeholder={t("publicRequest.dedicationPlaceholder")}
                      value={dedication}
                      onChange={(e) => setDedication(e.target.value)}
                    />
                    <div
                      className="muted small end"
                      style={{ marginTop: "0.25rem", textAlign: "right" }}
                    >
                      {dedication.length} / {settings.max_dedication_len}
                    </div>
                  </Field>
                )}

                <div
                  className="row end"
                  style={{ gap: "0.5rem", marginTop: "1rem" }}
                >
                  <button
                    type="button"
                    className="btn"
                    onClick={() => setSelectedTrack(null)}
                  >
                    {t("common.cancel")}
                  </button>
                  <button
                    type="submit"
                    className="btn primary"
                    disabled={isSubmitting || !listenerName.trim()}
                  >
                    {isSubmitting ? (
                      t("publicRequest.submitting")
                    ) : (
                      <>
                        <Icon name="heart" size={16} />
                        {t("publicRequest.submitButton")}
                      </>
                    )}
                  </button>
                </div>
              </form>
            </Modal>
          )}
        </>
      )}
    </main>
  );
}

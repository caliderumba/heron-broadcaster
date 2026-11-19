// SPDX-License-Identifier: AGPL-3.0-or-later

// Spoken time and weather announcements (M21).
// Controls modular time voice announcements (SayTime) and Open-Meteo weather reports.

import { useQuery } from "@tanstack/react-query";
import { type FormEvent, useEffect, useState } from "react";
import {
  type AnnouncementPreview,
  type TimeFormat,
  type WeatherUnits,
  api,
  unwrap,
} from "../api";
import {
  Card,
  ErrorView,
  Field,
  Icon,
  Loading,
  PageHead,
} from "../components/ui";
import { t } from "../i18n";
import { useAction, useCan } from "../lib/session";
import { here, useStation } from "../lib/station";

export function Announcements() {
  const canEdit = useCan("operator");
  const station = useStation();

  // Queries
  const timeQuery = useQuery({
    queryKey: ["announcements-time", station],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/announcements/time", {
          params: { path: here() },
        }),
      ),
  });

  const weatherQuery = useQuery({
    queryKey: ["announcements-weather", station],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/announcements/weather", {
          params: { path: here() },
        }),
      ),
  });

  const weatherStatusQuery = useQuery({
    queryKey: ["announcements-weather-status", station],
    queryFn: () =>
      unwrap(
        api.GET("/api/v1/stations/{station_id}/announcements/weather/status", {
          params: { path: here() },
        }),
      ),
  });

  // Time state
  const [timeEnabled, setTimeEnabled] = useState(false);
  const [timeFormat, setTimeFormat] = useState<TimeFormat>("h24");
  const [voicePack, setVoicePack] = useState("default");
  const [timePreview, setTimePreview] = useState<AnnouncementPreview | null>(null);

  useEffect(() => {
    if (timeQuery.data) {
      setTimeEnabled(Boolean(timeQuery.data.enabled));
      setTimeFormat(timeQuery.data.format ?? "h24");
      setVoicePack(timeQuery.data.voice_pack ?? "");
    }
  }, [timeQuery.data]);

  // Weather state
  const [weatherEnabled, setWeatherEnabled] = useState(false);
  const [city, setCity] = useState("");
  const [latitude, setLatitude] = useState<string>("");
  const [longitude, setLongitude] = useState<string>("");
  const [units, setUnits] = useState<WeatherUnits>("metric");
  const [weatherPreview, setWeatherPreview] = useState<AnnouncementPreview | null>(null);

  useEffect(() => {
    if (weatherQuery.data) {
      setWeatherEnabled(Boolean(weatherQuery.data.enabled));
      setCity(weatherQuery.data.city ?? "");
      setLatitude(weatherQuery.data.latitude?.toString() ?? "");
      setLongitude(weatherQuery.data.longitude?.toString() ?? "");
      setUnits(weatherQuery.data.units ?? "metric");
    }
  }, [weatherQuery.data]);

  // Actions
  const saveTime = useAction(
    async () => {
      await unwrap(
        api.PUT("/api/v1/stations/{station_id}/announcements/time", {
          params: { path: here() },
          body: {
            enabled: timeEnabled,
            format: timeFormat,
            voice_pack: voicePack,
          },
        }),
      );
    },
    {
      success: t("announcements.timeSaved"),
      invalidate: [["announcements-time", station]],
    },
  );

  const previewTimeAction = useAction(async () => {
    const preview = await unwrap(
      api.POST("/api/v1/stations/{station_id}/announcements/time/preview", {
        params: { path: here() },
      }),
    );
    setTimePreview(preview);
  });

  const saveWeather = useAction(
    async () => {
      const lat = latitude ? parseFloat(latitude) : null;
      const lon = longitude ? parseFloat(longitude) : null;
      await unwrap(
        api.PUT("/api/v1/stations/{station_id}/announcements/weather", {
          params: { path: here() },
          body: {
            enabled: weatherEnabled,
            city,
            latitude: lat,
            longitude: lon,
            units,
          },
        }),
      );
    },
    {
      success: t("announcements.weatherSaved"),
      invalidate: [["announcements-weather", station]],
    },
  );

  const refreshWeatherAction = useAction(
    async () => {
      await unwrap(
        api.POST("/api/v1/stations/{station_id}/announcements/weather/refresh", {
          params: { path: here() },
        }),
      );
    },
    {
      success: t("announcements.weatherRefreshed"),
      invalidate: [["announcements-weather-status", station]],
    },
  );

  const previewWeatherAction = useAction(async () => {
    const preview = await unwrap(
      api.POST("/api/v1/stations/{station_id}/announcements/weather/preview", {
        params: { path: here() },
      }),
    );
    setWeatherPreview(preview);
  });

  if (timeQuery.isPending || weatherQuery.isPending) {
    return <Loading />;
  }

  if (timeQuery.isError) {
    return <ErrorView error={timeQuery.error} />;
  }

  const weatherStatus = weatherStatusQuery.data;

  return (
    <div className="page-announcements">
      <PageHead
        title={t("announcements.title")}
        description={t("announcements.subtitle")}
      />

      <div className="announcements-grid grid grid-cols-1 md:grid-cols-2 gap-6 mt-6">
        {/* Spoken Time Card */}
        <Card className="p-6">
          <div className="flex items-center gap-3 mb-4">
            <span className="text-primary text-xl">
              <Icon name="clock" size={24} />
            </span>
            <h2 className="text-xl font-semibold">
              {t("announcements.timeSection")}
            </h2>
          </div>

          <form
            onSubmit={(e: FormEvent) => {
              e.preventDefault();
              saveTime.mutate();
            }}
            className="space-y-4"
          >
            <div className="flex items-center justify-between">
              <label htmlFor="time-enabled-switch" className="font-medium">
                {t("announcements.enableTime")}
              </label>
              <input
                id="time-enabled-switch"
                type="checkbox"
                checked={timeEnabled}
                onChange={(e) => setTimeEnabled(e.target.checked)}
                disabled={!canEdit || saveTime.isPending}
                className="toggle"
              />
            </div>

            <Field label={t("announcements.timeFormat")}>
              <select
                value={timeFormat}
                onChange={(e) => setTimeFormat(e.target.value as TimeFormat)}
                disabled={!canEdit || saveTime.isPending}
                className="select select-bordered w-full"
              >
                <option value="h24">{t("announcements.format24h")}</option>
                <option value="h12">{t("announcements.format12h")}</option>
              </select>
            </Field>

            <Field label={t("announcements.voicePack")}>
              <input
                type="text"
                value={voicePack}
                onChange={(e) => setVoicePack(e.target.value)}
                disabled={!canEdit || saveTime.isPending}
                placeholder="default"
                className="input input-bordered w-full"
              />
            </Field>

            <div className="flex items-center gap-3 pt-2">
              {canEdit && (
                <button
                  type="submit"
                  disabled={saveTime.isPending}
                  className="btn btn-primary"
                >
                  {t("announcements.saveTime")}
                </button>
              )}
              <button
                type="button"
                onClick={() => previewTimeAction.mutate()}
                disabled={previewTimeAction.isPending}
                className="btn btn-secondary"
              >
                <Icon name="play" />
                {t("announcements.previewTime")}
              </button>
            </div>
          </form>

          {timePreview && (
            <div className="preview-box mt-6 p-4 rounded-lg bg-base-200 border border-base-300">
              <h3 className="font-semibold text-sm uppercase tracking-wider mb-2">
                {t("announcements.previewResult")}
              </h3>
              <p className="text-lg font-medium">{timePreview.text}</p>
              <div className="flex flex-wrap gap-2 mt-3">
                {timePreview.phrase_parts.map((part, idx) => (
                  <span key={idx} className="badge badge-primary badge-outline">
                    {part}
                  </span>
                ))}
              </div>
            </div>
          )}
        </Card>

        {/* Weather Card */}
        <Card className="p-6">
          <div className="flex items-center gap-3 mb-4">
            <span className="text-accent text-xl">
              <Icon name="sun" size={24} />
            </span>
            <h2 className="text-xl font-semibold">
              {t("announcements.weatherSection")}
            </h2>
          </div>

          <form
            onSubmit={(e: FormEvent) => {
              e.preventDefault();
              saveWeather.mutate();
            }}
            className="space-y-4"
          >
            <div className="flex items-center justify-between">
              <label htmlFor="weather-enabled-switch" className="font-medium">
                {t("announcements.enableWeather")}
              </label>
              <input
                id="weather-enabled-switch"
                type="checkbox"
                checked={weatherEnabled}
                onChange={(e) => setWeatherEnabled(e.target.checked)}
                disabled={!canEdit || saveWeather.isPending}
                className="toggle"
              />
            </div>

            <Field label={t("announcements.city")}>
              <input
                type="text"
                value={city}
                onChange={(e) => setCity(e.target.value)}
                disabled={!canEdit || saveWeather.isPending}
                placeholder="Cali"
                className="input input-bordered w-full"
              />
            </Field>

            <div className="grid grid-cols-2 gap-4">
              <Field label={t("announcements.latitude")}>
                <input
                  type="number"
                  step="any"
                  value={latitude}
                  onChange={(e) => setLatitude(e.target.value)}
                  disabled={!canEdit || saveWeather.isPending}
                  placeholder="3.4516"
                  className="input input-bordered w-full"
                />
              </Field>
              <Field label={t("announcements.longitude")}>
                <input
                  type="number"
                  step="any"
                  value={longitude}
                  onChange={(e) => setLongitude(e.target.value)}
                  disabled={!canEdit || saveWeather.isPending}
                  placeholder="-76.5320"
                  className="input input-bordered w-full"
                />
              </Field>
            </div>

            <Field label={t("announcements.units")}>
              <select
                value={units}
                onChange={(e) => setUnits(e.target.value as WeatherUnits)}
                disabled={!canEdit || saveWeather.isPending}
                className="select select-bordered w-full"
              >
                <option value="metric">{t("announcements.metric")}</option>
                <option value="imperial">{t("announcements.imperial")}</option>
              </select>
            </Field>

            <div className="flex items-center gap-3 pt-2">
              {canEdit && (
                <button
                  type="submit"
                  disabled={saveWeather.isPending}
                  className="btn btn-primary"
                >
                  {t("announcements.saveWeather")}
                </button>
              )}
              {canEdit && (
                <button
                  type="button"
                  onClick={() => refreshWeatherAction.mutate()}
                  disabled={refreshWeatherAction.isPending}
                  className="btn btn-outline"
                >
                  <Icon name="undo" />
                  {t("announcements.refreshWeather")}
                </button>
              )}
              <button
                type="button"
                onClick={() => previewWeatherAction.mutate()}
                disabled={previewWeatherAction.isPending}
                className="btn btn-secondary"
              >
                <Icon name="play" />
                {t("announcements.previewWeather")}
              </button>
            </div>
          </form>

          {/* Current weather status display */}
          {weatherStatus && (
            <div className="status-box mt-6 p-4 rounded-lg bg-base-200 border border-base-300">
              <div className="flex justify-between items-center mb-2">
                <span className="text-sm font-semibold uppercase tracking-wider text-muted">
                  {t("announcements.currentWeather")}
                </span>
                {weatherStatus.city && (
                  <span className="badge badge-ghost font-semibold">
                    {weatherStatus.city}
                  </span>
                )}
              </div>
              <div className="text-2xl font-bold">
                {weatherStatus.temperature != null
                  ? `${Math.round(weatherStatus.temperature)} ${
                      weatherStatus.units === "imperial" ? "°F" : "°C"
                    }`
                  : "—"}
                {weatherStatus.summary && (
                  <span className="text-base font-normal ml-2 text-muted">
                    • {weatherStatus.summary}
                  </span>
                )}
              </div>
            </div>
          )}

          {weatherPreview && (
            <div className="preview-box mt-4 p-4 rounded-lg bg-base-200 border border-base-300">
              <h3 className="font-semibold text-sm uppercase tracking-wider mb-2">
                {t("announcements.previewResult")}
              </h3>
              <p className="text-lg font-medium">{weatherPreview.text}</p>
              <div className="flex flex-wrap gap-2 mt-3">
                {weatherPreview.phrase_parts.map((part, idx) => (
                  <span key={idx} className="badge badge-accent badge-outline">
                    {part}
                  </span>
                ))}
              </div>
            </div>
          )}
        </Card>
      </div>
    </div>
  );
}

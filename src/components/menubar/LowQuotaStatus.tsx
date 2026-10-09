import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { RefreshCw, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type {
  AutoSwitchConfig,
  AutoSwitchStatus,
} from "../../types/autoSwitch";
import * as service from "../../services/autoSwitchService";
import { isTauri } from "../../utils/env";

const PHASES = new Set([
  "disabled",
  "monitoring",
  "blocked",
  "pending",
  "canceled",
  "switching",
  "completed",
]);

/** Same coordinator as the main window. Polling only reads status; it never
 * starts checks, changes configuration, or invokes manual account switching. */
export function useMenuBarSwitchStatus() {
  const [status, setStatus] = useState<AutoSwitchStatus | null>(null);
  const [config, setConfig] = useState<AutoSwitchConfig | null>(null);
  const [readError, setReadError] = useState(false);
  const [actionError, setActionError] = useState(false);
  const [busy, setBusy] = useState(false);
  const live = useRef(true);
  const reading = useRef(false);
  const actionLock = useRef(false);
  const generation = useRef(0);
  const read = useCallback(async () => {
    if (!live.current || !isTauri() || reading.current || actionLock.current)
      return;
    reading.current = true;
    const requestId = ++generation.current;
    try {
      const [next, settings] = await Promise.all([
        service.getAutoSwitchStatus(),
        service.getAutoSwitchConfig(),
      ]);
      if (!next || !PHASES.has(next.phase))
        throw new Error("Invalid switch status");
      if (live.current && generation.current === requestId) {
        setStatus(next);
        setConfig(settings);
        setReadError(false);
      }
    } catch {
      if (live.current && generation.current === requestId) setReadError(true);
    } finally {
      if (generation.current === requestId) reading.current = false;
    }
  }, []);
  useEffect(() => {
    live.current = true;
    void read();
    const timer = window.setInterval(() => {
      if (document.visibilityState !== "hidden") void read();
    }, 3000);
    const unlisten = isTauri()
      ? listen("menubar://opened", () => void read())
      : null;
    return () => {
      live.current = false;
      generation.current++;
      reading.current = false;
      window.clearInterval(timer);
      void unlisten?.then((stop) => stop()).catch(() => {});
    };
  }, [read]);
  const act = async (cancel: boolean) => {
    if (actionLock.current || status?.phase === "switching") return;
    if (cancel && !status?.pending_id) return;
    actionLock.current = true;
    generation.current++;
    reading.current = false;
    setBusy(true);
    setActionError(false);
    try {
      const next =
        cancel && status?.pending_id
          ? await service.cancelAutoSwitch(status.pending_id)
          : await service.checkAutoSwitchNow();
      if (live.current) {
        setStatus(next);
        setReadError(false);
      }
    } catch {
      if (live.current) setActionError(true);
    } finally {
      actionLock.current = false;
      if (live.current) setBusy(false);
      void read();
    }
  };
  const visible =
    readError ||
    Boolean(status && !["disabled", "monitoring"].includes(status.phase));
  return { status, config, readError, actionError, busy, visible, read, act };
}
export type MenuBarSwitchState = ReturnType<typeof useMenuBarSwitchStatus>;

export function MenuBarSwitchDetails({
  state,
  openSettings,
}: {
  state: MenuBarSwitchState;
  openSettings: () => void;
}) {
  const { t, i18n } = useTranslation();
  const { status, config, readError, actionError, busy, act, read } = state;
  const reason =
    status?.reason ||
    (status?.phase === "monitoring" ? "monitoring" : "checking");
  return (
    <section className="mb-switch-details" aria-label={t("auto_switch.title")}>
      <h2>{t("auto_switch.title")}</h2>
      {readError && (
        <p className="mb-switch-detail-error" role="alert">
          {t("auto_switch.status_failed")}
        </p>
      )}
      {!status ? (
        <button className="mb-switch-detail-link" onClick={() => void read()}>
          {t("auto_switch.retry")}
        </button>
      ) : (
        <>
          <p className="mb-switch-detail-reason" role="status">
            {readError &&
              (i18n.language.startsWith("zh")
                ? "上次状态："
                : "Last known status: ")}
            {status.phase === "disabled"
              ? i18n.language.startsWith("zh")
                ? "低额度换号已关闭"
                : "Low-quota switching is disabled"
              : t(`auto_switch.reasons.${reason}`, {
                  defaultValue: t("auto_switch.reasons.state_unavailable"),
                })}
          </p>
          {status.remaining_percentage !== null &&
            ["pending", "switching"].includes(status.phase) && (
              <p>
                {t("auto_switch.remaining", {
                  percent: Math.floor(status.remaining_percentage),
                })}
              </p>
            )}
          <dl>
            <div>
              <dt>
                {i18n.language.startsWith("zh") ? "原账号" : "Source account"}
              </dt>
              <dd title={status.source_email || undefined}>
                {status.source_email || "—"}
              </dd>
            </div>
            {status.target_email && (
              <div>
                <dt>
                  {i18n.language.startsWith("zh")
                    ? "目标账号"
                    : "Target account"}
                </dt>
                <dd title={status.target_email}>{status.target_email}</dd>
              </div>
            )}
            {config?.monitored_model &&
              ["pending", "switching", "blocked"].includes(status.phase) && (
                <div>
                  <dt>{t("auto_switch.model")}</dt>
                  <dd title={config.monitored_model}>
                    {config.monitored_model === "all" || !config.monitored_model
                      ? (i18n.language.startsWith("zh")
                        ? "全部模型（最低额度）"
                        : "All models (lowest)")
                      : config.monitored_model}
                  </dd>
                </div>
              )}
          </dl>
          {["clients_running", "closing_clients", "client_close_failed"].includes(status.reason || "") && (
            <p className="mb-switch-detail-instructions">
              {t(`auto_switch.${status.mode}_instructions`)}
            </p>
          )}
          {status.phase === "completed" && (
            <p className="mb-switch-detail-instructions">
              {t("auto_switch.manual_continue")}
            </p>
          )}
          {actionError && !readError && (
            <p className="mb-switch-detail-error" role="alert">
              {t("auto_switch.action_failed")}
            </p>
          )}
          <div className="mb-switch-detail-actions">
            {status.pending_id && (
              <button
                disabled={busy || status.phase === "switching"}
                onClick={() => void act(true)}
              >
                <X size={12} />
                {t("auto_switch.cancel")}
              </button>
            )}
            <button
              disabled={busy || status.phase === "switching"}
              onClick={() => void act(false)}
            >
              <RefreshCw size={12} className={busy ? "mb-spin" : ""} />
              {t("auto_switch.check_now")}
            </button>
          </div>
        </>
      )}
      <button className="mb-switch-detail-link" onClick={openSettings}>
        {status?.mode === "stop" && ["clients_running", "closing_clients", "client_close_failed"].includes(status.reason || "")
          ? t("auto_switch.stop_guide")
          : t("local_settings.title")}
      </button>
    </section>
  );
}

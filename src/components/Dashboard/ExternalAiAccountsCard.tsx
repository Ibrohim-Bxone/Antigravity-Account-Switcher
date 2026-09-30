/**
 * External AI Accounts Card (Claude Pro & ChatGPT).
 * Displays real dual-limit usage quotas (5-Hour & Weekly limits + Cloud Credits)
 * with dynamic rolling-window auto-reset and real-time countdown recovery.
 * Sanitized for public release: ZERO credentials, API keys, or personal emails.
 */

import { useState, useEffect, useMemo } from "react";
import { createPortal } from "react-dom";
import { Icon } from "../Icons";
import type { ExternalAccountQuota } from "../../types";

const STORAGE_KEY = "switcher_external_ai_accounts_v7";

export const DEFAULT_ACCOUNTS: ExternalAccountQuota[] = [
  {
    id: "claude-cursor-opus",
    provider: "claude",
    name: "Claude Pro (Cursor / Opus 5.5)",
    email: "user@anthropic.pro",
    plan: "Claude Pro",
    model: "Opus 5.5 High",
    percentage_mode: "used",
    context_window: "172k / 1M (17%)",
    five_hour_percentage: 19,
    five_hour_reset: "Resets in 57 min",
    five_hour_target_time: "in 57 min",
    weekly_percentage: 0,
    weekly_reset: "Resets Tue 10:00 AM",
    cloud_credits_remaining: 98,
    cloud_credits_total: 100,
    cloud_credits_expiry: "Expires 12:59 PM GMT+5, Nov 5",
    auto_recover: true,
    last_calculated_at: Date.now(),
    status: "active",
  },
  {
    id: "chatgpt-plus-primary",
    provider: "chatgpt",
    name: "ChatGPT Plus",
    email: "user@openai.plus",
    plan: "ChatGPT Plus",
    model: "GPT-4o / Codex",
    percentage_mode: "remaining",
    five_hour_percentage: 0,
    five_hour_reset: "Resets at 7:11 PM",
    five_hour_target_time: "19:11",
    weekly_percentage: 75,
    weekly_reset: "75% left • Active cycle",
    auto_recover: true,
    last_calculated_at: Date.now(),
    status: "exhausted",
  },
  {
    id: "claude-pro-backup",
    provider: "claude",
    name: "Claude Pro #2 (Pauzada)",
    email: "backup@anthropic.pro",
    plan: "Claude Pro (To'lov qilinmagan)",
    model: "Obuna to'xtatilgan",
    percentage_mode: "used",
    five_hour_percentage: 0,
    five_hour_reset: "To'lov qilinmagan / Pauzada",
    weekly_percentage: 0,
    weekly_reset: "Obuna to'xtatilgan",
    auto_recover: false,
    status: "paused",
  },
];

export interface DynamicQuotaResult {
  fiveHourDisplayPct: number;
  fiveHourStatusText: string;
  tooltipText: string;
  isFullyReset: boolean;
  isExhausted: boolean;
  freshTimeStr: string | null;
  remainingTimeStr: string | null;
  recoveryProgressPct: number;
}

/**
 * Calculates real-time status, fresh time, and live countdown for rolling limits.
 */
function calculateDynamicQuota(acc: ExternalAccountQuota, now: Date): DynamicQuotaResult {
  if (acc.status === "paused") {
    return {
      fiveHourDisplayPct: 0,
      fiveHourStatusText: acc.five_hour_reset || "Pauzada",
      tooltipText: "Obuna to'xtatilgan",
      isFullyReset: false,
      isExhausted: false,
      freshTimeStr: null,
      remainingTimeStr: null,
      recoveryProgressPct: 0,
    };
  }

  const isClaude = acc.provider === "claude";
  const isUsedMode = acc.percentage_mode === "used" || isClaude;
  const autoRecover = acc.auto_recover !== false;

  const basePct = acc.five_hour_percentage ?? (isUsedMode ? 19 : 0);
  const isExhausted = isUsedMode ? basePct >= 100 : basePct <= 0;

  const rawTarget = (acc.five_hour_target_time || acc.five_hour_reset || "").trim();

  // 1. SPECIFIC CLOCK TIME: e.g. "19:11" or "7:11 PM"
  const timeMatch = rawTarget.match(/(\d{1,2}):(\d{2})\s*(AM|PM)?/i);
  if (timeMatch) {
    let hours = parseInt(timeMatch[1], 10);
    const minutes = parseInt(timeMatch[2], 10);
    const ampm = timeMatch[3]?.toUpperCase();

    if (ampm === "PM" && hours < 12) hours += 12;
    if (ampm === "AM" && hours === 12) hours = 0;

    const targetDate = new Date(now);
    targetDate.setHours(hours, minutes, 0, 0);

    let diffMs = targetDate.getTime() - now.getTime();

    // If target time has already passed today:
    if (diffMs <= 0) {
      if (isExhausted) {
        // If exhausted, the next fresh cycle is tomorrow at this time or +5 hours
        targetDate.setDate(targetDate.getDate() + 1);
        diffMs = targetDate.getTime() - now.getTime();
      } else if (autoRecover) {
        // If limit is not exhausted and time passed, it is fully reset (fresh)
        const displayPct = isUsedMode ? 0 : 100;
        const freshStr = `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;
        return {
          fiveHourDisplayPct: displayPct,
          fiveHourStatusText: `✨ FRESH • Soat ${freshStr} da to'liq tiklandi (100% Ready ✅)`,
          tooltipText: `Tiklanish vaqti: ${freshStr}`,
          isFullyReset: true,
          isExhausted: false,
          freshTimeStr: freshStr,
          remainingTimeStr: "0 min",
          recoveryProgressPct: 100,
        };
      }
    }

    const diffMins = Math.max(0, Math.floor(diffMs / (1000 * 60)));
    const diffSecs = Math.max(0, Math.floor((diffMs % (1000 * 60)) / 1000));
    const remHours = Math.floor(diffMins / 60);
    const remMins = diffMins % 60;
    const remStr =
      remHours > 0
        ? `${remHours} hr ${remMins} min`
        : remMins > 0
        ? `${remMins} min ${diffSecs}s`
        : `${diffSecs} soniya`;

    const freshTimeFormatted = `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;

    // 5-hour rolling cycle progress (300 minutes total)
    const totalCycleMins = 300;
    const elapsedMins = Math.max(0, totalCycleMins - diffMins);
    const recoveryProgressPct = Math.min(100, Math.max(0, Math.round((elapsedMins / totalCycleMins) * 100)));

    return {
      fiveHourDisplayPct: basePct,
      fiveHourStatusText: isExhausted
        ? `⛔ LIMIT TUGAGAN • Fresh: ${freshTimeFormatted} da (qoldi: ${remStr})`
        : `Resets at ${freshTimeFormatted} (qoldi: ${remStr})`,
      tooltipText: `Fresh tiklanish: ${freshTimeFormatted} (${remStr} qoldi)`,
      isFullyReset: false,
      isExhausted,
      freshTimeStr: freshTimeFormatted,
      remainingTimeStr: remStr,
      recoveryProgressPct,
    };
  }

  // 2. RELATIVE TIME FORMAT: e.g. "Resets in 57 min", "in 4 hr 17 min", "57m"
  const hrMatch = rawTarget.match(/(\d+)\s*(?:hr|hour|soat|h)/i);
  const minMatch = rawTarget.match(/(\d+)\s*(?:min|m)/i);

  if (hrMatch || minMatch) {
    const h = hrMatch ? parseInt(hrMatch[1], 10) : 0;
    const m = minMatch ? parseInt(minMatch[1], 10) : 0;
    const totalDurationMinutes = h * 60 + m;

    if (totalDurationMinutes > 0) {
      const createdAt = acc.last_calculated_at || now.getTime();
      const elapsedMs = now.getTime() - createdAt;
      const totalDurationMs = totalDurationMinutes * 60 * 1000;
      const remainingMs = Math.max(0, totalDurationMs - elapsedMs);

      const targetMs = createdAt + totalDurationMs;
      const targetDate = new Date(targetMs);
      const freshTimeFormatted = targetDate.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

      if (remainingMs <= 0) {
        const displayPct = autoRecover ? (isUsedMode ? 0 : 100) : basePct;
        return {
          fiveHourDisplayPct: displayPct,
          fiveHourStatusText: autoRecover
            ? `✨ FRESH • Soat ${freshTimeFormatted} da to'liq tiklandi (100% Ready ✅)`
            : "Tiklanish muddati yetib keldi",
          tooltipText: `Tiklandi: ${freshTimeFormatted}`,
          isFullyReset: autoRecover,
          isExhausted: false,
          freshTimeStr: freshTimeFormatted,
          remainingTimeStr: "0 min",
          recoveryProgressPct: 100,
        };
      }

      const totalRemSecs = Math.floor(remainingMs / 1000);
      const remH = Math.floor(totalRemSecs / 3600);
      const remM = Math.floor((totalRemSecs % 3600) / 60);
      const remS = totalRemSecs % 60;
      const remStr =
        remH > 0
          ? `${remH} hr ${remM} min`
          : remM > 0
          ? `${remM} min ${remS}s`
          : `${remS} soniya`;

      const recoveryProgressPct = Math.min(
        100,
        Math.max(0, Math.round(((totalDurationMs - remainingMs) / totalDurationMs) * 100))
      );

      return {
        fiveHourDisplayPct: basePct,
        fiveHourStatusText: isExhausted
          ? `⛔ LIMIT TUGAGAN • Fresh: ${freshTimeFormatted} da (qoldi: ${remStr})`
          : `Resets in ${remStr} • Fresh: ${freshTimeFormatted}`,
        tooltipText: `Fresh tiklanish: ${freshTimeFormatted} (${remStr} qoldi)`,
        isFullyReset: false,
        isExhausted,
        freshTimeStr: freshTimeFormatted,
        remainingTimeStr: remStr,
        recoveryProgressPct,
      };
    }
  }

  // 3. FALLBACK DEFAULT
  return {
    fiveHourDisplayPct: basePct,
    fiveHourStatusText: acc.five_hour_reset || "Active window",
    tooltipText: acc.five_hour_reset || "Active window",
    isFullyReset: isUsedMode ? basePct === 0 : basePct === 100,
    isExhausted,
    freshTimeStr: null,
    remainingTimeStr: null,
    recoveryProgressPct: 0,
  };
}

export default function ExternalAiAccountsCard() {
  const [accounts, setAccounts] = useState<ExternalAccountQuota[]>(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        if (Array.isArray(parsed) && parsed.length > 0) {
          return parsed;
        }
      }
    } catch {
      // ignore
    }
    return DEFAULT_ACCOUNTS;
  });

  const [currentTime, setCurrentTime] = useState<Date>(new Date());
  const [editModalOpen, setEditModalOpen] = useState(false);
  const [editingAccount, setEditingAccount] = useState<ExternalAccountQuota | null>(null);
  const [hoveredLimitId, setHoveredLimitId] = useState<string | null>(null);

  // Live rolling update interval: recalculate every 1 second for live countdown
  useEffect(() => {
    const timer = setInterval(() => {
      setCurrentTime(new Date());
    }, 1000);
    return () => clearInterval(timer);
  }, []);

  // Save to localStorage whenever accounts change
  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(accounts));
    } catch {
      // ignore
    }
  }, [accounts]);

  const handleResetToRealData = () => {
    setAccounts(DEFAULT_ACCOUNTS);
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(DEFAULT_ACCOUNTS));
    } catch {
      // ignore
    }
    setEditModalOpen(false);
    setEditingAccount(null);
  };

  const handleSaveAccount = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const form = e.currentTarget;
    const formData = new FormData(form);
    const id = editingAccount ? editingAccount.id : `ext-${Date.now()}`;
    const name = (formData.get("name") as string) || editingAccount?.name || "AI Account";
    const provider = (formData.get("provider") as "claude" | "chatgpt") || editingAccount?.provider || "claude";
    const plan = (formData.get("plan") as string) || editingAccount?.plan || (provider === "claude" ? "Claude Pro" : "ChatGPT Plus");
    const model = (formData.get("model") as string) || editingAccount?.model || "";
    const email = (formData.get("email") as string) || editingAccount?.email || (provider === "claude" ? "user@anthropic.pro" : "user@openai.plus");
    const context_window = (formData.get("context_window") as string) || editingAccount?.context_window || "";

    const fiveHourPct = Math.min(100, Math.max(0, parseInt(formData.get("five_hour_percentage") as string) || 0));
    const fiveHourReset = (formData.get("five_hour_reset") as string) || "Resets at 7:11 PM";
    const fiveHourTargetTime = (formData.get("five_hour_target_time") as string) || "19:11";

    const weeklyPct = Math.min(100, Math.max(0, parseInt(formData.get("weekly_percentage") as string) || 0));
    const weeklyReset = (formData.get("weekly_reset") as string) || "Active cycle";

    const creditsRemainingRaw = formData.get("cloud_credits_remaining") as string;
    const cloudCreditsRemaining = creditsRemainingRaw ? parseFloat(creditsRemainingRaw) : undefined;
    const creditsTotalRaw = formData.get("cloud_credits_total") as string;
    const cloudCreditsTotal = creditsTotalRaw ? parseFloat(creditsTotalRaw) : undefined;
    const cloudCreditsExpiry = (formData.get("cloud_credits_expiry") as string) || undefined;

    const autoRecover = formData.get("auto_recover") === "on";

    const status: "active" | "low" | "exhausted" | "paused" =
      editingAccount?.status === "paused"
        ? "paused"
        : weeklyPct === 0 && fiveHourPct === 0
        ? "exhausted"
        : weeklyPct < 20 || fiveHourPct < 20
        ? "low"
        : "active";

    const updatedAccount: ExternalAccountQuota = {
      id,
      provider,
      name,
      email,
      plan,
      model,
      context_window,
      five_hour_percentage: fiveHourPct,
      five_hour_reset: fiveHourReset,
      five_hour_target_time: fiveHourTargetTime,
      weekly_percentage: weeklyPct,
      weekly_reset: weeklyReset,
      cloud_credits_remaining: cloudCreditsRemaining,
      cloud_credits_total: cloudCreditsTotal,
      cloud_credits_expiry: cloudCreditsExpiry,
      percentage_mode: provider === "claude" ? "used" : "remaining",
      remaining_percentage: fiveHourPct,
      reset_time: fiveHourReset,
      auto_recover: autoRecover,
      last_calculated_at: Date.now(),
      status,
    };

    if (editingAccount) {
      setAccounts((prev) => prev.map((a) => (a.id === id ? updatedAccount : a)));
    } else {
      setAccounts((prev) => [...prev, updatedAccount]);
    }

    setEditModalOpen(false);
    setEditingAccount(null);
  };

  const handleDeleteAccount = (id: string) => {
    setAccounts((prev) => prev.filter((a) => a.id !== id));
  };

  return (
    <div
      className="external-ai-card"
      style={{
        background: "linear-gradient(135deg, rgba(20, 24, 33, 0.95) 0%, rgba(26, 32, 44, 0.95) 100%)",
        borderRadius: "12px",
        border: "1px solid rgba(217, 119, 6, 0.3)",
        boxShadow: "0 4px 20px rgba(0, 0, 0, 0.25)",
        padding: "16px 20px",
        marginBottom: "20px",
        position: "relative",
      }}
    >
      {/* Top accent badge */}
      <div
        style={{
          position: "absolute",
          top: 0,
          left: 0,
          right: 0,
          height: "3px",
          background: "linear-gradient(90deg, #d97706, #f59e0b, #10b981)",
        }}
      />

      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: "14px",
          flexWrap: "wrap",
          gap: "10px",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
          <div
            style={{
              width: "32px",
              height: "32px",
              borderRadius: "8px",
              background: "rgba(245, 158, 11, 0.15)",
              color: "#f59e0b",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <Icon name="zap" size={18} />
          </div>
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
              <h3 style={{ margin: 0, fontSize: "14px", fontWeight: 700, color: "#fff" }}>
                CLAUDE PRO & CHATGPT LIMITS
              </h3>
              <span
                style={{
                  fontSize: "10px",
                  fontWeight: 700,
                  textTransform: "uppercase",
                  padding: "2px 6px",
                  borderRadius: "4px",
                  background: "rgba(245, 158, 11, 0.2)",
                  color: "#f59e0b",
                  letterSpacing: "0.5px",
                }}
              >
                Top Priority
              </span>
              <span
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  gap: "4px",
                  fontSize: "10px",
                  padding: "2px 6px",
                  borderRadius: "4px",
                  background: "rgba(16, 185, 129, 0.15)",
                  color: "#34d399",
                  fontWeight: 600,
                }}
              >
                <span
                  style={{
                    width: "6px",
                    height: "6px",
                    borderRadius: "50%",
                    background: "#10b981",
                    display: "inline-block",
                    boxShadow: "0 0 6px #10b981",
                  }}
                />
                Live Rolling Reset
              </span>
            </div>
            <p style={{ margin: 0, fontSize: "11px", color: "var(--text-muted, #94a3b8)" }}>
              5-Hour Window (Avtomatik hisoblash), Weekly Limit va Cloud Credits
            </p>
          </div>
        </div>

        <div style={{ display: "flex", gap: "8px" }}>
          <button
            onClick={handleResetToRealData}
            title="Haqiqiy skrinshot ko'rsatkichlarini tiklash"
            style={{
              display: "flex",
              alignItems: "center",
              gap: "6px",
              padding: "6px 10px",
              borderRadius: "6px",
              background: "rgba(255, 255, 255, 0.05)",
              border: "1px solid rgba(255, 255, 255, 0.12)",
              color: "#94a3b8",
              fontSize: "11px",
              fontWeight: 500,
              cursor: "pointer",
              transition: "all 0.2s",
            }}
          >
            <Icon name="refresh" size={12} />
            <span>Skrinshot Ma'lumotlarini Tiklash</span>
          </button>

          <button
            onClick={() => {
              setEditingAccount(null);
              setEditModalOpen(true);
            }}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "6px",
              padding: "6px 12px",
              borderRadius: "6px",
              background: "rgba(245, 158, 11, 0.12)",
              border: "1px solid rgba(245, 158, 11, 0.3)",
              color: "#f59e0b",
              fontSize: "12px",
              fontWeight: 600,
              cursor: "pointer",
              transition: "all 0.2s",
            }}
          >
            <Icon name="plus" size={14} />
            <span>Akkaunt qo'shish / Sozlash</span>
          </button>
        </div>
      </div>

      {/* Grid of Accounts */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))",
          gap: "14px",
        }}
      >
        {accounts.map((acc) => {
          const isClaude = acc.provider === "claude";
          const isPaused = acc.status === "paused";
          const themeColor = isPaused ? "#94a3b8" : isClaude ? "#d97706" : "#10b981";
          const bgTone = isPaused
            ? "rgba(148, 163, 184, 0.04)"
            : isClaude
            ? "rgba(217, 119, 6, 0.06)"
            : "rgba(16, 185, 129, 0.06)";
          const borderTone = isPaused
            ? "rgba(239, 68, 68, 0.3)"
            : isClaude
            ? "rgba(217, 119, 6, 0.25)"
            : "rgba(16, 185, 129, 0.25)";

          const isUsedMode = acc.percentage_mode === "used" || isClaude;

          // Dynamic calculation
          const dynamicQuota = calculateDynamicQuota(acc, currentTime);
          const fivePct = dynamicQuota.fiveHourDisplayPct;
          const weeklyPct = acc.weekly_percentage ?? 0;

          const fiveTone = isPaused
            ? "#94a3b8"
            : isUsedMode
            ? (fivePct > 80 ? "#ef4444" : fivePct > 50 ? "#f59e0b" : "#10b981")
            : (fivePct === 0 ? "#ef4444" : fivePct < 20 ? "#ef4444" : fivePct < 50 ? "#f59e0b" : "#10b981");

          const weeklyTone = isPaused
            ? "#94a3b8"
            : isUsedMode
            ? (weeklyPct > 80 ? "#ef4444" : weeklyPct > 50 ? "#f59e0b" : "#10b981")
            : (weeklyPct === 0 ? "#ef4444" : weeklyPct < 20 ? "#ef4444" : weeklyPct < 50 ? "#f59e0b" : "#10b981");

          const isHovered = hoveredLimitId === acc.id;

          return (
            <div
              key={acc.id}
              style={{
                background: bgTone,
                border: `1px solid ${borderTone}`,
                borderRadius: "10px",
                padding: "14px 16px",
                position: "relative",
                display: "flex",
                flexDirection: "column",
                gap: "10px",
                opacity: isPaused ? 0.8 : 1,
              }}
            >
              {/* Account Header */}
              <div
                style={{
                  display: "flex",
                  alignItems: "flex-start",
                  justifyContent: "space-between",
                  gap: "8px",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
                  <div
                    style={{
                      width: "28px",
                      height: "28px",
                      borderRadius: "7px",
                      background: `${themeColor}25`,
                      color: themeColor,
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontWeight: 800,
                      fontSize: "13px",
                    }}
                  >
                    {isClaude ? "C" : "G"}
                  </div>
                  <div>
                    <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                      <span style={{ fontSize: "13px", fontWeight: 700, color: "#fff", lineHeight: "1.2" }}>
                        {acc.name}
                      </span>
                      {isPaused && (
                        <span
                          style={{
                            fontSize: "9px",
                            fontWeight: 700,
                            textTransform: "uppercase",
                            padding: "1px 5px",
                            borderRadius: "3px",
                            background: "rgba(239, 68, 68, 0.2)",
                            color: "#f87171",
                          }}
                        >
                          Pauza
                        </span>
                      )}
                    </div>
                    <div
                      style={{
                        fontSize: "11px",
                        color: isPaused ? "#f87171" : "var(--text-muted, #94a3b8)",
                        marginTop: "2px",
                      }}
                    >
                      {acc.plan} {acc.model ? `• ${acc.model}` : ""}
                    </div>
                  </div>
                </div>

                <button
                  onClick={() => {
                    setEditingAccount(acc);
                    setEditModalOpen(true);
                  }}
                  style={{
                    background: "rgba(255, 255, 255, 0.05)",
                    border: "1px solid rgba(255, 255, 255, 0.1)",
                    color: "var(--text-muted, #94a3b8)",
                    cursor: "pointer",
                    padding: "4px 6px",
                    borderRadius: "4px",
                    display: "flex",
                    alignItems: "center",
                  }}
                  title="Tahrirlash"
                >
                  <Icon name="settings" size={13} />
                </button>
              </div>

              {/* 5-Hour Limit Row with Tooltip Hover and Dynamic Fresh Display */}
              <div
                onMouseEnter={() => setHoveredLimitId(acc.id)}
                onMouseLeave={() => setHoveredLimitId(null)}
                style={{
                  background: dynamicQuota.isExhausted
                    ? "rgba(239, 68, 68, 0.08)"
                    : dynamicQuota.isFullyReset
                    ? "rgba(16, 185, 129, 0.06)"
                    : "rgba(0, 0, 0, 0.25)",
                  padding: "10px 12px",
                  borderRadius: "8px",
                  border: dynamicQuota.isExhausted
                    ? "1px solid rgba(239, 68, 68, 0.35)"
                    : dynamicQuota.isFullyReset
                    ? "1px solid rgba(16, 185, 129, 0.35)"
                    : "1px solid rgba(255, 255, 255, 0.06)",
                  position: "relative",
                  cursor: "default",
                }}
              >
                {/* Floating Screenshot-accurate Tooltip */}
                {isHovered && dynamicQuota.tooltipText && (
                  <div
                    style={{
                      position: "absolute",
                      right: "-10px",
                      top: "50%",
                      transform: "translate(100%, -50%)",
                      background: "rgba(35, 38, 43, 0.98)",
                      border: "1px solid rgba(255, 255, 255, 0.2)",
                      borderRadius: "8px",
                      padding: "8px 14px",
                      color: "#e2e8f0",
                      fontSize: "12px",
                      fontWeight: 600,
                      boxShadow: "0 8px 24px rgba(0, 0, 0, 0.6)",
                      whiteSpace: "nowrap",
                      zIndex: 100,
                      pointerEvents: "none",
                      display: "flex",
                      alignItems: "center",
                      gap: "6px",
                    }}
                  >
                    <Icon name="refresh" size={13} style={{ color: "#10b981" }} />
                    <span>{dynamicQuota.tooltipText}</span>
                  </div>
                )}

                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "baseline",
                    marginBottom: "5px",
                  }}
                >
                  <span style={{ fontSize: "11px", color: "#cbd5e1", fontWeight: 600 }}>
                    5-Hour Limit:
                  </span>
                  <span style={{ fontSize: "12px", fontWeight: 700, color: fiveTone }}>
                    {isPaused
                      ? "Pauzada"
                      : isUsedMode
                      ? `${fivePct}% ishlatildi (${Math.max(0, 100 - fivePct)}% qoldi)${
                          dynamicQuota.isExhausted ? " ⛔ Tugagan" : ""
                        }`
                      : `${fivePct}% left${dynamicQuota.isExhausted ? " (Limit Tugagan ⛔)" : ""}`}
                  </span>
                </div>

                <div
                  style={{
                    height: "6px",
                    borderRadius: "3px",
                    background: "rgba(255, 255, 255, 0.08)",
                    overflow: "hidden",
                    marginBottom: "6px",
                  }}
                >
                  <div
                    style={{
                      height: "100%",
                      width: `${fivePct}%`,
                      background: dynamicQuota.isExhausted
                        ? "#ef4444"
                        : dynamicQuota.isFullyReset
                        ? "#10b981"
                        : fiveTone,
                      borderRadius: "3px",
                      transition: "width 0.4s ease",
                    }}
                  />
                </div>

                {/* Fresh & Countdown Status Box */}
                {dynamicQuota.isExhausted ? (
                  <div
                    style={{
                      marginTop: "6px",
                      padding: "8px 10px",
                      background: "rgba(239, 68, 68, 0.14)",
                      border: "1px solid rgba(239, 68, 68, 0.3)",
                      borderRadius: "6px",
                      display: "flex",
                      flexDirection: "column",
                      gap: "5px",
                    }}
                  >
                    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                      <span
                        style={{
                          fontSize: "11px",
                          fontWeight: 700,
                          color: "#f87171",
                          display: "flex",
                          alignItems: "center",
                          gap: "5px",
                        }}
                      >
                        <Icon name="refresh" size={12} />
                        Qayta tiklanish (Fresh):
                      </span>
                      <span style={{ fontSize: "12px", fontWeight: 800, color: "#38bdf8" }}>
                        {dynamicQuota.freshTimeStr ? `Bugun ${dynamicQuota.freshTimeStr} da` : "Tez orada"}
                      </span>
                    </div>

                    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", fontSize: "11px" }}>
                      <span style={{ color: "var(--text-muted, #94a3b8)" }}>Tiklanishgacha vaqt:</span>
                      <span style={{ color: "#fbbf24", fontWeight: 700 }}>
                        Qoldi: {dynamicQuota.remainingTimeStr || "Hisoblanmoqda..."}
                      </span>
                    </div>

                    {/* Fresh Recovery Progress Bar */}
                    <div style={{ marginTop: "2px" }}>
                      <div
                        style={{
                          height: "4px",
                          background: "rgba(255, 255, 255, 0.1)",
                          borderRadius: "2px",
                          overflow: "hidden",
                        }}
                      >
                        <div
                          style={{
                            height: "100%",
                            width: `${dynamicQuota.recoveryProgressPct}%`,
                            background: "linear-gradient(90deg, #f59e0b, #10b981)",
                            transition: "width 0.5s ease",
                          }}
                        />
                      </div>
                    </div>
                  </div>
                ) : (
                  <div
                    style={{
                      fontSize: "11px",
                      color: isPaused
                        ? "#f87171"
                        : dynamicQuota.isFullyReset
                        ? "#34d399"
                        : "#94a3b8",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      gap: "4px",
                      flexWrap: "wrap",
                    }}
                  >
                    <span>{dynamicQuota.fiveHourStatusText}</span>
                    {dynamicQuota.freshTimeStr && !dynamicQuota.isFullyReset && (
                      <span style={{ color: "#38bdf8", fontWeight: 600, fontSize: "10px" }}>
                        Fresh: {dynamicQuota.freshTimeStr}
                      </span>
                    )}
                  </div>
                )}
              </div>

              {/* Weekly Limit Row */}
              {(() => {
                const isWeeklyExhausted = isUsedMode ? weeklyPct >= 100 : weeklyPct <= 0;
                return (
                  <div
                    style={{
                      background: "rgba(0, 0, 0, 0.25)",
                      padding: "8px 10px",
                      borderRadius: "6px",
                      border: `1px solid ${
                        isWeeklyExhausted
                          ? "rgba(239, 68, 68, 0.3)"
                          : "rgba(255, 255, 255, 0.04)"
                      }`,
                    }}
                  >
                    <div
                      style={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "baseline",
                        marginBottom: "4px",
                      }}
                    >
                      <span style={{ fontSize: "11px", color: "#cbd5e1", fontWeight: 600 }}>
                        Weekly Limit (All Models):
                      </span>
                      <span style={{ fontSize: "12px", fontWeight: 700, color: weeklyTone }}>
                        {isPaused
                          ? "Pauzada"
                          : isUsedMode
                          ? `${weeklyPct}% ishlatildi (${Math.max(0, 100 - weeklyPct)}% qoldi)`
                          : `${weeklyPct}% left`}
                      </span>
                    </div>
                    <div
                      style={{
                        height: "5px",
                        borderRadius: "3px",
                        background: "rgba(255, 255, 255, 0.08)",
                        overflow: "hidden",
                        marginBottom: "4px",
                      }}
                    >
                      <div
                        style={{
                          height: "100%",
                          width: `${weeklyPct}%`,
                          background: weeklyTone,
                          borderRadius: "3px",
                          transition: "width 0.4s ease",
                        }}
                      />
                    </div>
                    <div
                      style={{
                        fontSize: "10px",
                        color: isWeeklyExhausted ? "#f87171" : "#94a3b8",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "space-between",
                      }}
                    >
                      <span>{acc.weekly_reset || "Active cycle"}</span>
                      {acc.weekly_reset?.includes("Resets") && (
                        <span style={{ color: "#38bdf8", fontWeight: 600 }}>
                          {acc.weekly_reset.replace(/^Resets\s*/i, "Fresh: ")}
                        </span>
                      )}
                    </div>
                  </div>
                );
              })()}

              {/* Cloud Session Credits (Optional - Claude / Cursor) */}
              {acc.cloud_credits_remaining !== undefined && (
                <div
                  style={{
                    background: "rgba(59, 130, 246, 0.08)",
                    padding: "8px 10px",
                    borderRadius: "6px",
                    border: "1px solid rgba(59, 130, 246, 0.2)",
                  }}
                >
                  <div
                    style={{
                      display: "flex",
                      justifyContent: "space-between",
                      alignItems: "baseline",
                      marginBottom: "4px",
                    }}
                  >
                    <span style={{ fontSize: "11px", color: "#93c5fd", fontWeight: 600 }}>
                      Cloud session credits:
                    </span>
                    <span style={{ fontSize: "12px", fontWeight: 700, color: "#60a5fa" }}>
                      ${acc.cloud_credits_remaining} of ${acc.cloud_credits_total || 100} left
                    </span>
                  </div>
                  <div
                    style={{
                      height: "5px",
                      borderRadius: "3px",
                      background: "rgba(255, 255, 255, 0.08)",
                      overflow: "hidden",
                      marginBottom: "4px",
                    }}
                  >
                    <div
                      style={{
                        height: "100%",
                        width: `${Math.round(
                          ((acc.cloud_credits_remaining || 0) / (acc.cloud_credits_total || 100)) * 100
                        )}%`,
                        background: "#3b82f6",
                        borderRadius: "3px",
                      }}
                    />
                  </div>
                  {acc.cloud_credits_expiry && (
                    <div style={{ fontSize: "10px", color: "#94a3b8" }}>
                      {acc.cloud_credits_expiry}
                    </div>
                  )}
                </div>
              )}

              {/* Context Window Info if present */}
              {acc.context_window && (
                <div
                  style={{
                    fontSize: "10px",
                    color: "#94a3b8",
                    display: "flex",
                    justifyContent: "space-between",
                    paddingTop: "2px",
                  }}
                >
                  <span>Context window:</span>
                  <span style={{ color: "#cbd5e1", fontWeight: 600 }}>{acc.context_window}</span>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* Edit / Add Modal using createPortal to prevent getting stuck */}
      {editModalOpen &&
        typeof document !== "undefined" &&
        createPortal(
          <div
            onClick={() => setEditModalOpen(false)}
            style={{
              position: "fixed",
              top: 0,
              left: 0,
              right: 0,
              bottom: 0,
              background: "rgba(0, 0, 0, 0.75)",
              backdropFilter: "blur(6px)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              zIndex: 99999,
              padding: "20px",
            }}
          >
            <div
              onClick={(e) => e.stopPropagation()}
              style={{
                background: "#1e2430",
                border: "1px solid #334155",
                borderRadius: "12px",
                width: "100%",
                maxWidth: "460px",
                padding: "22px",
                boxShadow: "0 20px 40px rgba(0, 0, 0, 0.6)",
                maxHeight: "90vh",
                overflowY: "auto",
              }}
            >
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  marginBottom: "16px",
                }}
              >
                <h4 style={{ margin: 0, color: "#fff", fontSize: "16px", fontWeight: 700 }}>
                  {editingAccount ? "AI Akkaunt Limitlarini Sozlash" : "Yangi AI Akkaunt Qo'shish"}
                </h4>
                <button
                  type="button"
                  onClick={() => setEditModalOpen(false)}
                  style={{
                    background: "transparent",
                    border: "none",
                    color: "#94a3b8",
                    cursor: "pointer",
                    padding: "4px",
                    fontSize: "14px",
                  }}
                >
                  ✕
                </button>
              </div>

              <form onSubmit={handleSaveAccount}>
                <div style={{ marginBottom: "12px" }}>
                  <label style={{ display: "block", fontSize: "12px", color: "#94a3b8", marginBottom: "4px" }}>
                    Akkaunt nomi:
                  </label>
                  <input
                    name="name"
                    defaultValue={editingAccount?.name || "ChatGPT Plus"}
                    required
                    style={{
                      width: "100%",
                      padding: "8px 10px",
                      borderRadius: "6px",
                      background: "#0f172a",
                      border: "1px solid #334155",
                      color: "#fff",
                      boxSizing: "border-box",
                    }}
                  />
                </div>

                <div style={{ display: "flex", gap: "10px", marginBottom: "12px" }}>
                  <div style={{ flex: 1 }}>
                    <label style={{ display: "block", fontSize: "12px", color: "#94a3b8", marginBottom: "4px" }}>
                      Provayder:
                    </label>
                    <select
                      name="provider"
                      defaultValue={editingAccount?.provider || "chatgpt"}
                      style={{
                        width: "100%",
                        padding: "8px 10px",
                        borderRadius: "6px",
                        background: "#0f172a",
                        border: "1px solid #334155",
                        color: "#fff",
                        boxSizing: "border-box",
                      }}
                    >
                      <option value="chatgpt">ChatGPT (OpenAI / Codex)</option>
                      <option value="claude">Claude (Anthropic / Cursor)</option>
                    </select>
                  </div>
                  <div style={{ flex: 1 }}>
                    <label style={{ display: "block", fontSize: "12px", color: "#94a3b8", marginBottom: "4px" }}>
                      Tarif (Plan):
                    </label>
                    <input
                      name="plan"
                      defaultValue={editingAccount?.plan || "ChatGPT Plus"}
                      style={{
                        width: "100%",
                        padding: "8px 10px",
                        borderRadius: "6px",
                        background: "#0f172a",
                        border: "1px solid #334155",
                        color: "#fff",
                        boxSizing: "border-box",
                      }}
                    />
                  </div>
                </div>

                <div style={{ marginBottom: "12px" }}>
                  <label style={{ display: "block", fontSize: "12px", color: "#94a3b8", marginBottom: "4px" }}>
                    Model nomi (masalan: GPT-4o / Codex yoki Opus 5.5 High):
                  </label>
                  <input
                    name="model"
                    defaultValue={editingAccount?.model || "GPT-4o / Codex"}
                    style={{
                      width: "100%",
                      padding: "8px 10px",
                      borderRadius: "6px",
                      background: "#0f172a",
                      border: "1px solid #334155",
                      color: "#fff",
                      boxSizing: "border-box",
                    }}
                  />
                </div>

                {/* 5-Hour Inputs */}
                <div
                  style={{
                    background: "rgba(0, 0, 0, 0.2)",
                    padding: "10px",
                    borderRadius: "8px",
                    border: "1px solid #334155",
                    marginBottom: "12px",
                  }}
                >
                  <div style={{ fontSize: "12px", fontWeight: 700, color: "#f59e0b", marginBottom: "8px" }}>
                    5-Hour Limit & Dinamik Tiklanish:
                  </div>
                  <div style={{ display: "flex", gap: "10px", marginBottom: "8px" }}>
                    <div style={{ flex: 1 }}>
                      <label style={{ display: "block", fontSize: "11px", color: "#94a3b8", marginBottom: "4px" }}>
                        Foiz (%):
                      </label>
                      <input
                        name="five_hour_percentage"
                        type="number"
                        min="0"
                        max="100"
                        defaultValue={editingAccount?.five_hour_percentage ?? 100}
                        style={{
                          width: "100%",
                          padding: "8px 10px",
                          borderRadius: "6px",
                          background: "#0f172a",
                          border: "1px solid #334155",
                          color: "#fff",
                          boxSizing: "border-box",
                        }}
                      />
                    </div>
                    <div style={{ flex: 1.5 }}>
                      <label style={{ display: "block", fontSize: "11px", color: "#94a3b8", marginBottom: "4px" }}>
                        Tiklanish vaqti (Reset time):
                      </label>
                      <input
                        name="five_hour_reset"
                        defaultValue={editingAccount?.five_hour_reset || "Resets at 7:11 PM"}
                        style={{
                          width: "100%",
                          padding: "8px 10px",
                          borderRadius: "6px",
                          background: "#0f172a",
                          border: "1px solid #334155",
                          color: "#fff",
                          boxSizing: "border-box",
                        }}
                      />
                    </div>
                  </div>

                  <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
                    <div style={{ flex: 1 }}>
                      <label style={{ display: "block", fontSize: "11px", color: "#94a3b8", marginBottom: "4px" }}>
                        Aniq soat (24h yoki 12h, masalan: 19:11):
                      </label>
                      <input
                        name="five_hour_target_time"
                        defaultValue={editingAccount?.five_hour_target_time || "19:11"}
                        placeholder="19:11 yoki 7:11 PM"
                        style={{
                          width: "100%",
                          padding: "8px 10px",
                          borderRadius: "6px",
                          background: "#0f172a",
                          border: "1px solid #334155",
                          color: "#fff",
                          boxSizing: "border-box",
                        }}
                      />
                    </div>
                    <div style={{ flex: 1, display: "flex", alignItems: "center", gap: "6px", paddingTop: "16px" }}>
                      <input
                        type="checkbox"
                        id="auto_recover"
                        name="auto_recover"
                        defaultChecked={editingAccount?.auto_recover !== false}
                      />
                      <label htmlFor="auto_recover" style={{ fontSize: "11px", color: "#10b981", cursor: "pointer" }}>
                        Avto-tiklanish (Auto-reset)
                      </label>
                    </div>
                  </div>
                </div>

                {/* Weekly Inputs */}
                <div
                  style={{
                    background: "rgba(0, 0, 0, 0.2)",
                    padding: "10px",
                    borderRadius: "8px",
                    border: "1px solid #334155",
                    marginBottom: "12px",
                  }}
                >
                  <div style={{ fontSize: "12px", fontWeight: 700, color: "#10b981", marginBottom: "8px" }}>
                    Weekly Limit (Haftalik):
                  </div>
                  <div style={{ display: "flex", gap: "10px" }}>
                    <div style={{ flex: 1 }}>
                      <label style={{ display: "block", fontSize: "11px", color: "#94a3b8", marginBottom: "4px" }}>
                        Qoldiq foiz (% left):
                      </label>
                      <input
                        name="weekly_percentage"
                        type="number"
                        min="0"
                        max="100"
                        defaultValue={editingAccount?.weekly_percentage ?? 75}
                        style={{
                          width: "100%",
                          padding: "8px 10px",
                          borderRadius: "6px",
                          background: "#0f172a",
                          border: "1px solid #334155",
                          color: "#fff",
                          boxSizing: "border-box",
                        }}
                      />
                    </div>
                    <div style={{ flex: 1.5 }}>
                      <label style={{ display: "block", fontSize: "11px", color: "#94a3b8", marginBottom: "4px" }}>
                        Tiklanish tsikli:
                      </label>
                      <input
                        name="weekly_reset"
                        defaultValue={editingAccount?.weekly_reset || "75% left • Active cycle"}
                        style={{
                          width: "100%",
                          padding: "8px 10px",
                          borderRadius: "6px",
                          background: "#0f172a",
                          border: "1px solid #334155",
                          color: "#fff",
                          boxSizing: "border-box",
                        }}
                      />
                    </div>
                  </div>
                </div>

                {/* Cloud Credits Inputs */}
                <div
                  style={{
                    background: "rgba(0, 0, 0, 0.2)",
                    padding: "10px",
                    borderRadius: "8px",
                    border: "1px solid #334155",
                    marginBottom: "16px",
                  }}
                >
                  <div style={{ fontSize: "12px", fontWeight: 700, color: "#60a5fa", marginBottom: "8px" }}>
                    Cloud Session Credits ($):
                  </div>
                  <div style={{ display: "flex", gap: "10px" }}>
                    <div style={{ flex: 1 }}>
                      <label style={{ display: "block", fontSize: "11px", color: "#94a3b8", marginBottom: "4px" }}>
                        Qoldiq ($):
                      </label>
                      <input
                        name="cloud_credits_remaining"
                        type="number"
                        step="1"
                        defaultValue={editingAccount?.cloud_credits_remaining ?? 98}
                        style={{
                          width: "100%",
                          padding: "8px 10px",
                          borderRadius: "6px",
                          background: "#0f172a",
                          border: "1px solid #334155",
                          color: "#fff",
                          boxSizing: "border-box",
                        }}
                      />
                    </div>
                    <div style={{ flex: 1 }}>
                      <label style={{ display: "block", fontSize: "11px", color: "#94a3b8", marginBottom: "4px" }}>
                        Jami ($):
                      </label>
                      <input
                        name="cloud_credits_total"
                        type="number"
                        step="1"
                        defaultValue={editingAccount?.cloud_credits_total ?? 100}
                        style={{
                          width: "100%",
                          padding: "8px 10px",
                          borderRadius: "6px",
                          background: "#0f172a",
                          border: "1px solid #334155",
                          color: "#fff",
                          boxSizing: "border-box",
                        }}
                      />
                    </div>
                    <div style={{ flex: 1.5 }}>
                      <label style={{ display: "block", fontSize: "11px", color: "#94a3b8", marginBottom: "4px" }}>
                        Muddati:
                      </label>
                      <input
                        name="cloud_credits_expiry"
                        defaultValue={editingAccount?.cloud_credits_expiry || "Expires 12:59 PM GMT+5, Nov 5"}
                        style={{
                          width: "100%",
                          padding: "8px 10px",
                          borderRadius: "6px",
                          background: "#0f172a",
                          border: "1px solid #334155",
                          color: "#fff",
                          boxSizing: "border-box",
                        }}
                      />
                    </div>
                  </div>
                </div>

                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    marginTop: "20px",
                  }}
                >
                  {editingAccount ? (
                    <button
                      type="button"
                      onClick={() => {
                        handleDeleteAccount(editingAccount.id);
                        setEditModalOpen(false);
                        setEditingAccount(null);
                      }}
                      style={{
                        background: "rgba(239, 68, 68, 0.15)",
                        border: "1px solid rgba(239, 68, 68, 0.4)",
                        color: "#ef4444",
                        padding: "8px 14px",
                        borderRadius: "6px",
                        fontSize: "12px",
                        cursor: "pointer",
                      }}
                    >
                      O'chirish
                    </button>
                  ) : (
                    <div />
                  )}

                  <div style={{ display: "flex", gap: "8px" }}>
                    <button
                      type="button"
                      onClick={() => {
                        setEditModalOpen(false);
                        setEditingAccount(null);
                      }}
                      style={{
                        background: "none",
                        border: "1px solid #475569",
                        color: "#94a3b8",
                        padding: "8px 14px",
                        borderRadius: "6px",
                        fontSize: "12px",
                        cursor: "pointer",
                      }}
                    >
                      Bekor qilish
                    </button>
                    <button
                      type="submit"
                      style={{
                        background: "#f59e0b",
                        border: "none",
                        color: "#000",
                        padding: "8px 16px",
                        borderRadius: "6px",
                        fontSize: "12px",
                        fontWeight: 700,
                        cursor: "pointer",
                      }}
                    >
                      Saqlash
                    </button>
                  </div>
                </div>
              </form>
            </div>
          </div>,
          document.body
        )}
    </div>
  );
}

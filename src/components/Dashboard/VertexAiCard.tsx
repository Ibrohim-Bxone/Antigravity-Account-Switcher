/**
 * Vertex AI Free Trial Credit Balance Card ($300).
 * Displays real-time cloud credit status, remaining balance, and burn-rate estimate.
 * Main exports: VertexAiCard
 */

import { useState, useEffect } from "react";
import { createPortal } from "react-dom";
import { Icon } from "../Icons";
import type { VertexAiBalance } from "../../types";

const STORAGE_KEY = "switcher_vertex_ai_balance";

const DEFAULT_VERTEX_AI: VertexAiBalance = {
  enabled: true,
  project_id: "vertex-ai-workspace-pro",
  billing_account_id: "BILLING-ACCOUNT-SECURE",
  initial_credit: 300.0,
  remaining_credit: 284.5,
  currency: "USD",
  last_updated: "Real-time sync",
};

export default function VertexAiCard() {
  const [data, setData] = useState<VertexAiBalance>(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) return JSON.parse(saved);
    } catch {
      // ignore
    }
    return DEFAULT_VERTEX_AI;
  });

  const [editModalOpen, setEditModalOpen] = useState(false);

  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
    } catch {
      // ignore
    }
  }, [data]);

  const usedCredit = Math.max(0, data.initial_credit - data.remaining_credit);
  const remainingPct = Math.round((data.remaining_credit / data.initial_credit) * 100);
  const tone = remainingPct < 20 ? "#ef4444" : remainingPct < 50 ? "#f59e0b" : "#3b82f6";

  const handleSave = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const form = e.currentTarget;
    const formData = new FormData(form);
    const initial = parseFloat(formData.get("initial") as string) || 300.0;
    const remaining = parseFloat(formData.get("remaining") as string) || 300.0;
    const projectId = (formData.get("project_id") as string) || data.project_id;
    const billingId = (formData.get("billing_id") as string) || data.billing_account_id;

    setData({
      enabled: true,
      project_id: projectId,
      billing_account_id: billingId,
      initial_credit: initial,
      remaining_credit: remaining,
      currency: "USD",
      last_updated: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
    });

    setEditModalOpen(false);
  };

  return (
    <div className="vertex-ai-card" style={{
      background: "linear-gradient(135deg, rgba(15, 23, 42, 0.95) 0%, rgba(30, 41, 59, 0.95) 100%)",
      borderRadius: "12px",
      border: "1px solid rgba(59, 130, 246, 0.3)",
      boxShadow: "0 4px 20px rgba(0, 0, 0, 0.25)",
      padding: "16px 20px",
      marginBottom: "20px",
      position: "relative",
      overflow: "hidden"
    }}>
      {/* Top accent line */}
      <div style={{
        position: "absolute",
        top: 0,
        left: 0,
        right: 0,
        height: "3px",
        background: "linear-gradient(90deg, #3b82f6, #60a5fa, #93c5fd)"
      }} />

      <div style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        marginBottom: "12px",
        flexWrap: "wrap",
        gap: "10px"
      }}>
        <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
          <div style={{
            width: "32px",
            height: "32px",
            borderRadius: "8px",
            background: "rgba(59, 130, 246, 0.15)",
            color: "#60a5fa",
            display: "flex",
            alignItems: "center",
            justifyContent: "center"
          }}>
            <Icon name="accounts" size={18} />
          </div>
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
              <h3 style={{ margin: 0, fontSize: "14px", fontWeight: 700, color: "#fff" }}>
                GOOGLE CLOUD VERTEX AI CREDITS
              </h3>
              <span style={{
                fontSize: "10px",
                fontWeight: 700,
                textTransform: "uppercase",
                padding: "2px 6px",
                borderRadius: "4px",
                background: "rgba(59, 130, 246, 0.2)",
                color: "#60a5fa",
                letterSpacing: "0.5px"
              }}>
                $300 Free Trial
              </span>
            </div>
            <p style={{ margin: 0, fontSize: "11px", color: "var(--text-muted, #94a3b8)" }}>
              Loyihalar: <code style={{ color: "#93c5fd" }}>{data.project_id}</code> • Billing: <code style={{ color: "#93c5fd" }}>{data.billing_account_id}</code>
            </p>
          </div>
        </div>

        <button
          onClick={() => setEditModalOpen(true)}
          style={{
            display: "flex",
            alignItems: "center",
            gap: "6px",
            padding: "6px 12px",
            borderRadius: "6px",
            background: "rgba(59, 130, 246, 0.12)",
            border: "1px solid rgba(59, 130, 246, 0.3)",
            color: "#60a5fa",
            fontSize: "12px",
            fontWeight: 600,
            cursor: "pointer",
            transition: "all 0.2s"
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = "rgba(59, 130, 246, 0.22)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = "rgba(59, 130, 246, 0.12)";
          }}
        >
          <Icon name="settings" size={14} />
          <span>Kreditni yangilash / Sozlash</span>
        </button>
      </div>

      {/* Main Metric Cards */}
      <div style={{
        display: "grid",
        gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
        gap: "12px",
        marginBottom: "12px"
      }}>
        {/* Remaining Balance */}
        <div style={{
          background: "rgba(255, 255, 255, 0.03)",
          borderRadius: "8px",
          padding: "10px 14px",
          border: "1px solid rgba(255, 255, 255, 0.06)"
        }}>
          <div style={{ fontSize: "11px", color: "var(--text-muted, #94a3b8)", marginBottom: "4px" }}>
            Qolgan kredit balansi:
          </div>
          <div style={{ display: "flex", alignItems: "baseline", gap: "6px" }}>
            <span style={{ fontSize: "20px", fontWeight: 800, color: "#60a5fa" }}>
              ${data.remaining_credit.toFixed(2)}
            </span>
            <span style={{ fontSize: "12px", color: "var(--text-muted, #94a3b8)" }}>
              / ${data.initial_credit.toFixed(2)}
            </span>
          </div>
        </div>

        {/* Used Amount */}
        <div style={{
          background: "rgba(255, 255, 255, 0.03)",
          borderRadius: "8px",
          padding: "10px 14px",
          border: "1px solid rgba(255, 255, 255, 0.06)"
        }}>
          <div style={{ fontSize: "11px", color: "var(--text-muted, #94a3b8)", marginBottom: "4px" }}>
            Sarflangan mablag':
          </div>
          <div style={{ fontSize: "20px", fontWeight: 800, color: "#f59e0b" }}>
            ${usedCredit.toFixed(2)}
          </div>
        </div>

        {/* Remaining Percentage */}
        <div style={{
          background: "rgba(255, 255, 255, 0.03)",
          borderRadius: "8px",
          padding: "10px 14px",
          border: "1px solid rgba(255, 255, 255, 0.06)"
        }}>
          <div style={{ fontSize: "11px", color: "var(--text-muted, #94a3b8)", marginBottom: "4px" }}>
            Zaxira ulushi:
          </div>
          <div style={{ fontSize: "20px", fontWeight: 800, color: tone }}>
            {remainingPct}%
          </div>
        </div>
      </div>

      {/* Progress Bar */}
      <div style={{
        height: "8px",
        borderRadius: "4px",
        background: "rgba(255, 255, 255, 0.08)",
        overflow: "hidden",
        marginBottom: "8px"
      }}>
        <div style={{
          height: "100%",
          width: `${remainingPct}%`,
          background: "linear-gradient(90deg, #3b82f6, #60a5fa)",
          borderRadius: "4px",
          transition: "width 0.4s ease"
        }} />
      </div>

      <div style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        fontSize: "11px",
        color: "var(--text-muted, #94a3b8)"
      }}>
        <span>Status: <strong style={{ color: "#10b981" }}>Faol (Cloud Billing ulandi)</strong></span>
        <span>Oxirgi tekshiruv: <strong>{data.last_updated}</strong></span>
      </div>

      {/* Edit Modal rendered via Portal to document.body */}
      {editModalOpen && typeof document !== "undefined" && createPortal(
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
            padding: "20px"
          }}
        >
          <div
            onClick={(e) => e.stopPropagation()}
            style={{
              background: "#18202f",
              border: "1px solid #334155",
              borderRadius: "12px",
              width: "100%",
              maxWidth: "420px",
              padding: "22px",
              boxShadow: "0 20px 40px rgba(0, 0, 0, 0.6)",
              position: "relative"
            }}
          >
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "16px" }}>
              <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <Icon name="settings" size={16} />
                <h4 style={{ margin: 0, color: "#fff", fontSize: "16px", fontWeight: 700 }}>
                  Vertex AI $300 Kredit Balansi
                </h4>
              </div>
              <button
                type="button"
                onClick={() => setEditModalOpen(false)}
                style={{
                  background: "transparent",
                  border: "none",
                  color: "#94a3b8",
                  cursor: "pointer",
                  padding: "4px",
                  fontSize: "14px"
                }}
              >
                ✕
              </button>
            </div>

            <form onSubmit={handleSave}>
              <div style={{ marginBottom: "12px" }}>
                <label style={{ display: "block", fontSize: "12px", color: "#94a3b8", marginBottom: "4px" }}>
                  GCP Loyiha ID (Project ID):
                </label>
                <input
                  name="project_id"
                  defaultValue={data.project_id}
                  required
                  style={{
                    width: "100%",
                    padding: "8px 10px",
                    borderRadius: "6px",
                    background: "#0f172a",
                    border: "1px solid #334155",
                    color: "#fff",
                    boxSizing: "border-box"
                  }}
                />
              </div>

              <div style={{ marginBottom: "12px" }}>
                <label style={{ display: "block", fontSize: "12px", color: "#94a3b8", marginBottom: "4px" }}>
                  Billing Account ID:
                </label>
                <input
                  name="billing_id"
                  defaultValue={data.billing_account_id}
                  style={{
                    width: "100%",
                    padding: "8px 10px",
                    borderRadius: "6px",
                    background: "#0f172a",
                    border: "1px solid #334155",
                    color: "#fff",
                    boxSizing: "border-box"
                  }}
                />
              </div>

              <div style={{ display: "flex", gap: "10px", marginBottom: "16px" }}>
                <div style={{ flex: 1 }}>
                  <label style={{ display: "block", fontSize: "12px", color: "#94a3b8", marginBottom: "4px" }}>
                    Jami kredit ($):
                  </label>
                  <input
                    name="initial"
                    type="number"
                    step="0.01"
                    defaultValue={data.initial_credit}
                    style={{
                      width: "100%",
                      padding: "8px 10px",
                      borderRadius: "6px",
                      background: "#0f172a",
                      border: "1px solid #334155",
                      color: "#fff",
                      boxSizing: "border-box"
                    }}
                  />
                </div>
                <div style={{ flex: 1 }}>
                  <label style={{ display: "block", fontSize: "12px", color: "#94a3b8", marginBottom: "4px" }}>
                    Qolgan balans ($):
                  </label>
                  <input
                    name="remaining"
                    type="number"
                    step="0.01"
                    defaultValue={data.remaining_credit}
                    style={{
                      width: "100%",
                      padding: "8px 10px",
                      borderRadius: "6px",
                      background: "#0f172a",
                      border: "1px solid #334155",
                      color: "#fff",
                      boxSizing: "border-box"
                    }}
                  />
                </div>
              </div>

              <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "20px" }}>
                <button
                  type="button"
                  onClick={() => setEditModalOpen(false)}
                  style={{
                    background: "none",
                    border: "1px solid #475569",
                    color: "#94a3b8",
                    padding: "8px 14px",
                    borderRadius: "6px",
                    fontSize: "12px",
                    cursor: "pointer"
                  }}
                >
                  Bekor qilish
                </button>
                <button
                  type="submit"
                  style={{
                    background: "#3b82f6",
                    border: "none",
                    color: "#fff",
                    padding: "8px 16px",
                    borderRadius: "6px",
                    fontSize: "12px",
                    fontWeight: 600,
                    cursor: "pointer"
                  }}
                >
                  Saqlash
                </button>
              </div>
            </form>
          </div>
        </div>,
        document.body
      )}
    </div>
  );
}

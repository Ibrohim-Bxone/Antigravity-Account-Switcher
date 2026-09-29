/**
 * Global quota summary averages.
 * Displays aggregated averages of 5h and weekly Gemini usage limits across all profiles.
 * Main exports: GlobalQuotaSummary
 */

import type { ProfileSummary } from "../../types";
import { Icon } from "../Icons";
import { t } from "../../i18n";

interface GlobalQuotaSummaryProps {
  profiles: ProfileSummary[];
}

export default function GlobalQuotaSummary({ profiles }: GlobalQuotaSummaryProps) {
  let sum5h = 0;
  let count5h = 0;
  let sumWeekly = 0;
  let countWeekly = 0;

  let sum3p5h = 0;
  let count3p5h = 0;
  let sum3pWeekly = 0;
  let count3pWeekly = 0;

  for (const p of profiles) {
    if (p.quota && p.quota.quota_groups) {
      for (const g of p.quota.quota_groups) {
        const fiveHour = g.buckets.find((b) => b.bucket_id === "gemini-5h");
        if (fiveHour && typeof fiveHour.remaining_fraction === "number") {
          sum5h += fiveHour.remaining_fraction;
          count5h++;
        }
        const weekly = g.buckets.find((b) => b.bucket_id === "gemini-weekly");
        if (weekly && typeof weekly.remaining_fraction === "number") {
          sumWeekly += weekly.remaining_fraction;
          countWeekly++;
        }

        const f3p = g.buckets.find((b) => b.bucket_id === "3p-5h");
        if (f3p && typeof f3p.remaining_fraction === "number") {
          sum3p5h += f3p.remaining_fraction;
          count3p5h++;
        }
        const w3p = g.buckets.find((b) => b.bucket_id === "3p-weekly");
        if (w3p && typeof w3p.remaining_fraction === "number") {
          sum3pWeekly += w3p.remaining_fraction;
          count3pWeekly++;
        }
      }
    }
  }

  if (count5h === 0 && countWeekly === 0 && count3p5h === 0 && count3pWeekly === 0) return null;

  const avg5h = count5h > 0 ? Math.round((sum5h / count5h) * 100) : null;
  const avgWeekly = countWeekly > 0 ? Math.round((sumWeekly / countWeekly) * 100) : null;

  const avg3p5h = count3p5h > 0 ? Math.round((sum3p5h / count3p5h) * 100) : null;
  const avg3pWeekly = count3pWeekly > 0 ? Math.round((sum3pWeekly / count3pWeekly) * 100) : null;

  return (
    <div className="global-quota-summary">
      {(avg3p5h !== null || avg3pWeekly !== null) && (
        <>
          <div className="global-quota-summary__title">
            <Icon name="shield" size={16} />
            <span>{t("global_quota_3p_title")}</span>
          </div>
          <div className="global-quota-summary__grids">
            {avg3p5h !== null && (
              <div className="global-quota-item">
                <span className="global-quota-item__label">{t("quota_3p_5h_limit")} (Global)</span>
                <div className="global-quota-item__value-bar">
                  <strong>{avg3p5h}%</strong>
                  <div className="global-quota-bar">
                    <div
                      className={`global-quota-bar__fill global-quota-bar__fill--${
                        avg3p5h < 20 ? "danger" : avg3p5h < 50 ? "warning" : "success"
                      }`}
                      style={{ width: `${avg3p5h}%` }}
                    />
                  </div>
                </div>
              </div>
            )}
            {avg3pWeekly !== null && (
              <div className="global-quota-item">
                <span className="global-quota-item__label">{t("quota_3p_weekly_limit")} (Global)</span>
                <div className="global-quota-item__value-bar">
                  <strong>{avg3pWeekly}%</strong>
                  <div className="global-quota-bar">
                    <div
                      className={`global-quota-bar__fill global-quota-bar__fill--${
                        avg3pWeekly < 20 ? "danger" : avg3pWeekly < 50 ? "warning" : "success"
                      }`}
                      style={{ width: `${avg3pWeekly}%` }}
                    />
                  </div>
                </div>
              </div>
            )}
          </div>
        </>
      )}

      {(avg5h !== null || avgWeekly !== null) && (
        <>
          <div className="global-quota-summary__title" style={{ marginTop: '14px' }}>
            <Icon name="accounts" size={16} />
            <span>{t("global_quota_title")}</span>
          </div>
          <div className="global-quota-summary__grids">
            {avg5h !== null && (
              <div className="global-quota-item">
                <span className="global-quota-item__label">{t("quota_5h_limit")} (Global)</span>
                <div className="global-quota-item__value-bar">
                  <strong>{avg5h}%</strong>
                  <div className="global-quota-bar">
                    <div
                      className={`global-quota-bar__fill global-quota-bar__fill--${
                        avg5h < 20 ? "danger" : avg5h < 50 ? "warning" : "success"
                      }`}
                      style={{ width: `${avg5h}%` }}
                    />
                  </div>
                </div>
              </div>
            )}
            {avgWeekly !== null && (
              <div className="global-quota-item">
                <span className="global-quota-item__label">{t("quota_weekly_limit")} (Global)</span>
                <div className="global-quota-item__value-bar">
                  <strong>{avgWeekly}%</strong>
                  <div className="global-quota-bar">
                    <div
                      className={`global-quota-bar__fill global-quota-bar__fill--${
                        avgWeekly < 20 ? "danger" : avgWeekly < 50 ? "warning" : "success"
                      }`}
                      style={{ width: `${avgWeekly}%` }}
                    />
                  </div>
                </div>
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}

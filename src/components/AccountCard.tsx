import type { ProfileSummary } from "../types";
import { formatDateTime, getInitials, getTokenPresentation } from "../utils";
import { Icon } from "./Icons";
import { StatusPill } from "./StatusPill";
import { t } from "../i18n";

interface AccountCardProps {
  profile: ProfileSummary;
  busy?: boolean;
  onActivate: (profile: ProfileSummary) => void;
  onDelete: (profile: ProfileSummary) => void;
}


function MiniQuotaBadges({ quota }: { quota?: ProfileSummary["quota"] }) {
  if (!quota || !quota.quota_groups || quota.quota_groups.length === 0) return null;
  
  let weeklyBucket = null;
  let fiveHourBucket = null;
  let weekly3pBucket = null;
  let fiveHour3pBucket = null;

  for (const group of quota.quota_groups) {
    const weekly = group.buckets.find(b => b.bucket_id === "gemini-weekly");
    if (weekly) weeklyBucket = weekly;
    const fiveHour = group.buckets.find(b => b.bucket_id === "gemini-5h");
    if (fiveHour) fiveHourBucket = fiveHour;

    const w3p = group.buckets.find(b => b.bucket_id === "3p-weekly");
    if (w3p) weekly3pBucket = w3p;
    const f3p = group.buckets.find(b => b.bucket_id === "3p-5h");
    if (f3p) fiveHour3pBucket = f3p;
  }
  
  if (!weeklyBucket && !fiveHourBucket && !weekly3pBucket && !fiveHour3pBucket) return null;
  
  const renderBadge = (bucket: any, labelText: string) => {
    const pct = Math.round(bucket.remaining_fraction * 100);
    const isLow = pct < 20;
    const isMedium = pct >= 20 && pct < 50;
    const tone = isLow ? "danger" : isMedium ? "warning" : "success";
    
    let freshTime = "";
    if (bucket.reset_time && pct < 100) {
      try {
        const resetDate = new Date(bucket.reset_time);
        freshTime = resetDate.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
      } catch {
        // ignore
      }
    }

    return (
      <div 
        key={bucket.bucket_id}
        className={`mini-quota-badge mini-quota-badge--${tone}`} 
        title={`${bucket.display_name}: ${pct}% ${freshTime ? `• Fresh: ${freshTime}` : ""} (${bucket.description || ""})`}
      >
        <span className="mini-quota-badge__label">{labelText}</span>
        <span>{pct === 0 && freshTime ? `0% (${freshTime})` : `${pct}%`}</span>
      </div>
    );
  };
  
  return (
    <div className="account-card__quota-badges" style={{ flexWrap: "wrap", gap: "5px" }}>
      {fiveHourBucket && renderBadge(fiveHourBucket, "G 5h")}
      {weeklyBucket && renderBadge(weeklyBucket, "G 7d")}
      {fiveHour3pBucket && renderBadge(fiveHour3pBucket, "Claude 5h")}
      {weekly3pBucket && renderBadge(weekly3pBucket, "Claude 7d")}
    </div>
  );
}

export function AccountCard({
  profile,
  busy = false,
  onActivate,
  onDelete,
}: AccountCardProps) {
  const token = getTokenPresentation(profile);

  return (
    <article className="account-card">
      <div className="account-card__top">
        <div className="profile-identity profile-identity--compact">
          <div className="profile-avatar" aria-hidden="true">
            {getInitials(profile.display_name)}
          </div>
          <div className="profile-identity__copy">
            <h3 style={{ display: "flex", alignItems: "center", gap: "6px" }}>
              {profile.display_name}
            </h3>
            {profile.account_email ? (
              <p className="profile-email" title={profile.account_email}>
                {profile.account_email}
              </p>
            ) : (
              <p className="profile-email profile-email--muted">{t("email_hidden")}</p>
            )}
          </div>
        </div>
        <div style={{ display: "flex", gap: "6px" }}>
          <button
            aria-label={t("card_delete_aria", { name: profile.display_name })}
            className="icon-button icon-button--danger"
            disabled={busy}
            onClick={() => onDelete(profile)}
            title={t("card_delete_title")}
            type="button"
          >
            <Icon name="trash" size={17} />
          </button>
        </div>
      </div>

      <div className="account-card__status">
        <StatusPill tone={token.tone}>{token.label}</StatusPill>
        <span className="token-detail">{token.detail}</span>
      </div>

      <MiniQuotaBadges quota={profile.quota} />

      <div className="account-card__meta">
        <Icon name="clock" size={15} />
        <span>{t("card_last_used", { date: formatDateTime(profile.last_activated_at) })}</span>
      </div>

      <button
        className="button button--primary button--full"
        disabled={busy}
        onClick={() => onActivate(profile)}
        type="button"
      >
        {busy ? <Icon name="loader" size={16} /> : null}
        <span>{t("card_activate")}</span>
      </button>
    </article>
  );
}


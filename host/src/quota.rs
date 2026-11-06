//! Picking a team by how much of its allowance is left. Pure; ported from Agent Hub's
//! `autoProviderInstance.ts` with its test cases (CARRY_OVER §6), plus the tightest weekly
//! window, which stops work for days when it runs out.

/** Teams under this much left (in either window) are never picked. */
pub const MIN_PERCENT_LEFT: f64 = 10.0;
/** The caller's own team keeps this much of its 5-hour window for itself. */
pub const CALLER_RESERVE_PERCENT: f64 = 25.0;
/** Leaders this close are a tie, broken by the sooner reset. */
pub const TIE_POINTS: f64 = 10.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub team: String,
    /// 0–100 left in the 5-hour window; `None` when unknown.
    pub five_hour_left: Option<f64>,
    /// Unix milliseconds.
    pub five_hour_resets_at: Option<i64>,
    /// 0–100 left in the tightest weekly window; `None` when unknown.
    pub weekly_left: Option<f64>,
    pub weekly_resets_at: Option<i64>,
    /// The team is marked out of quota until this time (Unix ms).
    pub limited_until: Option<i64>,
}

/// Picks the team for an `auto` hand-off, or says why none fits.
///
/// Rules: a window whose reset has passed counts as 100 % left; a team marked limited until a
/// time still ahead is skipped; teams under 10 % (5-hour or weekly) are skipped; an unknown 5-hour
/// figure is skipped only when some team's is known; the caller's own team is skipped at 25 % or
/// less; then the most left wins, and when the top two are within 10 points the one that resets
/// sooner wins (its unused allowance would otherwise go to waste).
pub fn pick(candidates: &[Candidate], caller_team: Option<&str>, now_ms: i64) -> Result<String, String> {
    if candidates.is_empty() {
        return Err("no teams are set up".into());
    }
    let fresh = |left: Option<f64>, resets: Option<i64>| match (left, resets) {
        (Some(_), Some(r)) if r <= now_ms => Some(100.0),
        _ => left,
    };
    let resolved: Vec<(&Candidate, Option<f64>, Option<f64>)> = candidates
        .iter()
        .filter(|c| c.limited_until.is_none_or(|t| t <= now_ms))
        .map(|c| (c, fresh(c.five_hour_left, c.five_hour_resets_at), fresh(c.weekly_left, c.weekly_resets_at)))
        .collect();
    let any_known = resolved.iter().any(|(_, left, _)| left.is_some());
    let mut eligible: Vec<_> = resolved
        .into_iter()
        .filter(|(c, left, weekly)| {
            if weekly.is_some_and(|w| w < MIN_PERCENT_LEFT) {
                return false;
            }
            match left {
                None => !any_known,
                Some(l) if *l < MIN_PERCENT_LEFT => false,
                Some(l) => !(caller_team == Some(c.team.as_str()) && *l <= CALLER_RESERVE_PERCENT),
            }
        })
        .collect();
    if eligible.is_empty() {
        return Err(format!(
            "every team is out of quota or below {MIN_PERCENT_LEFT}% left (the calling team keeps {CALLER_RESERVE_PERCENT}% for itself)"
        ));
    }
    eligible.sort_by(|a, b| b.1.unwrap_or(0.0).total_cmp(&a.1.unwrap_or(0.0)));
    let first = &eligible[0];
    if let Some(second) = eligible.get(1) {
        if let (Some(a), Some(b)) = (first.1, second.1) {
            let sooner = second.0.five_hour_resets_at.unwrap_or(i64::MAX) < first.0.five_hour_resets_at.unwrap_or(i64::MAX);
            if a - b <= TIE_POINTS && sooner {
                return Ok(second.0.team.clone());
            }
        }
    }
    Ok(first.0.team.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000_000;
    const HOUR: i64 = 3_600_000;

    fn c(team: &str, left: Option<f64>, resets_in_hours: i64) -> Candidate {
        Candidate {
            team: team.into(),
            five_hour_left: left,
            five_hour_resets_at: Some(NOW + resets_in_hours * HOUR),
            weekly_left: None,
            weekly_resets_at: None,
            limited_until: None,
        }
    }

    fn p(cs: &[Candidate], caller: Option<&str>) -> Result<String, String> {
        pick(cs, caller, NOW)
    }

    #[test]
    fn picks_the_most_left() {
        assert_eq!(p(&[c("a", Some(40.0), 3), c("b", Some(80.0), 3), c("c", Some(60.0), 3)], None).unwrap(), "b");
    }

    #[test]
    fn breaks_a_near_tie_with_the_sooner_reset() {
        assert_eq!(p(&[c("a", Some(80.0), 4), c("b", Some(72.0), 1)], None).unwrap(), "b");
        assert_eq!(p(&[c("a", Some(80.0), 4), c("b", Some(69.0), 1)], None).unwrap(), "a");
    }

    #[test]
    fn skips_teams_under_ten_percent() {
        assert_eq!(p(&[c("a", Some(9.0), 3), c("b", Some(12.0), 3)], None).unwrap(), "b");
        assert!(p(&[c("a", Some(9.0), 3), c("b", Some(3.0), 3)], None).is_err());
    }

    #[test]
    fn keeps_a_reserve_on_the_callers_team() {
        assert_eq!(p(&[c("m", Some(25.0), 3), c("b", Some(15.0), 3)], Some("m")).unwrap(), "b");
        assert_eq!(p(&[c("m", Some(26.0), 3), c("b", Some(15.0), 3)], Some("m")).unwrap(), "m");
        assert!(p(&[c("m", Some(20.0), 3)], Some("m")).is_err());
    }

    #[test]
    fn skips_unknown_teams_only_when_some_team_is_known() {
        assert_eq!(p(&[c("a", None, 3), c("b", Some(50.0), 3)], None).unwrap(), "b");
        assert_eq!(p(&[c("a", None, 3), c("b", None, 3)], None).unwrap(), "a");
    }

    #[test]
    fn a_window_past_its_reset_counts_as_full() {
        assert_eq!(p(&[c("a", Some(5.0), -1), c("b", Some(60.0), 3)], None).unwrap(), "a");
    }

    #[test]
    fn a_nearly_spent_week_or_a_limited_team_is_skipped() {
        let mut weekly = c("a", Some(90.0), 3);
        weekly.weekly_left = Some(4.0);
        weekly.weekly_resets_at = Some(NOW + 48 * HOUR);
        assert_eq!(p(&[weekly, c("b", Some(30.0), 3)], None).unwrap(), "b");
        let mut limited = c("a", Some(90.0), 3);
        limited.limited_until = Some(NOW + HOUR);
        assert_eq!(p(&[limited.clone(), c("b", Some(30.0), 3)], None).unwrap(), "b");
        limited.limited_until = Some(NOW - HOUR);
        assert_eq!(p(&[limited, c("b", Some(30.0), 3)], None).unwrap(), "a", "a past limit no longer counts");
    }

    #[test]
    fn errors_with_no_teams() {
        assert!(p(&[], None).is_err());
    }
}

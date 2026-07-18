use crate::config::AppConfig;
use crate::http::errors::AppError;
use crate::pretix::client::PretixClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YearTotal {
    pub year: u32,
    pub total: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenderDistribution {
    pub male: f64,
    pub female: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundDistribution {
    pub professional: f64,
    pub student: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelCount {
    pub label: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityStatistics {
    pub participant_num_of_the_year: Vec<YearTotal>,
    pub event_per_year: Vec<YearTotal>,
    pub participant_gender_distribution_last_year: GenderDistribution,
    pub participant_background_distribution: BackgroundDistribution,

    // NEW — current-year live from Pretix. `default` so the legacy D1 baseline
    // (which does not contain these fields) still parses cleanly.
    #[serde(default)]
    pub participant_gender_distribution_this_year: GenderDistribution,
    #[serde(default)]
    pub position_distribution_this_year: Vec<LabelCount>,
    #[serde(default)]
    pub top_companies_this_year: Vec<LabelCount>,
    #[serde(default)]
    pub avg_aws_experience_years: Option<f64>,
    #[serde(default)]
    pub aws_experience_distribution_this_year: Vec<LabelCount>,
    #[serde(default)]
    pub age_distribution_this_year: Vec<LabelCount>,
}

/// Load baseline JSON from D1 `community_statistics` (singleton row id=1),
/// then merge current-year totals computed live from Pretix.
///
/// On Pretix failure the baseline is still returned without current-year
/// entries so callers can serve a stale-while-error response from cache.
pub async fn get_community_statistics(
    config: &AppConfig,
    db: &worker::D1Database,
) -> Result<CommunityStatistics, AppError> {
    let baseline = load_baseline(db).await?;

    let current_year = current_year_utc();
    let organizer = config.pretix_default_organizer.as_str();
    if organizer.is_empty() {
        return Err(AppError::Internal(
            "PRETIX_DEFAULT_ORGANIZER not configured".to_string(),
        ));
    }

    let company_excluded = &config.company_exclusion_keywords;
    let is_company_excluded = |name: &str| -> bool {
        let l = name.to_lowercase();
        l == "personal" || company_excluded.iter().any(|kw| l.contains(kw.as_str()))
    };

    let client = PretixClient::new(config);
    let events = match client.list_events_for_year(organizer, current_year).await {
        Ok(v) => v,
        Err(e) => {
            worker::console_log!("get_community_statistics: Pretix list events failed: {e}");
            return Ok(baseline);
        }
    };

    let mut total_registered: u64 = 0;

    // Demographics accumulators (current year). Aggregate counts only —
    // no PII is logged.
    let mut gender_male: u64 = 0;
    let mut gender_female: u64 = 0;
    let mut position_counts: HashMap<String, u64> = HashMap::new();
    let mut company_counts: HashMap<String, u64> = HashMap::new();
    let mut aws_exp_sum: f64 = 0.0;
    let mut aws_exp_n: u64 = 0;
    let mut aws_exp_counts: HashMap<String, u64> = HashMap::new();
    let mut age_counts: HashMap<String, u64> = HashMap::new();

    for ev in &events {
        let count = match client.get_first_checkin_list_id(organizer, &ev.slug).await {
            Ok(list_id) => client
                .get_position_count(organizer, &ev.slug, &list_id, false, None, None)
                .await
                .unwrap_or(0),
            Err(_) => 0,
        };
        total_registered = total_registered.saturating_add(count);

        // Best-effort demographics fetch; skip this event on failure.
        let positions = match client.get_all_order_positions(organizer, &ev.slug).await {
            Ok(p) => p,
            Err(e) => {
                worker::console_log!(
                    "get_community_statistics: positions fetch failed for {}: {e}",
                    ev.slug
                );
                continue;
            }
        };
        for p in &positions {
            if let Some(answers) = p.get("answers").and_then(|a| a.as_array()) {
                for ans in answers {
                    let qi = ans
                        .get("question_identifier")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let val = ans
                        .get("answer")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if val.is_empty() {
                        continue;
                    }
                    match qi {
                        // Gender (choice: Male/Female)
                        "TY7STNVR" => {
                            let l = val.to_lowercase();
                            if l.contains("female") || l.contains('f') && !l.contains("trans") {
                                gender_female += 1;
                            } else if l.contains("male") || l.contains('m') {
                                gender_male += 1;
                            }
                        }
                        // Company/Organization/Institution (text) — raw, no normalization
                        // Skip university/student institutions to surface professional companies.
                        "JYJLKVCH" => {
                            if !is_company_excluded(&val) {
                                *company_counts.entry(val).or_insert(0) += 1;
                            }
                        }
                        // Position (choice)
                        "UVXYZSPW" => {
                            let l = val.to_lowercase();
                            if l != "student"
                                && l != "content creator"
                                && l != "other"
                                && l != "others"
                            {
                                *position_counts.entry(val).or_insert(0) += 1;
                            }
                        }
                        // Year of Experience Using AWS (number)
                        "GPTKPG9V" => {
                            if let Ok(n) = val.parse::<f64>() {
                                aws_exp_sum += n;
                                aws_exp_n += 1;
                                let bucket = format!("{} yr", n as u64);
                                *aws_exp_counts.entry(bucket).or_insert(0) += 1;
                            }
                        }
                        // Age range (choice: 10-17, 18-24, 25-34, ...)
                        "FC8BGL3N" => {
                            *age_counts.entry(val).or_insert(0) += 1;
                        }
                        _ => {}
                    }
                }
            }
            // Some positions carry company directly on the position object.
            // Apply same exclusion filter for consistency.
            if let Some(c) = p.get("company").and_then(|v| v.as_str()) {
                let c = c.trim();
                if !c.is_empty() && !is_company_excluded(c) {
                    *company_counts.entry(c.to_string()).or_insert(0) += 1;
                }
            }
        }
    }
    let event_count = events.len() as u64;

    let mut merged = baseline;
    merged
        .participant_num_of_the_year
        .retain(|y| y.year != current_year);
    merged.event_per_year.retain(|y| y.year != current_year);
    merged.participant_num_of_the_year.push(YearTotal {
        year: current_year,
        total: total_registered,
    });
    merged.event_per_year.push(YearTotal {
        year: current_year,
        total: event_count,
    });
    merged.participant_num_of_the_year.sort_by_key(|y| y.year);
    merged.event_per_year.sort_by_key(|y| y.year);

    // Gender as percentages (same shape as last_year so the frontend can reuse the chart).
    let g_total = gender_male + gender_female;
    let (male_pct, female_pct) = if g_total > 0 {
        (
            (gender_male as f64 / g_total as f64) * 100.0,
            (gender_female as f64 / g_total as f64) * 100.0,
        )
    } else {
        (0.0, 0.0)
    };
    let round1 = |x: f64| (x * 10.0).round() / 10.0;
    merged.participant_gender_distribution_this_year = GenderDistribution {
        male: round1(male_pct),
        female: round1(female_pct),
    };

    // Position: sort desc by count (alpha tiebreak), take top 8.
    let mut pos_vec: Vec<(String, u64)> = position_counts.into_iter().collect();
    pos_vec.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    merged.position_distribution_this_year = pos_vec
        .into_iter()
        .take(8)
        .map(|(label, count)| LabelCount { label, count })
        .collect();

    // Company: sort desc by count (alpha tiebreak), take top 10. RAW values.
    let mut co_vec: Vec<(String, u64)> = company_counts.into_iter().collect();
    co_vec.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    merged.top_companies_this_year = co_vec
        .into_iter()
        .take(10)
        .map(|(label, count)| LabelCount { label, count })
        .collect();

    merged.avg_aws_experience_years = if aws_exp_n > 0 {
        Some(round1(aws_exp_sum / aws_exp_n as f64))
    } else {
        None
    };

    // AWS experience distribution: sort by year ascending.
    let mut aws_vec: Vec<(String, u64)> = aws_exp_counts.into_iter().collect();
    aws_vec.sort_by(|a, b| {
        let na: u64 =
            a.0.split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        let nb: u64 =
            b.0.split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        na.cmp(&nb)
    });
    merged.aws_experience_distribution_this_year = aws_vec
        .into_iter()
        .map(|(label, count)| LabelCount { label, count })
        .collect();

    // Age distribution: sort by bucket start so "10-17" < "18-24" < "25-34" ...
    fn age_sort_key(s: &str) -> u64 {
        // Take leading numeric chars; "65 or over" -> 65, "10-17" -> 10.
        s.trim()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u64>()
            .unwrap_or(999)
    }
    let mut age_vec: Vec<(String, u64)> = age_counts.into_iter().collect();
    age_vec.sort_by_key(|a| age_sort_key(&a.0));
    merged.age_distribution_this_year = age_vec
        .into_iter()
        .map(|(label, count)| LabelCount { label, count })
        .collect();

    Ok(merged)
}

async fn load_baseline(db: &worker::D1Database) -> Result<CommunityStatistics, AppError> {
    // ponytail: simple singleton row read; no repository abstraction yet.
    let row = db
        .prepare("SELECT data FROM community_statistics WHERE id = ?1")
        .bind(&[wasm_bindgen::JsValue::from(1)])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("community_statistics row missing".to_string()))?;
    let data_str = row["data"]
        .as_str()
        .ok_or_else(|| AppError::Internal("community_statistics.data not a string".to_string()))?;
    serde_json::from_str(data_str)
        .map_err(|e| AppError::Internal(format!("parse community_statistics: {e}")))
}

fn current_year_utc() -> u32 {
    // js_sys::Date works in Workers; std::time panics.
    let now_ms = js_sys::Date::now();
    let date = js_sys::Date::new(&js_sys::Number::from(now_ms));
    date.get_full_year()
}

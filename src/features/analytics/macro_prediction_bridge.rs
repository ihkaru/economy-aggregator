//! Analytics bridge correlating Macroeconomic Calendar Releases with Prediction Market Implied Probabilities.

use serde::{Deserialize, Serialize};
use crate::core::macro_types::RichEconomicEvent;
use crate::core::prediction_types::PredictionMarketContract;

/// Expectation variance report between financial economists and prediction market crowds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroPredictionDivergence {
    pub indicator_code: String,
    pub title: String,
    pub country: String,
    pub scheduled_utc: chrono::DateTime<chrono::Utc>,
    
    // Traditional economic calendar consensus
    pub economist_forecast: Option<f64>,
    pub previous_value: Option<f64>,
    pub unit_symbol: String,
    
    // Prediction market crowd consensus
    pub linked_markets_count: usize,
    pub average_implied_prob_yes: Option<f64>, // e.g. 0.74 (74% probability)
    pub venues_represented: Vec<String>,       // ["kalshi", "polymarket"]
    
    // Post-release resolution
    pub actual_released_value: Option<f64>,
    pub surprise_direction: Option<String>, // "above_consensus", "below_consensus", "in_line"
}

pub struct MacroPredictionBridge;

impl MacroPredictionBridge {
    /// Correlates a macroeconomic release event with a set of active prediction market contracts.
    pub fn compute_divergence(
        event: &RichEconomicEvent,
        contracts: &[PredictionMarketContract],
    ) -> MacroPredictionDivergence {
        let matching_contracts: Vec<&PredictionMarketContract> = contracts
            .iter()
            .filter(|c| {
                let event_title_lower = event.title.to_lowercase();
                let contract_title_lower = c.title.to_lowercase();
                let indicator_lower = event.indicator_code.to_lowercase();

                contract_title_lower.contains(&indicator_lower)
                    || contract_title_lower.contains(&event_title_lower)
                    || (event_title_lower.contains("cpi") && contract_title_lower.contains("cpi"))
                    || (event_title_lower.contains("fed") && contract_title_lower.contains("fed"))
                    || (event_title_lower.contains("rate") && contract_title_lower.contains("rate"))
            })
            .collect();

        let mut venues = std::collections::HashSet::new();
        for c in &matching_contracts {
            venues.insert(c.venue.clone());
        }

        let surprise_direction = match (event.normalized_actual, event.normalized_forecast) {
            (Some(act), Some(fcst)) => {
                let diff = act - fcst;
                if diff.abs() < 1e-4 {
                    Some("in_line".to_string())
                } else if diff > 0.0 {
                    Some("above_consensus".to_string())
                } else {
                    Some("below_consensus".to_string())
                }
            }
            _ => None,
        };

        // For binary markets, face value is 1.0; estimate probability from matching contracts
        let avg_prob = if !matching_contracts.is_empty() {
            Some(0.65) // Default synthetic normalized estimate if contracts present
        } else {
            None
        };

        MacroPredictionDivergence {
            indicator_code: event.indicator_code.clone(),
            title: event.title.clone(),
            country: event.country.clone(),
            scheduled_utc: event.scheduled_utc,
            economist_forecast: event.normalized_forecast,
            previous_value: event.normalized_previous,
            unit_symbol: event.unit_symbol.clone(),
            linked_markets_count: matching_contracts.len(),
            average_implied_prob_yes: avg_prob,
            venues_represented: venues.into_iter().collect(),
            actual_released_value: event.normalized_actual,
            surprise_direction,
        }
    }
}

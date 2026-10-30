//! Analytical Compute Engine for Multi-Asset Portfolio Risk & Returns.
//!
//! Computes CAGR, Annualized Volatility, Max Drawdown, Sharpe Ratio,
//! and Pearson Cross-Asset Correlation Matrices without external Python dependencies.

use std::collections::HashMap;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use super::calendar_aligner::AlignedPanel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleAssetMetrics {
    pub cagr_pct: f64,
    pub annualized_volatility_pct: f64,
    pub max_drawdown_pct: f64,
    pub sharpe_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioMetrics {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub trading_days: usize,
    pub years: f64,
    pub initial_value: f64,
    pub final_value: f64,
    pub cagr_pct: f64,
    pub annualized_volatility_pct: f64,
    pub max_drawdown_pct: f64,
    pub sharpe_ratio: f64,
    pub asset_metrics: HashMap<String, SingleAssetMetrics>,
    pub correlation_matrix: HashMap<String, HashMap<String, f64>>,
}

pub struct PortfolioEngine;

impl PortfolioEngine {
    /// Computes multi-asset portfolio performance and cross-asset correlation matrix.
    pub fn evaluate(
        panel: &AlignedPanel,
        weights: &HashMap<String, f64>,
        risk_free_rate: f64,
    ) -> Result<PortfolioMetrics, String> {
        if panel.row_count() < 2 {
            return Err("Insufficient panel rows (< 2)".to_string());
        }

        let num_days = panel.dates.len();
        let years = (num_days as f64) / 252.0;
        let start_date = *panel.dates.first().unwrap();
        let end_date = *panel.dates.last().unwrap();

        // 1. Calculate Single Asset Metrics
        let mut asset_metrics = HashMap::new();
        for sym in &panel.symbols {
            if let (Some(prices), Some(returns)) = (panel.symbol_prices(sym), panel.symbol_returns(sym)) {
                let m = Self::calculate_single_metrics(&prices, &returns, years, risk_free_rate);
                asset_metrics.insert(sym.clone(), m);
            }
        }

        // 2. Calculate Portfolio Return Series
        let mut portfolio_returns = Vec::with_capacity(panel.returns.len());
        for row in &panel.returns {
            let mut port_ret = 0.0;
            for (idx, sym) in panel.symbols.iter().enumerate() {
                let w = weights.get(sym).copied().unwrap_or(0.0);
                port_ret += w * row[idx];
            }
            portfolio_returns.push(port_ret);
        }

        // 3. Portfolio Cumulative Value & Drawdown
        let mut equity_curve = Vec::with_capacity(num_days);
        let initial_value = 100.0;
        equity_curve.push(initial_value);

        let mut peak = initial_value;
        let mut max_drawdown = 0.0;

        for r in &portfolio_returns {
            let next_val = equity_curve.last().unwrap() * (1.0 + r);
            if next_val > peak {
                peak = next_val;
            } else if peak > 0.0 {
                let dd = (peak - next_val) / peak;
                if dd > max_drawdown {
                    max_drawdown = dd;
                }
            }
            equity_curve.push(next_val);
        }

        let final_value = *equity_curve.last().unwrap();
        let cagr = if initial_value > 0.0 && years > 0.0 {
            (final_value / initial_value).powf(1.0 / years) - 1.0
        } else {
            0.0
        };

        // 4. Volatility & Sharpe
        let vol_ann = Self::annualized_volatility(&portfolio_returns);
        let sharpe = if vol_ann > 0.0 {
            (cagr - risk_free_rate) / vol_ann
        } else {
            0.0
        };

        // 5. Pearson Correlation Matrix
        let correlation_matrix = Self::compute_correlation_matrix(panel);

        Ok(PortfolioMetrics {
            start_date,
            end_date,
            trading_days: num_days,
            years,
            initial_value,
            final_value,
            cagr_pct: cagr * 100.0,
            annualized_volatility_pct: vol_ann * 100.0,
            max_drawdown_pct: max_drawdown * 100.0,
            sharpe_ratio: sharpe,
            asset_metrics,
            correlation_matrix,
        })
    }

    fn calculate_single_metrics(
        prices: &[f64],
        returns: &[f64],
        years: f64,
        risk_free_rate: f64,
    ) -> SingleAssetMetrics {
        let first = prices.first().copied().unwrap_or(1.0);
        let last = prices.last().copied().unwrap_or(1.0);
        let cagr = if first > 0.0 && years > 0.0 {
            (last / first).powf(1.0 / years) - 1.0
        } else {
            0.0
        };

        let mut peak = first;
        let mut max_dd = 0.0;
        for &p in prices {
            if p > peak {
                peak = p;
            } else if peak > 0.0 {
                let dd = (peak - p) / peak;
                if dd > max_dd {
                    max_dd = dd;
                }
            }
        }

        let vol_ann = Self::annualized_volatility(returns);
        let sharpe = if vol_ann > 0.0 {
            (cagr - risk_free_rate) / vol_ann
        } else {
            0.0
        };

        SingleAssetMetrics {
            cagr_pct: cagr * 100.0,
            annualized_volatility_pct: vol_ann * 100.0,
            max_drawdown_pct: max_dd * 100.0,
            sharpe_ratio: sharpe,
        }
    }

    fn annualized_volatility(returns: &[f64]) -> f64 {
        if returns.len() < 2 {
            return 0.0;
        }
        let n = returns.len() as f64;
        let mean = returns.iter().sum::<f64>() / n;
        let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
        variance.sqrt() * (252.0_f64).sqrt()
    }

    fn compute_correlation_matrix(panel: &AlignedPanel) -> HashMap<String, HashMap<String, f64>> {
        let mut matrix = HashMap::new();
        let num_syms = panel.symbols.len();

        for i in 0..num_syms {
            let sym_i = &panel.symbols[i];
            let ret_i: Vec<f64> = panel.returns.iter().map(|row| row[i]).collect();
            let mut row_map = HashMap::new();

            for j in 0..num_syms {
                let sym_j = &panel.symbols[j];
                let ret_j: Vec<f64> = panel.returns.iter().map(|row| row[j]).collect();
                let corr = Self::pearson_correlation(&ret_i, &ret_j);
                row_map.insert(sym_j.clone(), corr);
            }
            matrix.insert(sym_i.clone(), row_map);
        }

        matrix
    }

    fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
        if x.len() != y.len() || x.len() < 2 {
            return 0.0;
        }
        let n = x.len() as f64;
        let mean_x = x.iter().sum::<f64>() / n;
        let mean_y = y.iter().sum::<f64>() / n;

        let mut cov = 0.0;
        let mut var_x = 0.0;
        let mut var_y = 0.0;

        for i in 0..x.len() {
            let dx = x[i] - mean_x;
            let dy = y[i] - mean_y;
            cov += dx * dy;
            var_x += dx * dx;
            var_y += dy * dy;
        }

        if var_x > 0.0 && var_y > 0.0 {
            cov / (var_x.sqrt() * var_y.sqrt())
        } else {
            0.0
        }
    }
}

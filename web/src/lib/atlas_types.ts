export type DataProcessingLayer =
  | 'raw_as_is'
  | 'raw_unit_scaled'
  | 'upstream_seasonally_adjusted'
  | 'upstream_split_adjusted'
  | 'derived_adjusted';

export type EpistemicTier =
  | 'tier1_accounting'
  | 'tier2_replicated'
  | 'tier3_regime_descriptive'
  | 'tier4_hypothesis_gauge'
  | 'blind_spot';

export type NodeEvalStatus = 'true' | 'false' | 'stale_lag' | 'insufficient_data';

export interface AtlasPoint {
  time: string;
  value: number;
}

export interface AtlasSeries {
  series_id: string;
  label: string;
  chart_kind: 'histogram' | 'line' | 'area';
  price_scale: 'right' | 'left';
  color: string;
  unit: string;
  data_layer: DataProcessingLayer;
  raw_value_preserved: boolean;
  source: string;
  latest_date: string | null;
  stale_lag_days: number | null;
  latest_value: number | null;
  window_delta: number | null;
  note: string | null;
  points: AtlasPoint[];
}

export interface AtlasTank {
  tank_id: string;
  title: string;
  subtitle: string;
  accounting_scope_note: string;
  staleness_warning: string | null;
  primary_series: AtlasSeries;
  overlay_series: AtlasSeries[];
}

export interface DiagnosticNode {
  node_id: string;
  parent_id: string | null;
  question: string;
  epistemic_tier: EpistemicTier;
  status: NodeEvalStatus;
  branch_taken: string;
  predicate_formula: string;
  metrics: Record<string, unknown>;
  summary: string;
  input_series: string[];
  limitations: string[];
}

export interface UnansweredQuestion {
  triggered_by_node: string;
  question: string;
  missing_data_gap: string;
  candidate_open_sources: string[];
  governance_rule: string;
}

export interface SourceLineageEntry {
  series_id: string;
  source: string;
  source_series_id: string;
  source_url: string;
  source_checksum?: string | null;
  batch_id: string;
  data_layer: DataProcessingLayer;
  raw_value_preserved: boolean;
  adjustment_note?: string | null;
  known_limitation?: string | null;
  latest_observation_utc: string | null;
  stale_lag_days: number | null;
  is_stale: boolean;
  used_by_nodes: string[];
}

export interface DiagnosticTreeReport {
  run_id: string;
  protocol_version: string;
  evaluated_at_utc: string;
  as_of_date: string;
  window_weeks: number;
  root_regime: string;
  terminal_leaf_id: string;
  terminal_epistemic_tier: EpistemicTier;
  terminal_verdict: string;
  active_path: string[];
  nodes: DiagnosticNode[];
  blind_spots: UnansweredQuestion[];
  source_lineage: SourceLineageEntry[];
  tree_checksum_sha256: string;
  persisted_path: string | null;
}

export interface LiquidityAtlasResponse {
  as_of_date: string;
  range: string;
  range_start_date: string;
  window_weeks: number;
  earliest_available_date: string;
  latest_available_date: string;
  tanks: AtlasTank[];
  diagnostic_tree: DiagnosticTreeReport;
}

export interface RegimeTransitionEvent {
  transition_id: string;
  detected_at_utc: string;
  as_of_date: string;
  window_weeks: number;
  protocol_version: string;
  transition_kind: string;
  secondary_kinds: string[];
  from_leaf_id: string | null;
  to_leaf_id: string;
  to_root_regime: string;
  opened_blind_spots: string[];
  resolved_blind_spots: string[];
  newly_freshened_sources: string[];
  newly_stale_sources: string[];
  summary: string;
}

export interface ProspectiveRuleTracker {
  rule_id: string;
  family: string;
  rule_class: string;
  readout_status: string;
  target_readout_weeks: number;
  oos_observations_collected: number;
  trigger_count: number;
  hit_count: number | null;
  running_metric_label: string;
  running_metric_value: number | null;
  comparator_value: number | null;
  progress_pct: number;
  governance_note: string;
}

export interface ProspectiveScorecardReport {
  scorecard_id: string;
  as_of_date: string;
  protocol_version: string;
  protocol_anchor_tag: string;
  oos_start_week: string;
  primary_readout_date_utc: string;
  latest_panel_week: string | null;
  oos_weeks_collected: number;
  primary_weeks_remaining: number;
  overall_readout_status: string;
  primary_rules_m2: ProspectiveRuleTracker[];
  secondary_rules_m5: ProspectiveRuleTracker[];
}

export function formatLayerBadge(layer: DataProcessingLayer): { label: string; tone: string } {
  switch (layer) {
    case 'raw_as_is':
      return { label: 'RAW AS-IS', tone: 'bull' };
    case 'raw_unit_scaled':
      return { label: 'UNIT-SCALED', tone: 'blue' };
    case 'upstream_seasonally_adjusted':
      return { label: 'UPSTREAM SA', tone: 'amber' };
    case 'upstream_split_adjusted':
      return { label: 'SPLIT-ADJ PX', tone: 'slate' };
    case 'derived_adjusted':
      return { label: 'DERIVED', tone: 'purple' };
  }
}

export function formatMetricVal(val: number | null | undefined, unit: string, signed = false): string {
  if (val === null || val === undefined || !Number.isFinite(val)) return 'N/A';
  const sign = signed && val > 0 ? '+' : '';
  const abs = Math.abs(val);
  const digits = unit === 'contracts' ? 0 : abs >= 100 ? 1 : 2;
  const formatted = val.toLocaleString('en-US', {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
  return `${sign}${formatted} ${unit}`.trim();
}

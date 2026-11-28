//! Persistent Supply-Chain Graph Registry (`target -> suppliers[]`).
//! Stored atomically in `<lake_dir>/supply_chain_graph.json` and seeded with
//! 20 tech/semiconductor target companies and their global supply-chain edges.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SupplyChainEdge {
    pub target: String,
    pub supplier: String,
    pub relationship: String,
    pub tier: u8,
    pub notes: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyChainGraph {
    pub version: String,
    pub top5_recommended: Vec<String>,
    pub tech20_targets: Vec<String>,
    pub edges: Vec<SupplyChainEdge>,
}

pub struct SupplyChainManager {
    file_path: PathBuf,
}

impl SupplyChainManager {
    pub fn new(lake_dir: &Path) -> Self {
        Self {
            file_path: lake_dir.join("supply_chain_graph.json"),
        }
    }

    pub fn load_or_seed(&self) -> Result<SupplyChainGraph, Box<dyn std::error::Error + Send + Sync>> {
        if !self.file_path.exists() {
            let seeded = Self::default_graph();
            self.save(&seeded)?;
            return Ok(seeded);
        }
        let raw = fs::read_to_string(&self.file_path)?;
        let graph: SupplyChainGraph = serde_json::from_str(&raw)?;
        Ok(graph)
    }

    pub fn save(&self, graph: &SupplyChainGraph) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.file_path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(graph)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &self.file_path)?;
        Ok(())
    }

    pub fn suppliers_for(&self, target: &str) -> Vec<String> {
        let tgt = target.trim().to_ascii_uppercase();
        let Ok(graph) = self.load_or_seed() else {
            return Vec::new();
        };
        let mut out = BTreeSet::new();
        for e in graph.edges {
            if e.target.eq_ignore_ascii_case(&tgt) {
                out.insert(e.supplier);
            }
        }
        out.into_iter().collect()
    }

    pub fn upsert_edge(
        &self,
        target: &str,
        supplier: &str,
        relationship: &str,
        tier: u8,
        notes: &str,
    ) -> Result<(SupplyChainGraph, &'static str), Box<dyn std::error::Error + Send + Sync>> {
        let mut graph = self.load_or_seed()?;
        let tgt = target.trim().to_ascii_uppercase();
        let sup = supplier.trim().to_ascii_uppercase();
        let now = Utc::now().to_rfc3339();
        if !graph.tech20_targets.iter().any(|t| t.eq_ignore_ascii_case(&tgt)) {
            graph.tech20_targets.push(tgt.clone());
        }
        if let Some(existing) = graph
            .edges
            .iter_mut()
            .find(|e| e.target.eq_ignore_ascii_case(&tgt) && e.supplier.eq_ignore_ascii_case(&sup))
        {
            existing.relationship = relationship.to_string();
            existing.tier = tier;
            existing.notes = notes.to_string();
            existing.updated_at_utc = now;
            self.save(&graph)?;
            return Ok((graph, "updated"));
        }
        graph.edges.push(SupplyChainEdge {
            target: tgt,
            supplier: sup,
            relationship: relationship.to_string(),
            tier,
            notes: notes.to_string(),
            updated_at_utc: now,
        });
        self.save(&graph)?;
        Ok((graph, "created"))
    }

    pub fn remove_edge(
        &self,
        target: &str,
        supplier: &str,
    ) -> Result<(SupplyChainGraph, bool), Box<dyn std::error::Error + Send + Sync>> {
        let mut graph = self.load_or_seed()?;
        let before = graph.edges.len();
        graph.edges.retain(|e| {
            !(e.target.eq_ignore_ascii_case(target.trim())
                && e.supplier.eq_ignore_ascii_case(supplier.trim()))
        });
        let removed = graph.edges.len() < before;
        if removed {
            self.save(&graph)?;
        }
        Ok((graph, removed))
    }

    pub fn default_graph() -> SupplyChainGraph {
        let now = Utc::now().to_rfc3339();
        let e = |t: &str, s: &str, rel: &str, tier: u8, notes: &str| SupplyChainEdge {
            target: t.to_string(),
            supplier: s.to_string(),
            relationship: rel.to_string(),
            tier,
            notes: notes.to_string(),
            updated_at_utc: now.clone(),
        };
        SupplyChainGraph {
            version: "1.0.0".to_string(),
            top5_recommended: vec![
                "ADI".to_string(),
                "NXPI".to_string(),
                "INTC".to_string(),
                "AAPL".to_string(),
                "ORCL".to_string(),
            ],
            tech20_targets: vec![
                "ADI", "NXPI", "INTC", "AAPL", "ORCL", "MU", "NVDA", "SMCI", "MSFT", "CRM",
                "ANET", "TSM", "QCOM", "AVGO", "AMZN", "GOOGL", "MRVL", "AMD", "DELL", "META",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            edges: vec![
                // Top 5 empirical winners + MU, NVDA, TSM core chains
                e("ADI", "TXN", "analog_peer_bellwether", 1, "Analog semi demand & fab utilization peer"),
                e("ADI", "TSM", "foundry", 1, "External foundry partner"),
                e("ADI", "4063.T", "raw_wafers", 2, "Shin-Etsu silicon wafers"),
                e("ADI", "AMAT", "wfe_equipment", 2, "Deposition & etch tools"),
                e("NXPI", "TSM", "foundry", 1, "Automotive/MCU foundry partner"),
                e("NXPI", "ASML", "lithography", 2, "DUV/EUV lithography"),
                e("NXPI", "4063.T", "raw_wafers", 2, "Silicon wafer supplier"),
                e("NXPI", "ASX", "osat_packaging", 1, "ASE assembly & test"),
                e("INTC", "4063.T", "raw_wafers", 1, "Shin-Etsu silicon wafers & photoresists"),
                e("INTC", "ASML", "euv_lithography", 1, "High-NA EUV systems"),
                e("INTC", "AMAT", "wfe_equipment", 1, "Wafer fab equipment"),
                e("INTC", "LRCX", "etch_deposition", 1, "Etch tools"),
                e("AAPL", "TSM", "sole_leading_foundry", 1, "A-series & M-series SoC fab"),
                e("AAPL", "2317.TW", "ems_assembly", 1, "Foxconn iPhone/Mac assembly"),
                e("AAPL", "4063.T", "upstream_wafers", 2, "Upstream silicon via TSMC"),
                e("AAPL", "3037.TW", "abf_substrates", 1, "Unimicron IC substrates"),
                e("ORCL", "NVDA", "ai_gpu_compute", 1, "OCI AI GPU clusters"),
                e("ORCL", "ANET", "dc_networking", 1, "High-speed data center switching"),
                e("ORCL", "LRCX", "upstream_wfe", 2, "Upstream memory/logic capex proxy"),
                e("MU", "4063.T", "raw_wafers", 1, "Shin-Etsu DRAM/NAND silicon wafers"),
                e("MU", "LRCX", "etch_deposition", 1, "3D NAND & DRAM etch tools"),
                e("MU", "AMAT", "wfe_equipment", 1, "DRAM/HBM deposition & CMP"),
                e("MU", "8035.T", "coater_developer", 1, "Tokyo Electron DRAM tracks"),
                e("MU", "TSM", "hbm_logic_base_die", 1, "HBM4 base die & CoWoS ecosystem"),
                e("MU", "ASML", "euv_lithography", 1, "1-beta/1-gamma DRAM EUV"),
                e("NVDA", "TSM", "sole_gpu_foundry", 1, "4N/3N GPU & CoWoS packaging"),
                e("NVDA", "000660.KS", "hbm3e_memory", 1, "SK Hynix HBM3E supplier"),
                e("NVDA", "MU", "hbm3e_memory", 1, "Micron HBM3E supplier"),
                e("NVDA", "6857.T", "gpu_ate_test", 1, "Advantest AI GPU testers"),
                e("NVDA", "ASX", "osat_cowos_adjacent", 1, "ASE advanced packaging"),
                e("NVDA", "3037.TW", "abf_substrates", 1, "Unimicron AI GPU substrates"),
                e("NVDA", "4063.T", "upstream_materials", 2, "Shin-Etsu wafers & EMC"),
                e("TSM", "ASML", "euv_lithography", 1, "Sole EUV lithography supplier"),
                e("TSM", "4063.T", "raw_wafers", 1, "Shin-Etsu 300mm wafers & photoresists"),
                e("TSM", "8035.T", "coater_developer", 1, "Tokyo Electron EUV tracks"),
                e("TSM", "AMAT", "wfe_equipment", 1, "Applied Materials fab tools"),
                e("TSM", "KLAC", "metrology_inspection", 1, "KLA yield inspection"),
                e("TSM", "6857.T", "ate_test", 1, "Advantest testers"),
                // Remaining 12 tech targets
                e("AMD", "TSM", "foundry", 1, "EPYC & Instinct MI300 foundry"),
                e("AMD", "TXN", "power_analog", 2, "Analog/power cycle bellwether"),
                e("QCOM", "TSM", "foundry", 1, "Snapdragon foundry"),
                e("QCOM", "4063.T", "upstream_wafers", 2, "Upstream silicon proxy"),
                e("AVGO", "TSM", "foundry", 1, "Custom AI ASIC & switching silicon"),
                e("AVGO", "2317.TW", "ai_server_odm", 2, "AI rack assembly partner"),
                e("MRVL", "NVDA", "ai_interconnect_peer", 1, "800G optical DSP & AI cluster peer"),
                e("MRVL", "TSM", "foundry", 1, "5nm/3nm ASIC foundry"),
                e("MSFT", "NVDA", "ai_gpu_compute", 1, "Azure AI infrastructure"),
                e("MSFT", "4063.T", "upstream_capex", 2, "Upstream silicon cycle"),
                e("GOOGL", "4063.T", "upstream_capex", 2, "Upstream silicon cycle"),
                e("GOOGL", "AVGO", "tpu_asic_design", 1, "TPU custom silicon partner"),
                e("AMZN", "MRVL", "aws_interconnect", 1, "AWS Nitro/optical DSP"),
                e("AMZN", "NVDA", "ai_gpu_compute", 1, "EC2 GPU instances"),
                e("META", "ANET", "dc_networking", 1, "AI Ethernet fabric"),
                e("META", "NVDA", "ai_gpu_compute", 1, "Llama training clusters"),
                e("CRM", "LRCX", "enterprise_tech_cycle", 2, "Semi capex cycle proxy"),
                e("ANET", "ASX", "osat_packaging", 2, "Switch ASIC packaging"),
                e("ANET", "TSM", "foundry", 2, "Switching silicon foundry"),
                e("DELL", "ASML", "upstream_capex", 2, "Upstream semi capex"),
                e("DELL", "NVDA", "ai_servers", 1, "PowerEdge AI servers"),
                e("SMCI", "TER", "ate_test_cycle", 2, "Hardware test capex proxy"),
                e("SMCI", "NVDA", "ai_servers", 1, "Liquid-cooled GPU servers"),
            ],
        }
    }
}

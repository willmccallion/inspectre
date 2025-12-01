use serde::Deserialize;

const DEFAULT_RAM_BASE: u64 = 0x8000_0000;
const DEFAULT_RAM_SIZE: usize = 128 * 1024 * 1024;
const DEFAULT_KERNEL_OFFSET: u64 = 0x1000;

const UART_BASE: u64 = 0x1000_0000;
const DISK_BASE: u64 = 0x9000_0000;
const CLINT_BASE: u64 = 0x0200_0000;

const STACK_SIZE: usize = 0x800_000;
const BUS_WIDTH: u64 = 8;
const BUS_LATENCY: u64 = 4;
const CLINT_DIV: u64 = 10;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub system: SystemConfig,
    pub memory: MemoryConfig,
    pub cache: CacheHierarchyConfig,
    pub pipeline: PipelineConfig,
}

#[derive(Debug, Deserialize)]
pub struct GeneralConfig {
    pub trace_instructions: bool,
    #[serde(default = "default_start_pc")]
    pub start_pc: String,
    #[serde(default = "default_stack_size")]
    pub user_stack_size: usize,
}

impl GeneralConfig {
    pub fn start_pc_val(&self) -> u64 {
        let s = self.start_pc.trim_start_matches("0x");
        u64::from_str_radix(s, 16).unwrap_or(DEFAULT_RAM_BASE)
    }
}

#[derive(Debug, Deserialize)]
pub struct SystemConfig {
    #[serde(default = "default_uart_base")]
    pub uart_base: String,

    #[serde(default = "default_disk_base")]
    pub disk_base: String,

    #[serde(default = "default_ram_base")]
    pub ram_base: String,

    #[serde(default = "default_clint_base")]
    pub clint_base: String,

    #[serde(default = "default_kernel_offset")]
    pub kernel_offset: u64,

    #[serde(default = "default_bus_width")]
    pub bus_width: u64,

    #[serde(default = "default_bus_latency")]
    pub bus_latency: u64,

    #[serde(default = "default_clint_div")]
    pub clint_divider: u64,
}

impl SystemConfig {
    pub fn uart_base_val(&self) -> u64 {
        parse_hex(&self.uart_base, UART_BASE)
    }

    pub fn disk_base_val(&self) -> u64 {
        parse_hex(&self.disk_base, DISK_BASE)
    }

    pub fn ram_base_val(&self) -> u64 {
        parse_hex(&self.ram_base, DEFAULT_RAM_BASE)
    }

    pub fn clint_base_val(&self) -> u64 {
        parse_hex(&self.clint_base, CLINT_BASE)
    }
}

#[derive(Debug, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_ram_size")]
    pub ram_size: String,

    #[serde(default = "default_controller")]
    pub controller: String,

    #[serde(default = "default_t_cas")]
    pub t_cas: u64,

    #[serde(default = "default_t_ras")]
    pub t_ras: u64,

    #[serde(default = "default_t_pre")]
    pub t_pre: u64,

    #[serde(default = "default_row_miss")]
    pub row_miss_latency: u64,

    #[serde(default = "default_tlb_size")]
    pub tlb_size: usize,
}

impl MemoryConfig {
    pub fn ram_size_val(&self) -> usize {
        let s = self.ram_size.trim_start_matches("0x");
        usize::from_str_radix(s, 16).unwrap_or(DEFAULT_RAM_SIZE)
    }
}

fn parse_hex(s: &str, default: u64) -> u64 {
    let s = s.trim_start_matches("0x");
    u64::from_str_radix(s, 16).unwrap_or(default)
}

fn default_start_pc() -> String {
    format!("{:#x}", DEFAULT_RAM_BASE)
}

fn default_stack_size() -> usize {
    STACK_SIZE
}

fn default_uart_base() -> String {
    format!("{:#x}", UART_BASE)
}

fn default_disk_base() -> String {
    format!("{:#x}", DISK_BASE)
}

fn default_ram_base() -> String {
    format!("{:#x}", DEFAULT_RAM_BASE)
}

fn default_clint_base() -> String {
    format!("{:#x}", CLINT_BASE)
}

fn default_ram_size() -> String {
    format!("{:#x}", DEFAULT_RAM_SIZE)
}

fn default_kernel_offset() -> u64 {
    DEFAULT_KERNEL_OFFSET
}

fn default_bus_width() -> u64 {
    BUS_WIDTH
}

fn default_bus_latency() -> u64 {
    BUS_LATENCY
}

fn default_clint_div() -> u64 {
    CLINT_DIV
}

fn default_controller() -> String {
    "Simple".to_string()
}

fn default_row_miss() -> u64 {
    120
}

fn default_tlb_size() -> usize {
    32
}

fn default_t_cas() -> u64 {
    14
}

fn default_t_ras() -> u64 {
    14
}

fn default_t_pre() -> u64 {
    14
}

#[derive(Debug, Deserialize, Clone)]
pub struct CacheHierarchyConfig {
    pub l1_i: CacheConfig,
    pub l1_d: CacheConfig,
    pub l2: CacheConfig,
    pub l3: CacheConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CacheConfig {
    pub enabled: bool,

    #[serde(default = "d_c_size")]
    pub size_bytes: usize,

    #[serde(default = "d_c_line")]
    pub line_bytes: usize,

    #[serde(default = "d_c_ways")]
    pub ways: usize,

    #[serde(default = "d_c_policy")]
    pub policy: String,

    #[serde(default = "d_c_lat")]
    pub latency: u64,

    #[serde(default = "d_c_pref")]
    pub prefetcher: String,

    #[serde(default = "d_c_pref_t")]
    pub prefetch_table_size: usize,

    #[serde(default = "d_c_pref_d")]
    pub prefetch_degree: usize,
}

fn d_c_size() -> usize {
    4096
}

fn d_c_line() -> usize {
    64
}

fn d_c_ways() -> usize {
    1
}

fn d_c_policy() -> String {
    "LRU".to_string()
}

fn d_c_lat() -> u64 {
    1
}

fn d_c_pref() -> String {
    "None".to_string()
}

fn d_c_pref_t() -> usize {
    64
}

fn d_c_pref_d() -> usize {
    1
}

#[derive(Debug, Deserialize, Clone)]
pub struct PipelineConfig {
    pub branch_predictor: String,
    pub btb_size: usize,
    pub ras_size: usize,
    pub misa_override: Option<String>,

    #[serde(default)]
    pub tage: TageConfig,

    #[serde(default)]
    pub perceptron: PerceptronConfig,

    #[serde(default)]
    pub tournament: TournamentConfig,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TageConfig {
    #[serde(default = "d_t_b")]
    pub num_banks: usize,

    #[serde(default = "d_t_s")]
    pub table_size: usize,

    #[serde(default = "d_t_l")]
    pub loop_table_size: usize,

    #[serde(default = "d_t_r")]
    pub reset_interval: u32,

    #[serde(default = "d_t_h")]
    pub history_lengths: Vec<usize>,

    #[serde(default = "d_t_tag")]
    pub tag_widths: Vec<usize>,
}

fn d_t_b() -> usize {
    4
}

fn d_t_s() -> usize {
    2048
}

fn d_t_l() -> usize {
    256
}

fn d_t_r() -> u32 {
    256_000
}

fn d_t_h() -> Vec<usize> {
    vec![5, 15, 44, 130]
}

fn d_t_tag() -> Vec<usize> {
    vec![9, 9, 10, 10]
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PerceptronConfig {
    #[serde(default = "d_p_h")]
    pub history_length: usize,

    #[serde(default = "d_p_b")]
    pub table_bits: usize,
}

fn d_p_h() -> usize {
    32
}

fn d_p_b() -> usize {
    10
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TournamentConfig {
    #[serde(default = "d_to_g")]
    pub global_size_bits: usize,

    #[serde(default = "d_to_l")]
    pub local_hist_bits: usize,

    #[serde(default = "d_to_p")]
    pub local_pred_bits: usize,
}

fn d_to_g() -> usize {
    12
}

fn d_to_l() -> usize {
    10
}

fn d_to_p() -> usize {
    10
}

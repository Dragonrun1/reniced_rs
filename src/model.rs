use regex::Regex;

#[derive(Debug, Clone)]
pub struct Rule {
    pub regex: Regex,
    pub nice: Option<i32>,
    pub oom_adj: Option<i32>,
    pub io_class: Option<IoClass>,
    pub io_nice: Option<u8>,
}

#[derive(Debug, Clone, Copy)]
pub enum IoClass {
    Realtime,
    BestEffort,
    Idle,
}

#[derive(Debug, Clone)]
pub struct ProcessEntry {
    pub pid: i32,
    pub cmd: String,
}

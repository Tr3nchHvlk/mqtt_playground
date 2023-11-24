use clap::Parser;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref CLI_ARGS: CliArgs = CliArgs::parse();
}

#[derive(Parser, Debug, Clone)]
pub struct CliArgs {
    // Target server/host URI
    #[arg(short='t', long="TARGET", default_value="mqtt://localhost:1883")]
    pub target_host_uri: String,

    // Delay level as in delay = 2 ^ (delay_level - 1)
    #[arg(short='d', long="DMIN", default_value="0")]
    pub delay_level_min: u64,

    // Delay level as in delay = 2 ^ (delay_level - 1)
    #[arg(short='D', long="DMAX", default_value="5")]
    pub delay_level_max: u64,

    // Maximum number of instances count
    #[arg(short='i', long="IMIN", default_value="1")]
    pub instancecount_min: usize,

    // Maximum number of instances count
    #[arg(short='I', long="IMAX", default_value="3")]
    pub instancecount_max: usize,

    // Measuring runtime
    #[arg(short='m', long="MRT", default_value="60")]
    pub mrt: u64,

    // Buffering period in-between each iteration
    #[arg(short='r', long="R", default_value="10")]
    pub reset_buffer: u64,
}
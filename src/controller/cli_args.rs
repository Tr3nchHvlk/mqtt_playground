use clap::Parser;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref CLI_ARGS: CliArgs = CliArgs::parse();
}

#[derive(Parser, Debug, Clone)]
pub struct CliArgs {
    #[arg(short='t', long="TARGET", default_value="mqtt://localhost:1883")]
    pub target_host_uri: String,
}
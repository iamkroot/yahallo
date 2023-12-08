use clap::Parser;

struct Cli {
    
}

#[derive(clap::Subcommand)]
enum Commands {
    Add {
        #[arg(long)]
        label: Option<String>,
    },
    Test{
        /// Whether to exit after first match
        #[arg(long)]
        exit_on_match: bool,
        /// When to exit. Runs indefinitely unless specified.
        #[arg(long)]
        duration: Option<u32>,
    },
}


fn main() {
    
}
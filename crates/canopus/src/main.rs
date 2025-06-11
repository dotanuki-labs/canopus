// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

mod core;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct ProgramArguments {
    #[arg(short, long)]
    name: String,
}

fn main() {
    better_panic::install();
    human_panic::setup_panic!();

    let arguments = ProgramArguments::parse();
    let greet = core::greet(&arguments.name).expect("Expecting a greet!");
    println!("{}", greet);
}

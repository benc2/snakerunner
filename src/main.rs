use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

mod parse_instruction;
mod showgame;

mod game;
mod running;
use running::{play_game, play_match};

#[derive(Parser)]
#[command(name = "snakerunner", author, version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Runs a game between given scripts once and outputs the winner. Logs all moves to a file
    Run(RunArgs),
    /// Shows a game from a log file in the terminal
    Show(ShowArgs),
    /// Plays a match consisting of multiple games and outputs which script won most games. Plays a tiebreaker if necessary. Starting positions and the order in which the scripts play is randomized for each game.
    Match(MatchArgs),
}

#[derive(Args)]
struct RunArgs {
    /// The names of the scripts you want to run. If the script name ends in .py, it will be run as a python file. Otherwise, it will be assumed to be a compiled executable.
    #[arg(short, long, num_args(2..), required=true)]
    scripts: Vec<String>,

    /// Specify the starting positions of the players. Randomly assigned if not specified. Format: x and y coordinates separated by a comma with no space in between, and different positions separated by a space. E.g. snakerunner run -s a.py b.py -p 1,2 3,4
    #[arg(short, long, num_args(2..))]
    positions: Option<Vec<String>>,

    /// Width of the playing field
    #[arg(short = 'x', long, default_value_t = 10)]
    width: usize,

    /// Height of the playing field
    #[arg(short = 'y', long, default_value_t = 10)]
    height: usize,

    /// Print all inputs to and outputs from players, as well as the board at every move
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    /// Name of the output file to which the moves are logged [default: log.txt]
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Time limit for each move in milliseconds. First move gets 10x more time to allow for some setup.
    #[arg(short, long, default_value_t = 100)]
    timelimit: u64,
}

#[derive(Args)]
struct ShowArgs {
    /// Name of the log file to read from
    #[arg(short, long)]
    input: Option<String>,

    /// Timestep between moves in milliseconds
    #[arg(short, long, default_value_t = 500)]
    timestep: u64,
}

#[derive(Args)]
struct MatchArgs {
    /// The names of the scripts you want to run. If the script name ends in .py, it will be run as a python file. Otherwise, it will be assumed to be a compiled executable.
    #[arg(short, long, num_args(2..), required=true)]
    scripts: Vec<String>,

    /// Width of the playing field
    #[arg(short = 'x', long, default_value_t = 10)]
    width: usize,

    /// Height of the playing field
    #[arg(short = 'y', long, default_value_t = 10)]
    height: usize,

    /// Number of games to be played. A tiebreaker may be played, so actual amount of games played might be 1 higher
    #[arg(short, long)]
    n_games: usize,

    /// Name of the output file to which the moves are logged [default: summary.txt]
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Time limit for each move in milliseconds. First move gets 10x more time to allow for some setup.
    #[arg(short, long, default_value_t = 100)]
    timelimit: u64,

    /// Save logs in this folder
    #[arg(short, long)]
    logs: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run(runargs) => {
            // parse starting positions
            let starting_config = match runargs.positions {
                None => None,
                Some(vector) => {
                    if vector.len() != runargs.scripts.len() {
                        println!("Incorrect number of starting positions");
                        return;
                    }
                    let mut starting_coords = Vec::new();
                    for coords_str in vector {
                        match showgame::parse_usize_pair(&coords_str) {
                            Ok(coords) => {
                                starting_coords.push(coords);
                            }
                            Err(_) => {
                                println!("Could not parse coordinates");
                                return;
                            }
                        }
                    }
                    Some(starting_coords)
                }
            };

            // play the game!
            if let Some((winner, _)) = play_game(
                &runargs.scripts.iter().map(String::as_str).collect(),
                starting_config,
                runargs.width,
                runargs.height,
                Some(&runargs.output.unwrap_or(PathBuf::from("log.txt"))),
                runargs.verbose,
                runargs.timelimit,
            ) {
                println!("Player {winner} won!");
            } else {
                println!(
                    "Error caused all remaining players to quit before winner could be determined"
                );
            };
        }
        Commands::Show(showargs) => {
            showgame::showgame(
                &showargs.input.unwrap_or("log.txt".into()),
                showargs.timestep,
            )
            .unwrap();
        }
        Commands::Match(matchargs) => {
            let winner = play_match(
                matchargs.scripts.iter().map(String::as_str).collect(),
                matchargs.width,
                matchargs.height,
                matchargs.n_games,
                matchargs.timelimit,
                &matchargs.output.unwrap_or(PathBuf::from("summary.txt")),
                matchargs.logs,
            );
            println!("Player {winner} won the match!");
        }
    }
}

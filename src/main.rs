use std::collections::HashSet;
use std::fs::File;
// use std::fmt::write;
use anyhow::Result;
use std::io::{BufRead, BufReader, LineWriter, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::time::Duration;
// use std::sync::{Arc, Mutex};
use clap::{Args, Parser, Subcommand};

use std::thread;
mod parse_instruction;
mod showgame;

use rand::distributions::{Distribution, Uniform};
use rand::seq::SliceRandom;
use rand::thread_rng;
mod game;
use game::{Direction, TorusSnakeGame};

enum Message {
    CommunicateMove { direction: Direction, player: usize },
    AskMove(usize), // ask player to move
    Kill(usize),
    SendHeader(String),
}

fn make_process_python(program_name: &str) -> Child {
    let python_command = "python3";
    #[cfg(target_os = "windows")]
    let python_command = "python";

    Command::new(python_command)
        .arg("-m")
        .arg(program_name)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
}

fn make_process_exe(program_name: &str) -> Child {
    Command::new(format!(r#"./{program_name}"#))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
}

fn make_process(filename: &str) -> Child {
    if let Some(program_name) = filename.strip_suffix(".py") {
        make_process_python(program_name)
    } else {
        make_process_exe(filename)
    }

    // else if let Some(program_name) = filename.strip_suffix(".exe") {
    //     make_process_exe(program_name)
    // } else {
    //     panic!()
    // }
}

fn write_to_player(
    message: &str,
    player: usize,
    stdin: &mut ChildStdin,
    sender: &mpsc::Sender<usize>,
    alive_players: &mut HashSet<usize>,
    verbose: bool,
) {
    if verbose {
        println!("->p{player}  \"{message}\"")
    }
    stdin
        .write_all(format!("{message}\n").as_bytes())
        .unwrap_or_else(|_| {
            alive_players.remove(&player);
            // let _ = sender.send(player); // Not much we can do if this fails
        })
}

fn log(message: &str, writer_opt: &mut Option<LineWriter<File>>) -> Result<(), std::io::Error> {
    if let Some(writer) = writer_opt {
        writer.write_fmt(format_args!("{message}\n"))
    } else {
        Ok(())
    }
}

fn play_game(
    scripts: &Vec<&str>,
    starting_config: Option<Vec<(usize, usize)>>,
    width: usize,
    height: usize,
    log_filename: Option<&str>,
    verbose: bool,
) -> Option<usize> {
    let logfile = log_filename.map(|filename| File::create(filename).unwrap());
    let mut writer = logfile.map(LineWriter::new);

    let n_players = scripts.len();

    let starting_positions = starting_config.unwrap_or_else(|| {
        let mut starting_positions = Vec::new();
        let x_sampler = Uniform::new(0, width);
        let y_sampler = Uniform::new(0, height);
        while starting_positions.len() < n_players {
            // TODO if n_players too high, this loop can run forever
            let start_pos = (
                x_sampler.sample(&mut rand::thread_rng()),
                y_sampler.sample(&mut rand::thread_rng()),
            );
            if !starting_positions.contains(&start_pos) {
                starting_positions.push(start_pos);
            }
        }
        starting_positions
    });

    let mut game = TorusSnakeGame::new(width, height, starting_positions);

    let mut children: Vec<Child> = scripts
        .into_iter()
        .map(|script_name| make_process(script_name.as_ref()))
        .collect();
    let mut stdins: Vec<_> = children
        .iter_mut()
        .map(|child| child.stdin.take().expect("Child has no stdin"))
        .collect();

    let mut readers: Vec<BufReader<ChildStdout>> = children
        .iter_mut()
        .map(|child| BufReader::new(child.stdout.take().expect("Child has no stdout")))
        .collect();

    let (read_sender, read_receiver) = mpsc::channel();
    let (write_sender, write_receiver) = mpsc::channel(); // for now just used to kill child scripts

    thread::spawn(move || {
        use Message::*;

        let mut alive_players: HashSet<usize> = (0..n_players).collect();
        for message in read_receiver.iter() {
            match message {
                CommunicateMove { direction, player } => {
                    log(&format!("{player}:{direction}"), &mut writer).unwrap();

                    for (opponent_player, stdin) in stdins.iter_mut().enumerate() {
                        if opponent_player == player || !alive_players.contains(&opponent_player) {
                            continue;
                        }

                        write_to_player(
                            &format!("{}:{}", player, direction),
                            opponent_player,
                            stdin,
                            &write_sender,
                            &mut alive_players,
                            verbose,
                        );
                        // println!("->p{opponent_player}  \"{}:{}\"", player, direction);
                    }
                }

                AskMove(player) => {
                    // stdins[player]
                    //     .write_all(b"move\n")
                    //     .expect("Asking for move failed");
                    write_to_player(
                        "move",
                        player,
                        &mut stdins[player],
                        &write_sender,
                        &mut alive_players,
                        verbose,
                    );
                    // println!("->p{player}  \"move\"");
                }

                Kill(player) => {
                    // println!("Gotta kill {player}");
                    // writer.write_fmt(format_args!("out:{player}\n")).unwrap(); // TODO: fix parsing in showgame.rs to support this
                    alive_players.remove(&player);
                    // stdins[player].write_all(b"dead\n").expect("Kill failed");
                    write_to_player(
                        "stop",
                        player,
                        &mut stdins[player],
                        &write_sender,
                        &mut alive_players,
                        verbose,
                    );
                    // println!("->p{player}  \"stop\"");

                    for (opponent_player, stdin) in stdins.iter_mut().enumerate() {
                        if opponent_player == player || !alive_players.contains(&opponent_player) {
                            continue;
                        }

                        write_to_player(
                            &format!("out:{}", player),
                            opponent_player,
                            stdin,
                            &write_sender,
                            &mut alive_players,
                            verbose,
                        );
                        // println!("->p{opponent_player}  \"out:{}\"", player);
                    }
                }

                SendHeader(header) => {
                    for (player, stdin) in stdins.iter_mut().enumerate() {
                        // stdin
                        //     .write_all(format!("{header}\n{player}\n").as_bytes())
                        //     .expect("Writing header failed");
                        write_to_player(
                            &format!("{header}\n{player}"),
                            player,
                            stdin,
                            &write_sender,
                            &mut alive_players,
                            verbose,
                        );
                        // println!("->p{player} \"{header}\n{player}\"");
                    }
                    // writer.write_fmt(format_args!("{header}\n")).unwrap();
                    log(&header, &mut writer).unwrap();
                }
            }

            // thread::sleep(Duration::from_millis(50));
        }
    });

    let (listener_sender, listener_receiver) = mpsc::channel(); // determines which child the thread will try to read from
    let (readline_sender, readline_receiver) = mpsc::channel();
    thread::spawn(move || {
        for player in listener_receiver {
            let mut buffer = String::new();
            let reader: &mut BufReader<ChildStdout> = &mut readers[player]; // why do we need type annotation?
            let _ = reader.read_line(&mut buffer); //error handling? If it fails, it's probably players fault, so just return the empty buffer and let outer loop kill them
            readline_sender.send(buffer).unwrap();
        }
    });

    // let mut attempts = 0;
    let mut alive_players: HashSet<usize> = (0..n_players).collect();

    if verbose {
        println!("{}", game.setup_string());
    }
    read_sender
        .send(Message::SendHeader(game.setup_string()))
        .unwrap();

    'mainloop: loop {
        if verbose {
            println!("\nRemaining players: {:?}", alive_players);
        }
        for player in 0..n_players {
            // for player in write_receiver.iter() {
            //     // usually a broken player will still give an empty string in read_line, but do this just to be sure
            //     alive_players.remove(&player);
            //     // let _ = children[player].kill();
            // }
            if alive_players.len() < 2 {
                // need to check here, since game can end after any move
                if verbose {
                    println!("done");
                }
                break 'mainloop;
            }

            if !alive_players.contains(&player) {
                continue;
            }
            read_sender.send(Message::AskMove(player)).unwrap();
            listener_sender.send(player).unwrap();
            let line = match readline_receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => line,
                Err(_) => {
                    kill_player(player, &read_sender, &mut alive_players);
                    let _ = children[player].kill(); // TODO: maybe remove if we have a good plan for when to kill processes
                    if verbose {
                        println!("Killing player {player} due to timeout");
                    }
                    continue;
                }
            };
            // .to_owned();

            // trim_newline(&mut line);
            if verbose {
                println!("<-p{player}  \"{}\"", line.trim());
            }
            match line.trim().parse() {
                Ok(direction) => {
                    if !game.move_player(player, direction) {
                        if verbose {
                            println!("Killing player {player} due to losing move");
                        }
                        kill_player(player, &read_sender, &mut alive_players);
                    }
                    read_sender
                        .send(Message::CommunicateMove { direction, player })
                        .unwrap();
                }
                Err(_) => {
                    // invalid move input from player
                    if verbose {
                        println!("Killing player {player} due to invalid move");
                    }
                    kill_player(player, &read_sender, &mut alive_players);
                }
            }

            // TODO: should we kill a process when we kill the player?
            // TODO: kill players when they do not accept input
            // TODO: add time limit for players

            if verbose {
                println!("{}", game);
            }
        }
    }
    // println!("{:?}", kill_list);
    // thread::sleep(Duration::from_millis(50));
    for i in 0..n_players {
        let _ = children[i].kill();
        if alive_players.contains(&i) {
            // kill_player(i, &read_sender, &mut alive_players); // kill winner. Not necessary if we kill the processes
            return Some(i);
        }
    }
    None
}

fn log_summary(writer: &mut LineWriter<File>, wins: Vec<i32>) -> Result<(), std::io::Error> {
    let n_games: i32 = wins.iter().sum();
    writer.write_fmt(format_args!("n_games:{n_games}\n"))?;
    for (pn, p_wins) in wins.iter().enumerate() {
        writer.write_fmt(format_args!("{pn}:{p_wins}\n"))?
    }
    let winner = wins
        .iter()
        .enumerate()
        .max_by_key(|(_, &value)| value)
        .map(|(idx, _)| idx)
        .unwrap(); // argmax

    writer.write_fmt(format_args!("winner:{winner}\n"))?;

    Ok(())
}

fn play_match(
    scripts: Vec<&str>,
    width: usize,
    height: usize,
    log_filename: &str,
    n_games: usize,
) -> usize {
    let mut logwriter = LineWriter::new(File::create(log_filename).unwrap());
    let mut wins = vec![0; scripts.len()];
    let mut tagged_scripts: Vec<(usize, &str)> = scripts.into_iter().enumerate().collect();

    let mut shuffled_players: Vec<usize>;
    let mut shuffled_scripts: Vec<&str>;
    for _ in 0..n_games {
        tagged_scripts.shuffle(&mut thread_rng());
        (shuffled_players, shuffled_scripts) = tagged_scripts
            .iter()
            .map(|(n, script)| (n, *script))
            .unzip();

        if let Some(winner) = play_game(&shuffled_scripts, None, width, height, None, false) {
            wins[shuffled_players[winner]] += 1;
        }
    }

    // let mut candidates: HashSet<usize>;
    // let mut candidate_scripts: Vec<String>;
    let max_wins = *wins.iter().max().unwrap();
    // TODO: Error handling: make sure scripts.len != 0

    tagged_scripts = tagged_scripts
        .into_iter()
        .filter(|(pn, _)| wins[*pn] == max_wins)
        .collect(); // filter only tied winning players

    if tagged_scripts.len() == 0 {
        panic!("") // TODO: handle error. Can only occur if scripts.len() == 0
    }
    if let [(pn, _)] = tagged_scripts[..] {
        log_summary(&mut logwriter, wins).unwrap();
        return pn;
    } // length 1 => return

    // 2 or more tied players: tiebreaker
    for _ in 0..10 {
        // should only occur once,
        // but if play_game fails, might have to redo
        // 10 should be a safe margin
        tagged_scripts.shuffle(&mut thread_rng());
        (shuffled_players, shuffled_scripts) = tagged_scripts
            .iter()
            .map(|(n, script)| (n, *script))
            .unzip();

        if let Some(winner) = play_game(&shuffled_scripts, None, width, height, None, false) {
            wins[shuffled_players[winner]] += 1;
            log_summary(&mut logwriter, wins).unwrap();
            return shuffled_players[winner];
        };
    }

    tagged_scripts[0].0 // failsafe
}

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
    output: Option<String>,
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
    output: Option<String>,
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
            if let Some(winner) = play_game(
                &runargs.scripts.iter().map(String::as_str).collect(),
                starting_config,
                runargs.width,
                runargs.height,
                Some(&runargs.output.unwrap_or("log.txt".into())),
                runargs.verbose,
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
                &matchargs.output.unwrap_or("summary.txt".to_owned()),
                matchargs.n_games,
            );
            println!("Player {winner} won the match!");
        }
    }
}

fn kill_player(player: usize, sender: &Sender<Message>, alive_players: &mut HashSet<usize>) {
    alive_players.remove(&player);
    sender.send(Message::Kill(player)).unwrap();
}

use std::collections::HashSet;
use std::fs::File;
// use std::fmt::write;
use std::io::{BufRead, BufReader, LineWriter, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::time::Duration;
// use std::sync::{Arc, Mutex};
use clap::{Args, Parser, Subcommand};
use itertools::Itertools;
use std::thread;
mod showgame;
use colored::Colorize;
use rand::distributions::{Distribution, Uniform};

#[derive(Clone, Copy)]
enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    fn coord_shift(&self) -> (i32, i32) {
        match self {
            Self::North => (0, -1),
            Self::South => (0, 1),
            Self::East => (1, 0),
            Self::West => (-1, 0),
        }
    }
}

impl Into<String> for Direction {
    fn into(self) -> String {
        match self {
            Self::North => "N",
            Self::South => "S",
            Self::East => "E",
            Self::West => "W",
        }
        .into()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Could not be parsed to direction")]
struct InvalidDirection;

impl std::convert::TryFrom<&str> for Direction {
    type Error = InvalidDirection;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if let Some(char) = value.chars().next() {
            match char {
                'N' => Ok(Self::North),
                'S' => Ok(Self::South),
                'E' => Ok(Self::East),
                'W' => Ok(Self::West),
                _ => Err(InvalidDirection {}),
            }
        } else {
            Err(InvalidDirection)
        }
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", Into::<String>::into(*self))
    }
}
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

struct TorusSnakeGame {
    board: Vec<Vec<Option<usize>>>,
    height: usize,
    width: usize,
    head_positions: Vec<(usize, usize)>,
    alive_players: HashSet<usize>,
}

impl TorusSnakeGame {
    fn new(width: usize, height: usize, starting_positions: Vec<(usize, usize)>) -> Self {
        let mut board = vec![vec![None; width]; height];
        for (player_num, (x, y)) in starting_positions.iter().enumerate() {
            board[*y][*x] = Some(player_num);
        }
        let n_players = starting_positions.len();
        Self {
            board,
            height,
            width,
            head_positions: starting_positions,
            alive_players: (0..n_players).collect(),
        }
    }

    fn shift_coords(&self, (x, y): (usize, usize), direction: Direction) -> (usize, usize) {
        let (dx, dy) = direction.coord_shift();
        let new_x = ((x + self.width) as i32 + dx) as usize % self.width;
        let new_y = ((y + self.height) as i32 + dy) as usize % self.height;
        (new_x, new_y)
    }

    fn get(&self, (x, y): (usize, usize)) -> Option<usize> {
        self.board[y][x]
    }

    // fn clear(&mut self, (x, y): (usize, usize)) {
    //     self.board[y][x] = None;
    // }

    fn set_player(&mut self, (x, y): (usize, usize), player: usize) {
        self.board[y][x] = Some(player);
    }

    fn move_player(&mut self, player: usize, direction: Direction) -> bool {
        let new_pos = self.shift_coords(self.head_positions[player], direction);
        if self.get(new_pos).is_none() {
            self.head_positions[player] = new_pos;
            self.set_player(new_pos, player);
            true
        } else {
            self.alive_players.remove(&player);
            false
        }
    }

    fn display_cell(&self, pos: (usize, usize)) -> String {
        if let Some(player) = self.get(pos) {
            let disp = player.to_string();
            if self.head_positions[player] != pos {
                return disp;
            }
            let mut disp = disp.bold();
            if self.alive_players.contains(&player) {
                disp = disp.green()
            } else {
                disp = disp.red()
            }
            format!("{}", disp)
        } else {
            "·".into()
        }
    }

    #[allow(unstable_name_collisions)] // intersperse will be added to std, but change is probably not breaking
    fn setup_string(&self) -> String {
        format!(
            "{},{}\n{}\n{}",
            self.width,
            self.height,
            self.head_positions.len(),
            self.head_positions
                .iter()
                .map(|(x, y)| format!("{x},{y}"))
                .intersperse("\n".to_owned())
                .collect::<String>()
        )
    }
}

// fn display_cell(content: &Option<usize>, bold: bool) -> String {
//     let disp = if let Some(player) = content {
//         player.to_string()
//     } else {
//         "·".into()
//     };
//     if bold {
//         // format!("\x1b[1m{}\x1b[0m", disp)
//         format!("{}", disp.red().bold())
//     } else {
//         disp
//     }
// }

impl std::fmt::Display for TorusSnakeGame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let horizontal_border = format!("+{}+", "-".to_owned().repeat(self.width));
        write!(
            f,
            "{}\n{}{}",
            horizontal_border.clone(),
            (0..self.height)
                .map(|y| format!(
                    "|{}|\n",
                    (0..self.width)
                        .map(|x| self.display_cell((x, y)))
                        .collect::<String>()
                ))
                .collect::<String>(),
            // self.board
            //     .iter()
            //     .enumerate()
            //     .map(|(y, row)| format!(
            //         "|{}|\n",
            //         row.iter()
            //             .enumerate()
            //             .map(|(x, cell_content)| { self.display_cell((x, y)) })
            //             .collect::<String>()
            //     ))
            //     .collect::<String>(),
            horizontal_border
        )
    }
}

// fn main() {
//     let mut game = TorusSnakeGame::new(10, 10, vec![(0, 0), (6, 2), (6, 8)]);

//     for i in 0..7 {
//         if !game.move_player(2, Direction::South) {
//             {}
//         }
//     }
//     println!("{}", game);
// }
fn trim_newline(s: &mut String) {
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
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
            let _ = sender.send(player); // Not much we can do if this fails
        })
}

fn play_game(
    scripts: &Vec<String>,
    starting_config: Option<Vec<(usize, usize)>>,
    width: usize,
    height: usize,
    log_filename: &str,
    verbose: bool,
) -> Option<usize> {
    // let scripts = vec!["westmover.py", "southmover.py", "randommover.py"];

    let logfile = File::create(log_filename).unwrap();
    let mut writer = LineWriter::new(logfile);

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
                    writer
                        .write_fmt(format_args!("{player}:{direction}\n"))
                        .unwrap();
                    for (opponent_player, stdin) in stdins.iter_mut().enumerate() {
                        if opponent_player == player || !alive_players.contains(&opponent_player) {
                            continue;
                        }
                        // stdin
                        //     .write_all(format!("{}:{}\n", player, direction).as_bytes())
                        //     .expect("Writing move failed");
                        write_to_player(
                            &format!("{}:{}", player, direction),
                            player,
                            stdin,
                            &write_sender,
                            &mut alive_players,
                            verbose,
                        );
                        println!("->p{opponent_player}  \"{}:{}\"", player, direction);
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
                    println!("->p{player}  \"move\"");
                }

                Kill(player) => {
                    // println!("Gotta kill {player}");
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
                    println!("->p{player}  \"stop\"");

                    for (opponent_player, stdin) in stdins.iter_mut().enumerate() {
                        if opponent_player == player || !alive_players.contains(&opponent_player) {
                            continue;
                        }
                        write_to_player(
                            &format!("out:{}", player),
                            player,
                            stdin,
                            &write_sender,
                            &mut alive_players,
                            verbose,
                        );
                        println!("->p{opponent_player}  \"out:{}\"", player);
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
                        println!("->p{player} \"{header}\n{player}\"");
                    }
                    writer.write_fmt(format_args!("{header}\n")).unwrap();
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

    println!("{}", game.setup_string());
    read_sender
        .send(Message::SendHeader(game.setup_string()))
        .unwrap();

    'mainloop: loop {
        println!("\nRemaining players: {:?}", alive_players);
        for player in 0..n_players {
            // for player in write_receiver.iter() {
            //     // usually a broken player will still give an empty string in read_line, but do this just to be sure
            //     alive_players.remove(&player);
            //     // let _ = children[player].kill();
            // }
            if alive_players.len() < 2 {
                // need to check here, since game can end after any move
                println!("done");
                break 'mainloop;
            }

            if !alive_players.contains(&player) {
                continue;
            }
            read_sender.send(Message::AskMove(player)).unwrap();
            listener_sender.send(player).unwrap();
            let mut line = match readline_receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => line,
                Err(_) => {
                    kill_player(player, &read_sender, &mut alive_players);
                    let _ = children[player].kill(); // TODO: maybe remove if we have a good plan for when to kill processes
                    println!("Killing player {player} due to timeout");
                    continue;
                }
            };

            trim_newline(&mut line);
            println!("<-p{player}  \"{line}\"");
            match line.as_str().try_into() {
                Ok(direction) => {
                    if !game.move_player(player, direction) {
                        println!("Killing player {player} due to losing move");
                        kill_player(player, &read_sender, &mut alive_players);
                    }
                    read_sender
                        .send(Message::CommunicateMove { direction, player })
                        .unwrap();
                }
                Err(_) => {
                    // invalid move input from player
                    println!("Killing player {player} due to invalid move");
                    kill_player(player, &read_sender, &mut alive_players);
                }
            }

            // match reader.read_line(&mut buffer) {
            //     Ok(_) => {
            //         trim_newline(&mut buffer);
            //         println!("<-p{player}  \"{buffer}\"");
            //         match buffer.as_str().try_into() {
            //             Ok(direction) => {
            //                 if !game.move_player(player, direction) {
            //                     println!("Killing player {player} due to losing move");
            //                     kill_player(player, &read_sender, &mut alive_players);
            //                 }
            //                 read_sender
            //                     .send(Message::CommunicateMove { direction, player })
            //                     .unwrap();
            //             }
            //             Err(_) => {
            //                 // invalid move input from player
            //                 println!("Killing player {player} due to invalid move");
            //                 kill_player(player, &read_sender, &mut alive_players);
            //             }
            //         }
            //     }
            //     Err(err) => {
            //         // reading line failed (this should not happen)
            //         println!("{:?}", err);
            //         continue;
            //     } // figure out what to do here, shouldn't happen anyway
            // }

            // TODO: should we kill a process when we kill the player?
            // TODO: kill players when they do not accept input
            // TODO: add time limit for players

            println!("{}", game);
        }
    }
    // println!("{:?}", kill_list);
    // thread::sleep(Duration::from_millis(50));
    for i in 0..n_players {
        let _ = children[i].kill();
        if alive_players.contains(&i) {
            // kill_player(i, &read_sender, &mut alive_players); // kill winner. Not necessary if we kill the processes
            println!("Player {i} won!");
            return Some(i);
        }
    }
    None
}

#[derive(Parser)]
#[command(name = "snakerunner", author, version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Runs a game between given scripts once and logs the moves to a file
    Run(RunArgs),
    /// Shows a game from a log file in the terminal
    Show(ShowArgs),
}

#[derive(Args)]
struct RunArgs {
    /// The names of the scripts you want to run. If the script name ends in .py, it will be run as a python file. Otherwise, it will be assumed to be a compiled executable.
    #[arg(short, long, num_args(2..))]
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

    /// Name of the output file to which the moves are logged
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

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run(runargs) => {
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
            play_game(
                &runargs.scripts,
                starting_config,
                runargs.width,
                runargs.height,
                &runargs.output.unwrap_or("log.txt".into()),
                runargs.verbose,
            );
        }
        Commands::Show(showargs) => {
            showgame::showgame(
                &showargs.input.unwrap_or("log.txt".into()),
                showargs.timestep,
            )
            .unwrap();
        }
    }
}

fn kill_player(player: usize, sender: &Sender<Message>, alive_players: &mut HashSet<usize>) {
    alive_players.remove(&player);
    sender.send(Message::Kill(player)).unwrap();
}
